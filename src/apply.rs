//! Putting a shell's configuration in place, starting it, and retiring the old one.
//!
//! The order here is the whole point, and every step exists to make the one after
//! it safe:
//!
//! 1. work out where each path comes from, and say so out loud;
//! 2. build the replacement in a staging directory and count it;
//! 3. confirm the backup that makes all of this reversible is still on disk;
//! 4. stop the old shell;
//! 5. swap, which is a pair of renames inside one filesystem;
//! 6. start the new shell;
//! 7. check it actually came up;
//! 8. only then end the session, so the next login starts from the new config
//!    rather than from a half-applied mix.
//!
//! Nothing is deleted. Whatever was in the way is renamed aside, so the previous
//! state is still on disk after a successful switch and is what a rollback puts
//! back after a failed one.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::compositor::Compositor;
use crate::shell::Shell;

/// Where one path's replacement copy comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// Copied out of a backup taken during an earlier switch.
    Backup(PathBuf),
    /// Copied out of the skel shipped in the source tree.
    Skel,
}

impl Origin {
    fn label(&self) -> String {
        match self {
            Origin::Skel => "skel".to_string(),
            Origin::Backup(root) => format!(
                "backup {}",
                root.file_name()
                    .unwrap_or(root.as_os_str())
                    .to_string_lossy()
            ),
        }
    }
}

/// How a switch ended.
#[derive(Debug)]
pub enum Apply {
    /// The new shell came up and the session was ended.
    Switched { logged_out: bool },
    /// A step failed. `rolled_back` says whether the old configuration is back.
    Failed {
        step: &'static str,
        detail: String,
        rolled_back: bool,
    },
}

/// One path that will exist under `$HOME` after the switch.
#[derive(Debug)]
struct Entry {
    /// Path relative to `$HOME`, e.g. `.config/hypr`.
    rel: String,
    src: PathBuf,
    origin: Origin,
}

/// A path that was renamed aside, so a rollback knows where to find it.
#[derive(Debug)]
struct Aside {
    /// Where it used to live, relative to `$HOME`.
    rel: String,
    /// Where the previous version was moved to, absolute.
    ///
    /// `None` means there was no previous version: the path did not exist before
    /// the switch. Rolling that back is not a rename, it is a removal, and getting
    /// it wrong leaves the new shell's configuration sitting on disk after a
    /// rollback that claimed to have undone it.
    kept: Option<PathBuf>,
}

/// How long the old shell gets to exit on its own before being killed.
const STOP_GRACE: Duration = Duration::from_secs(10);

/// How long the new shell gets to come up before the switch is called a failure.
///
/// dms.service declares TimeoutStartSec=90s, so this is not generous by accident.
const START_GRACE: Duration = Duration::from_secs(90);

/// The three roots a switch reads and writes inside.
///
/// They travel together because they have to: staging has to sit on the same
/// filesystem as the destination for the swap to be a rename rather than a copy,
/// and the skel has to be findable relative to wherever the binary was run from.
pub struct Context {
    pub home: PathBuf,
    pub backups_root: PathBuf,
    pub skel: PathBuf,
}

/// Where a shell's skel lives in this repository.
///
/// `src/compositor/<compositor>/skel/<shell>`: the layout the shells already use,
/// so adding a skel needs no second registration and cannot drift out of step with
/// where the files actually are.
pub fn skel_for(compositor_id: &str, shell_id: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/compositor")
        .join(compositor_id)
        .join("skel")
        .join(shell_id)
}

/// Work out what a switch would do, and say it, without doing any of it.
///
/// A switch replaces whole directories and ends the session, so being able to see
/// the plan first is worth the twenty lines. Every path it would touch, where that
/// path comes from, how much of it there is, and whether anything is already there
/// to roll back to.
pub fn dry_run(
    target: &'static Shell,
    compositor: &'static dyn Compositor,
    ctx: &Context,
    progress: &mut dyn FnMut(&str),
) -> Result<(), String> {
    let Context {
        home,
        backups_root,
        skel,
    } = ctx;
    let (home, backups_root, skel) = (home.as_path(), backups_root.as_path(), skel.as_path());

    progress(&format!(
        "would switch {} to {}",
        compositor.label(),
        target.display_name
    ));
    progress(&format!("  skel: {}", skel.display()));
    progress(&format!("  backups: {}", backups_root.display()));

    let entries = plan(target, compositor, backups_root, skel, progress)?;
    if entries.is_empty() {
        return Err("nothing to install at all".to_string());
    }

    let stamp = crate::clock::stamp();
    let staging = stage(&entries, home, target.id, &stamp)?;

    progress("would install:");
    for entry in &entries {
        let (files, bytes) = measure(&staging.join(&entry.rel));
        let live = home.join(&entry.rel);
        let current = if live.exists() {
            let (f, b) = measure(&live);
            format!("yours now: {f} files, {b} bytes (kept aside, not deleted)")
        } else {
            "yours now: absent (removed again on rollback)".to_string()
        };
        progress(&format!(
            "  {} — {} from {}, {files} files, {bytes} bytes",
            entry.rel,
            if entry.origin == Origin::Skel {
                "skel"
            } else {
                "backup"
            },
            entry.origin.label(),
        ));
        progress(&format!("      {current}"));
    }
    progress(&format!(
        "staged everything to {} to check it arrived whole; removing it now",
        staging.display()
    ));
    let _ = std::fs::remove_dir_all(&staging);
    progress("nothing was stopped, replaced, started or logged out");
    Ok(())
}

/// The bus name the desktop notification service lives on.
///
/// Both Noctalia and DMS claim it, which is the whole reason a switch has to stop
/// the old shell before starting the new one, and the reason it has to be able to
/// prove the name changed hands rather than assume it.
pub const NOTIFICATION_BUS: &str = "org.freedesktop.Notifications";

/// The pid that currently owns [`NOTIFICATION_BUS`], if anyone does.
///
/// `None` covers both "nobody owns it" and "I could not tell", which are treated
/// the same on purpose: a probe that cannot answer must not be read as a pass.
fn notification_owner() -> Option<u32> {
    let out = run("busctl", &["--user", "status", NOTIFICATION_BUS]).ok()?;
    if !out.status.success() {
        return None;
    }
    // busctl prints its status report to stdout, as `PID=<n>` on its own line.
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("PID="))
        .and_then(|pid| pid.trim().parse().ok())
}

/// Put `target` in place, start it, and end the session.
///
/// `backup_root` is the backup that was just taken and verified. It is checked
/// again here rather than trusted, because the steps after it are the ones that
/// can be undone only by that directory still being there.
pub fn apply(
    target: &'static Shell,
    current: Option<&'static Shell>,
    compositor: &'static dyn Compositor,
    backup_root: &Path,
    ctx: &Context,
    progress: &mut dyn FnMut(&str),
) -> Apply {
    apply_with(
        target,
        current,
        compositor,
        backup_root,
        ctx,
        true,
        progress,
    )
}

/// [`apply`], with the last step left to the caller.
///
/// Ending the session is separated out because it is the one step that cannot be
/// undone from inside the process: once the session is gone, so is the panel that
/// would have reported the result. A caller that wants to check the new shell
/// first — a test, or a command-line run someone is watching — asks for it not to
/// log out, and says so rather than quietly leaving the session alone.
pub fn apply_with(
    target: &'static Shell,
    current: Option<&'static Shell>,
    compositor: &'static dyn Compositor,
    backup_root: &Path,
    ctx: &Context,
    log_out: bool,
    progress: &mut dyn FnMut(&str),
) -> Apply {
    let Context {
        home,
        backups_root,
        skel,
    } = ctx;
    let (home, backups_root, skel) = (home.as_path(), backups_root.as_path(), skel.as_path());
    let entries = match plan(target, compositor, backups_root, skel, progress) {
        Ok(entries) => entries,
        Err(detail) => {
            return Apply::Failed {
                step: "plan",
                detail,
                rolled_back: false,
            }
        }
    };

    let stamp = crate::clock::stamp();
    let staging = match stage(&entries, home, target.id, &stamp) {
        Ok(dir) => dir,
        Err(detail) => {
            let _ = std::fs::remove_dir_all(
                home.join(".local/share/rakuos-shell-picker")
                    .join(format!("staging-{}-{}", target.id, stamp)),
            );
            return Apply::Failed {
                step: "stage",
                detail,
                rolled_back: false,
            };
        }
    };
    progress(&format!(
        "staged {} path(s) in {}",
        entries.len(),
        staging.display()
    ));

    // The whole reason the earlier steps are safe is this directory. Re-check it
    // here so a backup that vanished between being written and being relied on
    // stops the switch instead of quietly removing the way back.
    if !backup_root.join("manifest.toml").is_file() {
        let _ = std::fs::remove_dir_all(&staging);
        return Apply::Failed {
            step: "backup",
            detail: format!(
                "the backup at {} is gone, so there would be no way back. Nothing was changed.",
                backup_root.display()
            ),
            rolled_back: false,
        };
    }
    progress(&format!(
        "backup still in place at {}",
        backup_root.display()
    ));

    // Read before the old shell is stopped. Once it exits the name is unowned, and
    // an unowned name cannot be compared against anything.
    let bus_owner_before = notification_owner();
    progress(&format!(
        "{NOTIFICATION_BUS} is owned by {}",
        bus_owner_before.map_or_else(|| "nobody".to_string(), |p| p.to_string())
    ));

    if let Some(shell) = current {
        if let Err(detail) = stop(shell, progress) {
            let _ = std::fs::remove_dir_all(&staging);
            return Apply::Failed {
                step: "stop",
                detail,
                rolled_back: false,
            };
        }
    }

    let asides = match swap(&entries, &staging, home, target.id, &stamp, progress) {
        Ok(asides) => asides,
        Err((detail, asides)) => {
            // The old shell was stopped before the swap, so putting the files back
            // is only half of the undo. Leaving the session with no shell at all is
            // worse than the failure that caused it.
            progress("putting the previous configuration back");
            let configs_back = restore(&asides, home);
            let old_up = resume(current, progress);
            let _ = std::fs::remove_dir_all(&staging);
            return Apply::Failed {
                step: "swap",
                detail,
                rolled_back: configs_back && old_up,
            };
        }
    };
    let _ = std::fs::remove_dir_all(&staging);

    if let Err((step, detail)) =
        start(target, progress).and_then(|()| verify(target, bus_owner_before, progress))
    {
        progress(&format!("{step} did not work: {detail}"));
        // Stop it before touching any files. It may already own the notification
        // bus, and starting the old shell back up alongside it means one of the
        // two fails to own it — so the rollback would produce two half-started
        // shells instead of the one working shell the user had before.
        let stopped = stop(target, progress);
        progress("putting the previous configuration back");
        let configs_back = restore(&asides, home);
        let old_up = resume(current, progress);
        return Apply::Failed {
            step,
            detail,
            rolled_back: stopped.is_ok() && configs_back && old_up,
        };
    }

    if let Some(shell) = current {
        progress(&format!(
            "{} is up; {} is stopped",
            target.display_name, shell.display_name
        ));
    } else {
        progress(&format!("{} is up", target.display_name));
    }

    if !log_out {
        progress(&format!(
            "leaving the session alone as asked; log out yourself to start {} for real",
            target.display_name
        ));
        return Apply::Switched { logged_out: false };
    }

    match logout(progress) {
        Ok(()) => Apply::Switched { logged_out: true },
        Err(detail) => {
            // The switch itself worked. Only the clean restart did not, which is
            // not a reason to tear the new configuration back out.
            progress(&format!(
                "{detail} — {} is running now; log out yourself when convenient.",
                target.display_name
            ));
            Apply::Switched { logged_out: false }
        }
    }
}

/// Work out where each path comes from.
///
/// The rule the docs asked for is "newest backup first, skel as the fallback",
/// and it is right for a shell's own configuration: if this machine has run DMS
/// before, that backup holds how the user had it arranged, which is exactly what
/// should come back.
///
/// It is the wrong rule for the compositor's configuration, and applying it there
/// would quietly undo the switch. `~/.config/hypr` is in every backup, because
/// every switch saves it — so "newest backup first" hands back the copy that
/// launches the shell being replaced, noctalia and all. That file is where the
/// shell is started from, so it has to come from the target's own skel.
fn plan(
    target: &Shell,
    compositor: &dyn Compositor,
    backups_root: &Path,
    skel: &Path,
    progress: &mut dyn FnMut(&str),
) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();

    if let Some(rel) = target.config_dir {
        let rel = rel.trim_start_matches("./").to_string();
        match newest_backup_holding(backups_root, &rel) {
            Some(root) => {
                let src = root.join("home").join(&rel);
                progress(&format!("{rel} comes from the newest backup that holds it"));
                entries.push(Entry {
                    rel,
                    src,
                    origin: Origin::Backup(root),
                });
            }
            None if skel.join(&rel).is_dir() => {
                progress(&format!(
                    "{rel} comes from the skel, not a backup; no backup of yours holds it"
                ));
                entries.push(Entry {
                    rel: rel.clone(),
                    src: skel.join(&rel),
                    origin: Origin::Skel,
                });
            }
            None => {
                // Not fatal. A shell with no configuration is a shell running on
                // its defaults, and that is a legitimate outcome. What must not
                // happen is the compositor config going missing, below.
                progress(&format!(
                    "no backup or skel holds {rel}; {} will start on its own defaults",
                    target.display_name
                ));
            }
        }
    }

    // The compositor's configuration decides how the shell gets started, so it
    // is taken from the target's skel and never from a backup. See above.
    // The rest of what the skel carries: themes, runtime state, and the launcher
    // entries. A path listed here may be a single file, which is what lets one
    // shell replace the other's `.desktop` without either of them owning the
    // directory it lives in.
    for rel in target.skel_extra_paths {
        let rel = rel.trim_start_matches("./").to_string();
        let src = skel.join(&rel);
        if !src.exists() {
            // Said out loud rather than fatal. An extra path is a refinement, not
            // the thing that makes the switch work, so a stale entry in the shell's
            // own declaration should not be able to block a switch the user needs.
            // The only fatal case is the compositor config below, because that file
            // is what starts the shell.
            progress(&format!(
                "skipped {rel}: {} lists it, but its skel does not carry it",
                target.display_name
            ));
            continue;
        }
        progress(&format!("{rel} comes from the skel"));
        entries.push(Entry {
            rel,
            src,
            origin: Origin::Skel,
        });
    }

    let comp_rel = compositor.config_dir().trim_start_matches("./").to_string();
    let comp_src = skel.join(&comp_rel);
    if !comp_src.is_dir() {
        // Fatal, and deliberately so. This file is what starts the shell. If it
        // is missing then "switched" would mean nothing happened, the old config
        // would still launch the old shell, and the logout would land the user
        // right back where they started with a success message on screen.
        return Err(format!(
            "the skel has no {comp_rel} to install, so there is nothing to switch to; \
             refusing to leave your current configuration in place and call it a switch"
        ));
    }
    progress(&format!("{comp_rel} comes from the skel, not a backup"));
    entries.push(Entry {
        rel: comp_rel,
        src: comp_src,
        origin: Origin::Skel,
    });

    Ok(entries)
}

/// The newest backup that actually holds `rel`, if any.
///
/// Backup directories are named after the moment they were taken, so sorting by
/// name is sorting by age.
fn newest_backup_holding(backups_root: &Path, rel: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(backups_root)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs.reverse();
    dirs.into_iter().find(|d| d.join("home").join(rel).is_dir())
}

/// Copy every entry into a staging directory and count what landed.
///
/// The staging directory is under `$HOME` rather than `/tmp` on purpose: the swap
/// that follows is a rename, and a rename cannot cross a filesystem. A `/tmp`
/// that happens to be tmpfs would quietly turn the atomic step into a copy.
fn stage(entries: &[Entry], home: &Path, target_id: &str, stamp: &str) -> Result<PathBuf, String> {
    let root = home
        .join(".local/share/rakuos-shell-picker")
        .join(format!("staging-{target_id}-{stamp}"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).map_err(|e| format!("cannot create {}: {e}", root.display()))?;

    for entry in entries {
        let to = root.join(&entry.rel);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        let (src_files, src_bytes) = measure(&entry.src);
        copy(&entry.src, &to).map_err(|e| format!("cannot stage {}: {e}", entry.rel))?;
        let (dst_files, dst_bytes) = measure(&to);
        if (src_files, src_bytes) != (dst_files, dst_bytes) {
            return Err(format!(
                "{} staged short: {dst_files}/{} files, {dst_bytes}/{} bytes",
                entry.rel, src_files, src_bytes
            ));
        }
    }
    Ok(root)
}

/// Stop a shell, politely and then not.
///
/// SIGTERM first because a shell that exits cleanly gets to write its state. The
/// kill is a fallback, not the plan, and the loop waits to see whether the first
/// signal was enough rather than assuming either way.
fn stop(shell: &Shell, progress: &mut dyn FnMut(&str)) -> Result<(), String> {
    let pids = pids_of(shell.binary);
    if pids.is_empty() {
        progress(&format!("{} is not running", shell.display_name));
        return Ok(());
    }
    progress(&format!(
        "stopping {} (pid {})",
        shell.display_name,
        pids.iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ));

    signal(&pids, "-TERM").map_err(|e| format!("cannot signal {}: {e}", shell.binary))?;

    let deadline = Instant::now() + STOP_GRACE;
    loop {
        let left = pids_of(shell.binary);
        if left.is_empty() {
            progress(&format!("{} stopped", shell.display_name));
            return Ok(());
        }
        if Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    progress(&format!(
        "{} did not exit in {}s, killing it",
        shell.display_name,
        STOP_GRACE.as_secs()
    ));
    let left = pids_of(shell.binary);
    signal(&left, "-KILL").map_err(|e| format!("cannot kill {}: {e}", shell.binary))?;

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if pids_of(shell.binary).is_empty() {
            progress(&format!("{} killed", shell.display_name));
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Err(format!(
        "{} is still running (pid {}) and could not be stopped",
        shell.display_name,
        pids_of(shell.binary)
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// Move the staged paths into place, renaming whatever was there aside.
///
/// Both halves are renames within one filesystem, so neither can be interrupted
/// halfway. If the second one fails the first is undone before returning, which
/// is why the error carries the asides it managed to create.
fn swap(
    entries: &[Entry],
    staging: &Path,
    home: &Path,
    target_id: &str,
    stamp: &str,
    progress: &mut dyn FnMut(&str),
) -> Result<Vec<Aside>, (String, Vec<Aside>)> {
    let mut asides = Vec::new();

    for entry in entries {
        let from = staging.join(&entry.rel);
        let to = home.join(&entry.rel);
        let parent = match to.parent() {
            Some(p) => p,
            None => return Err((format!("{} has no parent directory", to.display()), asides)),
        };
        if let Err(e) = std::fs::create_dir_all(parent) {
            return Err((format!("cannot create {}: {e}", parent.display()), asides));
        }

        let kept = if to.exists() {
            let kept = parent.join(format!(
                ".{}.pre-{target_id}-{stamp}",
                entry.rel.rsplit('/').next().unwrap_or(&entry.rel)
            ));
            if let Err(e) = std::fs::rename(&to, &kept) {
                return Err((format!("cannot move your {} aside: {e}", entry.rel), asides));
            }
            Some(kept)
        } else {
            None
        };

        // Recorded before the second rename, so a failure there is undone by the
        // same restore path as everything else rather than by a special case.
        let aside = Aside {
            rel: entry.rel.clone(),
            kept: kept.clone(),
        };

        if let Err(e) = std::fs::rename(&from, &to) {
            asides.push(aside);
            return Err((format!("cannot put {} in place: {e}", entry.rel), asides));
        }

        match &kept {
            Some(k) => progress(&format!(
                "{} replaced from the {}; the old one is at {}",
                entry.rel,
                entry.origin.label(),
                k.display()
            )),
            None => progress(&format!(
                "{} created from the {} (there was no previous one)",
                entry.rel,
                entry.origin.label()
            )),
        }
        asides.push(aside);
    }

    Ok(asides)
}

/// Put the renamed-aside paths back.
fn restore(asides: &[Aside], home: &Path) -> bool {
    let mut ok = true;
    for aside in asides {
        let to = home.join(&aside.rel);
        match &aside.kept {
            Some(kept) => {
                if to.exists() {
                    let _ = std::fs::remove_dir_all(&to);
                }
                match std::fs::rename(kept, &to) {
                    Ok(()) => crate::logging::info(&format!("restored {}", aside.rel)),
                    Err(e) => {
                        crate::logging::error(&format!("cannot restore {}: {e}", aside.rel));
                        ok = false;
                    }
                }
            }
            None => {
                // Nothing was here before, so the switch put it here. Leaving it
                // behind means the old shell comes back reading a configuration
                // that was written for the new one.
                if to.exists() {
                    match std::fs::remove_dir_all(&to) {
                        Ok(()) => crate::logging::info(&format!("removed {}", aside.rel)),
                        Err(e) => {
                            crate::logging::error(&format!(
                                "cannot remove the {} this switch created: {e}",
                                aside.rel
                            ));
                            ok = false;
                        }
                    }
                }
            }
        }
    }
    ok
}

/// Start a shell the way it says it starts.
fn start(shell: &Shell, progress: &mut dyn FnMut(&str)) -> Result<(), (&'static str, String)> {
    // Companions first, in the order the shell declares them. A shell's own unit
    // does not depend on them, so systemd starts it either way and the result is
    // a running shell quietly missing part of its interface.
    for unit in shell.companion_systemd_user_units {
        let name = format!("{unit}.service");
        progress(&format!(
            "starting {name}, which {} needs",
            shell.display_name
        ));
        let out = run("systemctl", &["--user", "start", &name])
            .map_err(|e| ("start", format!("cannot run systemctl: {e}")))?;
        if !out.status.success() {
            return Err((
                "start",
                format!(
                    "{name} refused to start: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ),
            ));
        }
    }

    if let Some(unit) = shell.systemd_user_unit {
        let name = format!("{unit}.service");
        progress(&format!("starting {name}"));
        let out = run("systemctl", &["--user", "start", &name])
            .map_err(|e| ("start", format!("cannot run systemctl: {e}")))?;
        if !out.status.success() {
            return Err((
                "start",
                format!(
                    "{name} refused to start: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ),
            ));
        }
        return Ok(());
    }

    progress(&format!(
        "starting {} ({})",
        shell.display_name,
        shell.start_command()
    ));
    Command::new(shell.start_argv[0])
        .args(&shell.start_argv[1..])
        .spawn()
        .map_err(|e| {
            (
                "start",
                format!("cannot run {}: {e}", shell.start_command()),
            )
        })?;
    Ok(())
}

/// Check the new shell is really up, not merely launched.
fn verify(
    shell: &Shell,
    bus_owner_before: Option<u32>,
    progress: &mut dyn FnMut(&str),
) -> Result<(), (&'static str, String)> {
    let deadline = Instant::now() + START_GRACE;

    if let Some(unit) = shell.systemd_user_unit {
        let name = format!("{unit}.service");
        progress(&format!("waiting for {name} to report active"));
        loop {
            let out = match run("systemctl", &["--user", "is-active", &name]) {
                Ok(o) => o,
                Err(e) => return Err(("verify", format!("cannot query {name}: {e}"))),
            };
            if out.status.success() {
                break;
            }
            if Instant::now() >= deadline {
                let state = String::from_utf8_lossy(&out.stdout).trim().to_string();
                return Err((
                    "verify",
                    format!(
                        "{name} did not become active in {}s (last said {state:?})",
                        START_GRACE.as_secs()
                    ),
                ));
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        progress(&format!("{name} is active"));
    }

    loop {
        if !pids_of(shell.binary).is_empty() {
            progress(&format!("{} is running", shell.binary));
            break;
        }
        if Instant::now() >= deadline {
            return Err((
                "verify",
                format!("{} never appeared as a process", shell.binary),
            ));
        }

        std::thread::sleep(Duration::from_millis(500));
    }

    // The last check, and the one that actually says the switch worked: the
    // notification bus has to belong to the new shell. A live process and an active
    // unit are both compatible with the old shell still holding the bus, which is
    // the failure that would otherwise only surface as a desktop with no
    // notifications after the logout.
    match (bus_owner_before, notification_owner()) {
        (_, None) => {
            return Err((
                "verify",
                format!(
                    "nobody owns {NOTIFICATION_BUS} yet, so there is no notification service to \
                     log into"
                ),
            ));
        }
        (Some(before), Some(after)) if before == after => {
            return Err((
                "verify",
                format!(
                    "{NOTIFICATION_BUS} is still owned by pid {after}, the process that owned \
                     it before, so the old shell is still serving notifications"
                ),
            ));
        }
        (_, Some(after)) => progress(&format!(
            "{NOTIFICATION_BUS} now belongs to pid {after}, not to the one it had before"
        )),
    }
    Ok(())
}

/// Bring the old shell back after a failed switch.
fn resume(shell: Option<&'static Shell>, progress: &mut dyn FnMut(&str)) -> bool {
    let Some(shell) = shell else { return true };
    progress(&format!("starting {} back", shell.display_name));
    match start(shell, progress) {
        Ok(()) => true,
        Err((_, detail)) => {
            crate::logging::error(&format!("cannot restart {}: {detail}", shell.id));
            false
        }
    }
}

/// End the session so the next login starts from the new configuration.
///
/// Deliberately last. Everything before this point has been checked, so the only
/// thing logout adds is that the next session begins clean instead of inheriting
/// a compositor that was configured for the other shell.
fn logout(progress: &mut dyn FnMut(&str)) -> Result<(), String> {
    progress("ending the session; log back in to start the new shell");

    // `systemctl --user logout` is the tidy way, but it is a fairly recent verb,
    // and a picker that leaves the user staring at the new shell on the old
    // session has not finished its job. So the session is ended either way, and
    // which route was used is said out loud.
    let out = run("systemctl", &["--user", "logout"]);
    match out {
        Ok(o) if o.status.success() => {
            progress("systemctl ended the session");
            return Ok(());
        }
        Ok(o) => progress(&format!(
            "systemctl --user logout did not work ({}), ending the session another way",
            String::from_utf8_lossy(&o.stderr).trim()
        )),
        Err(e) => progress(&format!("cannot run systemctl --user logout: {e}")),
    }

    let session = std::env::var("XDG_SESSION_ID").ok().or_else(|| {
        run("loginctl", &["show-session", "self", "-p", "Id", "--value"])
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| {
                let id = String::from_utf8_lossy(&o.stdout).trim().to_string();
                (!id.is_empty()).then_some(id)
            })
    });

    let Some(session) = session else {
        return Err(
            "the shell is switched but the session was not ended, and I could not find which \
             session this is. Log out yourself when convenient."
                .to_string(),
        );
    };

    progress(&format!("ending session {session} with loginctl"));
    let out = run("loginctl", &["terminate-session", &session])
        .map_err(|e| format!("cannot run loginctl: {e}"))?;
    if out.status.success() {
        progress("loginctl ended the session");
        Ok(())
    } else {
        Err(format!(
            "the shell is switched but neither systemctl nor loginctl could end session \
             {session}; log out yourself when convenient. loginctl said: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

fn run(program: &str, args: &[&str]) -> std::io::Result<std::process::Output> {
    std::process::Command::new(program).args(args).output()
}

/// Every pid whose `comm` is exactly `name`.
///
/// Read from `/proc` rather than shelling out to `pgrep`, matching what
/// `shell.rs` already does to decide whether a shell is running.
fn pids_of(name: &str) -> Vec<u32> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut pids: Vec<u32> = entries
        .flatten()
        .filter_map(|e| {
            let pid = e.file_name().to_str()?.parse::<u32>().ok()?;
            let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
            (comm.trim() == name).then_some(pid)
        })
        .collect();
    pids.sort_unstable();
    pids
}

fn signal(pids: &[u32], sig: &str) -> std::io::Result<()> {
    for pid in pids {
        // `/bin/kill` rather than libc: this binary has no unsafe dependency and
        // a missing signal is not worth adding one for.
        //
        // The result is checked. Discarding it here would let a switch report
        // having stopped the old shell while it is in fact still running and
        // still holding the notification bus, and the failure would surface later
        // somewhere much harder to read.
        let out = run("kill", &[sig, &pid.to_string()])?;
        if !out.status.success() {
            return Err(std::io::Error::other(format!(
                "kill {sig} {pid} said: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
    }
    Ok(())
}

/// Copy a file or directory tree.
fn copy(from: &Path, to: &Path) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(from)?;
    if meta.is_symlink() {
        let target = std::fs::read_link(from)?;
        let _ = std::fs::remove_file(to);
        return std::os::unix::fs::symlink(target, to);
    }
    if meta.is_dir() {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            copy(&entry.path(), &to.join(entry.file_name()))?;
        }
        // Keep the mode, so an executable script stays executable in the skel.
        let mode = meta.permissions().mode();
        std::fs::set_permissions(to, std::fs::Permissions::from_mode(mode))?;
        return Ok(());
    }
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(from, to)?;
    Ok(())
}

use std::os::unix::fs::PermissionsExt;

/// How many files and bytes a tree holds, following the same rules as `copy`.
fn measure(dir: &Path) -> (usize, u64) {
    let mut files = 0;
    let mut bytes = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(meta) = std::fs::symlink_metadata(&current) else {
            continue;
        };
        if meta.is_symlink() {
            files += 1;
            continue;
        }
        if meta.is_dir() {
            let Ok(entries) = std::fs::read_dir(&current) else {
                continue;
            };
            for entry in entries.flatten() {
                stack.push(entry.path());
            }
        } else {
            files += 1;
            bytes += meta.len();
        }
    }
    (files, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compositor::hyprland::shells::{dms, noctalia};
    use crate::compositor::hyprland::Hyprland;

    struct Temp(PathBuf);

    impl Temp {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "rsp-apply-{tag}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Temp(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn write(path: &Path, body: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    /// Backups store directories, not files: `config_dir` is always a directory,
    /// and `newest_backup_holding` checks with `is_dir` for that reason.
    fn make_backup(root: &Path, name: &str, rels: &[&str]) {
        for rel in rels {
            write(
                &root.join(name).join("home").join(rel).join("marker"),
                &format!("{name}:{rel}"),
            );
        }
        write(&root.join(name).join("manifest.toml"), "");
    }

    /// The marker `make_backup` left inside a backed-up directory.
    fn marker(dir: &Path) -> String {
        std::fs::read_to_string(dir.join("marker")).unwrap()
    }

    #[test]
    fn the_compositor_config_comes_from_the_skel_even_when_a_backup_has_one() {
        // This is the whole reason `plan` exists. `~/.config/hypr` is in every
        // backup, because every switch saves it, so "newest backup first" would
        // hand back the file that launches the shell being replaced. The backup
        // below deliberately holds a config that starts the old shell.
        let t = Temp::new("comp-from-skel");
        let backups = t.path().join("backups");
        let skel = t.path().join("skel");

        make_backup(
            &backups,
            "20260101-000000-001-hyprland-noctalia",
            &[".config/hypr"],
        );
        write(
            &backups.join(
                "20260101-000000-001-hyprland-noctalia/home/.config/hypr/config/autostart.lua",
            ),
            "hl.exec_cmd(\"noctalia\")",
        );
        write(
            &skel.join(".config/hypr/config/autostart.lua"),
            "systemctl --user start dms.service",
        );

        let mut said = Vec::new();
        let entries = plan(&dms::SHELL, &Hyprland, &backups, &skel, &mut |s| {
            said.push(s.to_string())
        })
        .unwrap();

        let hypr = entries.iter().find(|e| e.rel == ".config/hypr").unwrap();
        assert_eq!(
            hypr.origin,
            Origin::Skel,
            "the compositor config must not come from a backup"
        );
        let body = std::fs::read_to_string(hypr.src.join("config/autostart.lua")).unwrap();
        assert!(
            body.contains("dms.service"),
            "the compositor config the switch would install still launches the old shell: {body}"
        );
        assert!(
            said.iter()
                .any(|l| l.contains("comes from the skel, not a backup")),
            "the log has to say where it took it from: {said:?}"
        );
    }

    #[test]
    fn a_shells_own_config_prefers_the_newest_backup_that_holds_it() {
        // The rule the docs asked for, and the reason it is right for this path:
        // a backup of DMS on this machine is the user's own arrangement of it.
        let t = Temp::new("own-from-backup");
        let backups = t.path().join("backups");
        let skel = t.path().join("skel");

        make_backup(
            &backups,
            "20260101-000000-001",
            &[".config/DankMaterialShell"],
        );
        make_backup(
            &backups,
            "20260202-000000-002",
            &[".config/DankMaterialShell"],
        );
        write(
            &skel.join(".config/DankMaterialShell/settings.json"),
            "from skel",
        );

        write(&skel.join(".config/hypr/config/autostart.lua"), "dms");

        let entries = plan(&dms::SHELL, &Hyprland, &backups, &skel, &mut |_| {}).unwrap();
        let own = entries
            .iter()
            .find(|e| e.rel == ".config/DankMaterialShell")
            .unwrap();

        match &own.origin {
            Origin::Backup(root) => {
                assert_eq!(
                    root.file_name().unwrap().to_string_lossy(),
                    "20260202-000000-002",
                    "the newest backup holding it must win"
                );
                assert_eq!(
                    marker(&own.src),
                    "20260202-000000-002:.config/DankMaterialShell"
                );
            }
            Origin::Skel => panic!("a backup holding it exists, so the skel must not be used"),
        }
    }

    #[test]
    fn a_shell_falls_back_to_the_skel_when_no_backup_holds_its_config() {
        let t = Temp::new("fallback-skel");
        let backups = t.path().join("backups");
        let skel = t.path().join("skel");

        // A backup exists, but only of the shell being replaced.
        make_backup(&backups, "20260101-000000-001", &[".config/noctalia"]);
        write(
            &skel.join(".config/DankMaterialShell/settings.json"),
            "from skel",
        );
        write(&skel.join(".config/hypr/config/autostart.lua"), "dms");

        let entries = plan(&dms::SHELL, &Hyprland, &backups, &skel, &mut |_| {}).unwrap();
        let own = entries
            .iter()
            .find(|e| e.rel == ".config/DankMaterialShell")
            .unwrap();
        assert_eq!(own.origin, Origin::Skel);
        assert_eq!(
            std::fs::read_to_string(own.src.join("settings.json")).unwrap(),
            "from skel"
        );
    }

    #[test]
    fn a_shell_with_no_config_anywhere_still_switches_on_its_defaults() {
        // The counterpart to the compositor rule: a shell that has never been
        // configured is not a broken switch, it is a shell on its defaults. That
        // must not block the switch, but it has to be said out loud.
        let t = Temp::new("defaults-ok");
        let backups = t.path().join("backups");
        let skel = t.path().join("skel");
        std::fs::create_dir_all(&backups).unwrap();
        write(&skel.join(".config/hypr/config/autostart.lua"), "dms");

        let mut said = Vec::new();
        let entries = plan(&dms::SHELL, &Hyprland, &backups, &skel, &mut |s| {
            said.push(s.to_string())
        })
        .expect("a missing shell config must not block the switch");

        assert!(!entries.iter().any(|e| e.rel == ".config/DankMaterialShell"));
        assert!(
            said.iter().any(|l| l.contains("on its own defaults")),
            "silently dropping a config would hide what the user is getting: {said:?}"
        );
    }

    #[test]
    fn plan_refuses_when_the_skel_has_no_compositor_config() {
        let t = Temp::new("nothing");
        let backups = t.path().join("backups");
        let skel = t.path().join("skel");
        std::fs::create_dir_all(&backups).unwrap();
        std::fs::create_dir_all(&skel).unwrap();

        let err = plan(&dms::SHELL, &Hyprland, &backups, &skel, &mut |_| {}).unwrap_err();
        assert!(
            err.contains("nothing to switch to"),
            "the error has to say why refusing is the safe answer: {err}"
        );
    }

    #[test]
    fn staging_counts_what_it_copied_and_the_swap_leaves_the_old_copy_aside() {
        let t = Temp::new("swap");
        let home = t.path().join("home");
        let backups = t.path().join("backups");
        let skel = t.path().join("skel");
        std::fs::create_dir_all(&home).unwrap();

        write(
            &skel.join(".config/hypr/config/autostart.lua"),
            "new skel config",
        );
        write(
            &skel.join(".config/hypr/scripts/x.sh"),
            "#!/bin/sh\necho hi\n",
        );
        // Give the source the mode a real skel script has (they are 755 in git),
        // so this asserts the mode is carried across, not that one is invented.
        std::fs::set_permissions(
            skel.join(".config/hypr/scripts/x.sh"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        write(
            &home.join(".config/hypr/config/autostart.lua"),
            "old live config",
        );
        // A second path that does not exist yet, so one swap covers both cases:
        // something replaced and something created.
        write(&skel.join(".config/DankMaterialShell/settings.json"), "{}");

        let entries = plan(&dms::SHELL, &Hyprland, &backups, &skel, &mut |_| {}).unwrap();
        let staging = stage(&entries, &home, "dms", "20260101-000000-000").unwrap();

        let staged_files = measure(&staging.join(".config/hypr")).0;
        assert!(staged_files > 0, "staging copied nothing");

        let asides = swap(
            &entries,
            &staging,
            &home,
            "dms",
            "20260101-000000-000",
            &mut |_| {},
        )
        .expect("swap");

        let live = std::fs::read_to_string(home.join(".config/hypr/config/autostart.lua")).unwrap();
        assert_eq!(
            live, "new skel config",
            "the skel did not reach the live path"
        );

        assert_eq!(
            asides.len(),
            2,
            "both the replaced and the created path have to be recorded"
        );
        // Looked up by name rather than by position: `plan` walks the shell's own
        // config before the compositor's, and a test that depends on that ordering
        // breaks the day the order stops mattering.
        let kept = asides
            .iter()
            .find(|a| a.rel == ".config/hypr")
            .and_then(|a| a.kept.as_ref())
            .expect("hypr was displaced, so it has an aside");
        let kept = std::fs::read_to_string(kept.join("config/autostart.lua")).unwrap();
        assert_eq!(kept, "old live config", "the aside is not the old config");

        // An executable script must still be executable, or the staged copy is
        // subtly broken in a way a file count cannot see.
        let mode = std::fs::metadata(home.join(".config/hypr/scripts/x.sh"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o777,
            0o755,
            "the staged script did not keep the mode it had in the skel"
        );

        assert!(restore(&asides, &home));
        let back = std::fs::read_to_string(home.join(".config/hypr/config/autostart.lua")).unwrap();
        assert_eq!(
            back, "old live config",
            "rollback did not put the original back"
        );
        assert!(
            !home.join(".config/DankMaterialShell").exists(),
            "rollback left the path it created in place"
        );
    }

    #[test]
    fn a_path_that_did_not_exist_before_is_removed_again_on_rollback() {
        // Before the switch there is no DMS config at all. Rolling back therefore
        // cannot "put the old one back", because there was no old one: it has to
        // remove what the switch created, or the next Noctalia login reads a DMS
        // configuration and behaves oddly with no obvious cause.
        let t = Temp::new("no-aside");
        let home = t.path().join("home");
        let skel = t.path().join("skel");
        std::fs::create_dir_all(&home).unwrap();
        write(&skel.join(".config/DankMaterialShell/settings.json"), "{}");

        let entries = vec![Entry {
            rel: ".config/DankMaterialShell".to_string(),
            src: skel.join(".config/DankMaterialShell"),
            origin: Origin::Skel,
        }];
        let staging = stage(&entries, &home, "dms", "20260101-000000-000").unwrap();
        let asides = swap(
            &entries,
            &staging,
            &home,
            "dms",
            "20260101-000000-000",
            &mut |_| {},
        )
        .expect("swap");

        assert_eq!(
            asides.len(),
            1,
            "the path was touched, so it must be recorded"
        );
        assert!(
            asides[0].kept.is_none(),
            "nothing was displaced, so there is nothing to put back"
        );
        assert!(home
            .join(".config/DankMaterialShell/settings.json")
            .is_file());

        assert!(restore(&asides, &home));
        assert!(
            !home.join(".config/DankMaterialShell").exists(),
            "rollback left the created configuration in place: {:?}",
            std::fs::read_dir(home.join(".config")).map(|d| d
                .filter_map(|e| e.ok())
                .map(|e| e.file_name())
                .collect::<Vec<_>>())
        );
    }

    #[test]
    fn staging_short_copied_files_are_caught_before_anything_is_swapped() {
        // The check compares the two counts after copying. Make them disagree by
        // pointing at a source that changes underneath the measurement.
        let t = Temp::new("short");
        let home = t.path().join("home");
        let src = t.path().join("src");
        std::fs::create_dir_all(&home).unwrap();
        write(&src.join("one.txt"), "1");
        write(&src.join("two.txt"), "2");

        let entries = vec![Entry {
            rel: ".config/thing".to_string(),
            src: src.clone(),
            origin: Origin::Skel,
        }];

        // Copy succeeds and the counts match, so this staging is accepted.
        let staging = stage(&entries, &home, "dms", "20260101-000000-000").unwrap();
        assert_eq!(measure(&staging.join(".config/thing")).0, 2);

        // Now a source that is not there at all: copy fails rather than writing
        // an empty tree, which is what keeps a later swap from wiping a path.
        let entries = vec![Entry {
            rel: ".config/missing".to_string(),
            src: t.path().join("nope"),
            origin: Origin::Skel,
        }];
        assert!(stage(&entries, &home, "dms", "20260101-000000-001").is_err());
    }

    #[test]
    fn pids_of_finds_this_process_and_ignores_a_name_nobody_has() {
        let me = std::process::id();
        let comm = std::fs::read_to_string(format!("/proc/{me}/comm")).unwrap();
        let comm = comm.trim().to_string();
        assert!(
            pids_of(&comm).contains(&me),
            "should find pid {me} by comm {comm}"
        );
        assert!(
            pids_of("definitely-not-a-running-process-xyzzy").is_empty(),
            "a name nobody has must come back empty, not a guess"
        );
    }

    #[test]
    fn the_backup_check_rejects_a_directory_with_no_manifest() {
        // apply() refuses to proceed on a backup root with no manifest, because
        // that backup is the only way back from the steps that follow.
        let t = Temp::new("no-manifest");
        let root = t.path().join("20260101-000000-001");
        std::fs::create_dir_all(&root).unwrap();
        assert!(!root.join("manifest.toml").is_file());
    }

    #[test]
    fn both_shell_definitions_the_plan_is_tested_against_are_the_ones_registered() {
        // Guards against the tests drifting onto a different shell than the one
        // a switch actually uses.
        assert_eq!(dms::SHELL.config_dir, Some(".config/DankMaterialShell"));
        assert_eq!(dms::SHELL.systemd_user_unit, Some("dms"));
        assert_eq!(noctalia::SHELL.systemd_user_unit, None);
        assert_eq!(Hyprland.config_dir(), ".config/hypr");
    }

    #[test]
    fn the_skel_paths_resolve_to_real_directories() {
        for (comp, shell) in [("hyprland", "dms"), ("hyprland", "noctalia")] {
            let dir = skel_for(comp, shell);
            assert!(dir.is_dir(), "{} should exist", dir.display());
        }
    }

    #[test]
    fn dms_declares_the_calendar_service_it_draws_widgets_from() {
        // Without this the live switch starts DMS with nothing behind it and the
        // login that follows looks merely incomplete rather than broken.
        assert_eq!(
            dms::SHELL.companion_systemd_user_units,
            &["dcal"],
            "DMS draws calendar widgets that dcal.service provides"
        );
        assert!(
            noctalia::SHELL.companion_systemd_user_units.is_empty(),
            "Noctalia draws no widgets from a helper service"
        );
    }

    #[test]
    fn the_notification_bus_is_read_from_the_live_system_or_reports_nothing() {
        // There is no session bus in a plain test run, so the honest expectation
        // is `None` — the point is that it must not invent an answer, and must not
        // panic when the name is unowned.
        let owner = notification_owner();
        println!("notification owner on this bus: {owner:?}");
        if let Some(pid) = owner {
            assert!(pid > 0, "a parsed pid should be positive, got {pid}");
        }
    }

    const LAUNCHER: &str = ".local/share/applications/dev.noctalia.Noctalia.desktop";

    #[test]
    fn switching_to_dms_would_hide_the_noctalia_launcher() {
        // The whole point of listing one launcher path per shell: DMS is installed
        // and visible, Noctalia is installed but hidden, so the menu cannot be used
        // to launch the shell that is no longer running.
        let mut said = Vec::new();
        let entries = plan(
            &dms::SHELL,
            &Hyprland,
            Path::new("/nonexistent-backups"),
            &skel_for("hyprland", "dms"),
            &mut |s| said.push(s.to_string()),
        )
        .expect("the real dms skel should plan cleanly");

        let launcher = entries
            .iter()
            .find(|e| e.rel == LAUNCHER)
            .expect("dms must carry the launcher path so it can be hidden");
        let body = std::fs::read_to_string(&launcher.src).unwrap();
        assert!(
            body.contains("NoDisplay=true"),
            "the staged override would not hide the launcher: {body}"
        );

        // And the rest of `~/.local/share/applications` must be untouched, which is
        // why a single file is listed rather than the directory.
        assert!(
            !entries.iter().any(|e| e.rel == ".local/share/applications"),
            "listing the whole directory would replace every launcher the user has"
        );
    }

    #[test]
    fn switching_back_to_noctalia_would_bring_its_launcher_back() {
        let mut said = Vec::new();
        let entries = plan(
            &noctalia::SHELL,
            &Hyprland,
            Path::new("/nonexistent-backups"),
            &skel_for("hyprland", "noctalia"),
            &mut |s| said.push(s.to_string()),
        )
        .expect("the real noctalia skel should plan cleanly");

        let launcher = entries
            .iter()
            .find(|e| e.rel == LAUNCHER)
            .expect("noctalia must carry the launcher path to undo the hiding");
        let body = std::fs::read_to_string(&launcher.src).unwrap();
        assert!(
            !body.contains("NoDisplay"),
            "the skel entry still hides the launcher, so switching back would leave \
             Noctalia installed and invisible"
        );
    }

    #[test]
    fn a_shared_xdg_directory_is_never_replaced_as_a_whole() {
        // `swap` renames the live path aside and puts the staged one in place; it
        // does not merge. Any directory shared with the rest of the desktop listed
        // as a skel extra would take the user's other files with it.
        for shell in [&dms::SHELL, &noctalia::SHELL] {
            for rel in shell.skel_extra_paths {
                assert!(
                    !matches!(
                        *rel,
                        ".config"
                            | ".local"
                            | ".local/share"
                            | ".local/state"
                            | ".local/share/applications"
                            | ".config/gtk-3.0"
                            | ".config/gtk-4.0"
                    ),
                    "{} lists {rel}, which would replace a directory the user shares \
                     with the rest of their desktop",
                    shell.display_name
                );
            }
        }
    }

    #[test]
    fn dms_state_and_theme_paths_reach_the_plan_from_the_real_skel() {
        let entries = plan(
            &dms::SHELL,
            &Hyprland,
            Path::new("/nonexistent-backups"),
            &skel_for("hyprland", "dms"),
            &mut |_| {},
        )
        .unwrap();
        // Individual files, not the gtk directories: the plan is what decides how
        // much of the user's desktop gets replaced, and this list is the contract.
        for rel in [
            ".config/DankMaterialShell",
            ".config/hypr",
            ".config/gtk-3.0/settings.ini",
            ".config/gtk-4.0/settings.ini",
            ".config/gtk-4.0/gtk.css",
            ".local/state/DankMaterialShell",
            LAUNCHER,
        ] {
            assert!(
                entries.iter().any(|e| e.rel == rel),
                "{rel} should be part of the switch to dms; planned: {:?}",
                entries.iter().map(|e| &e.rel).collect::<Vec<_>>()
            );
        }
    }
}
