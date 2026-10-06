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
/// `src/compositor/<compositor>/shells/<shell>/skel`: the templates sit inside the
/// shell's own folder, so everything about one shell on one compositor is in one
/// place — the `PORT` that says where its files go, and the files themselves. The
/// path is derived rather than registered, so a skel cannot end up somewhere other
/// than where the shell's code expects it.
pub fn skel_for(compositor_id: &str, shell_id: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/compositor")
        .join(compositor_id)
        .join("shells")
        .join(shell_id)
        .join("skel")
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

    // The new shell is up, so nothing can roll back to the copies this switch
    // renamed aside: `restore` only runs from the failure paths above. They are
    // dropped here rather than left behind, which is what a day of switching
    // used to do — one generation per attempt, and then the next backup copied
    // every one of them along with the user's config.
    let (dropped, held) = discard_asides(&asides, backups_root);
    if dropped > 0 {
        progress(&format!(
            "removed {dropped} of the copies this switch set aside; each is still in {}",
            backup_root.display()
        ));
    }
    if held > 0 {
        progress(&format!(
            "kept {held} of the copies set aside, which no backup holds"
        ));
    }

    // The shell just left goes, now that nothing can roll back to it: the new
    // one is up, and the backup written before the switch is the way home. Both
    // steps are reported rather than fatal — a switch that has reached here has
    // succeeded, and a file or a package that would not go away leaves the
    // machine dirtier than asked, not broken.
    if let Some(outgoing) = current {
        let (cleaned, held) = clean(outgoing, target, home, backup_root, progress);
        if cleaned > 0 {
            progress(&format!(
                "cleaned {cleaned} path(s) belonging to {}",
                outgoing.display_name
            ));
        }
        if held > 0 {
            progress(&format!("kept {held} path(s) of {}", outgoing.display_name));
        }

        if let Some(plan) = outgoing.uninstall_plan {
            progress(&format!("removing the {} packages", outgoing.display_name));
            match crate::switch::uninstall(plan, progress) {
                crate::switch::Removed::Finished { stages, .. } => progress(&format!(
                    "{}'s packages are off the machine ({stages} stage(s))",
                    outgoing.display_name
                )),
                crate::switch::Removed::AlreadyGone { .. } => {
                    progress("its packages were already gone");
                }
                crate::switch::Removed::Dismissed { stage } => progress(&format!(
                    "stage {stage} was not authorized, so {} stays installed",
                    outgoing.display_name
                )),
                crate::switch::Removed::Failed { stage, status, .. } => {
                    let line = format!(
                        "stage {stage} could not remove {}: {status}",
                        outgoing.display_name
                    );
                    crate::logging::warn(&line);
                    progress(&line);
                }
                crate::switch::Removed::Malformed { package } => {
                    let line = format!(
                        "the removal plan for {package} names no packages, so nothing was removed"
                    );
                    crate::logging::error(&line);
                    progress(&line);
                }
            }
        }
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
        if let Some(entry) = pull(
            rel,
            backups_root,
            skel,
            false,
            &format!("{} will start on its own defaults", target.display_name),
            progress,
        ) {
            entries.push(entry);
        }
    }

    // State is the same question as configuration, and it has to be asked the
    // same way: a switch that removes the outgoing shell's state directory on
    // its way out can only do that because the backup is about to be the way
    // back. Without this the state would be saved and never read — which is
    // exactly what happened while nothing deleted it, since the files simply
    // stayed on disk.
    if let Some(rel) = target.state_dir {
        if let Some(entry) = pull(
            rel,
            backups_root,
            skel,
            true,
            &format!(
                "no backup or skel holds {rel}; {} starts with no state carried over",
                target.display_name
            ),
            progress,
        ) {
            entries.push(entry);
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

/// Where one of a shell's own paths comes from.
///
/// Newest backup first and the skel as the fallback, which is the right answer
/// for anything belonging to a shell rather than to the session: if this machine
/// has run that shell before, the backup holds how the user had it arranged, and
/// that is what should come back. The compositor's configuration is the one
/// exception and is handled by name in [`plan`] instead.
///
/// `allow_file` is for paths whose skel copy may be a single file rather than a
/// directory; a state path is usually a directory and a configuration path
/// always is, so the strict `is_dir` check stands for the latter. `empty` is
/// what to say when neither holds the path — a legitimate outcome, never fatal
/// here, because the fatal case is the compositor's own configuration below.
fn pull(
    rel: &str,
    backups_root: &Path,
    skel: &Path,
    allow_file: bool,
    empty: &str,
    progress: &mut dyn FnMut(&str),
) -> Option<Entry> {
    let rel = rel.trim_start_matches("./").to_string();
    match newest_backup_holding(backups_root, &rel) {
        Some(root) => {
            let src = root.join("home").join(&rel);
            progress(&format!("{rel} comes from the newest backup that holds it"));
            Some(Entry {
                rel,
                src,
                origin: Origin::Backup(root),
            })
        }
        None if skel.join(&rel).is_dir() || (allow_file && skel.join(&rel).is_file()) => {
            progress(&format!(
                "{rel} comes from the skel, not a backup; no backup of yours holds it"
            ));
            Some(Entry {
                rel: rel.clone(),
                src: skel.join(&rel),
                origin: Origin::Skel,
            })
        }
        None => {
            progress(&format!("no backup or skel holds {rel}; {empty}"));
            None
        }
    }
}

/// The newest backup that actually holds `rel`, if any.
///
/// Backup directories are named after the day they were taken, so sorting by
/// name is sorting by age.
fn newest_backup_holding(backups_root: &Path, rel: &str) -> Option<PathBuf> {
    backup_holding(backups_root, rel)
}

/// The newest backup holding `rel`, whether `rel` is a directory or a file.
///
/// Hidden directories are skipped: a copy still being built, or a previous one
/// left aside by an interrupted replace, is not a backup anything should be
/// sourcing files from. `exists` rather than `is_dir` because a shell's own
/// declaration may name a single file, and because an aside can belong to one.
fn backup_holding(backups_root: &Path, rel: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(backups_root)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter(|p| {
            !p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        })
        .collect();
    dirs.sort();
    dirs.reverse();
    dirs.into_iter().find(|d| d.join("home").join(rel).exists())
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

/// Throw away the rollback copies a finished switch renamed aside.
///
/// They exist for exactly one thing: the rollback that a failed `start` or
/// `verify` would have triggered. Both have succeeded by the time this runs, so
/// no path in the code can read them again — undoing the switch later reads the
/// backup instead. Leaving them on disk costs a file per path per switch, and
/// because most of them live inside a directory that is itself backed up, each
/// generation was being copied into the next backup too.
///
/// "The backup already holds this" is checked, never assumed: the aside is only
/// dropped when some backup contains that exact path with the same number of
/// files and the same number of bytes. An aside no backup holds is kept, because
/// after the swap it may be the only copy left of what was there.
///
/// Returns how many were removed and how many were kept.
fn discard_asides(asides: &[Aside], backups_root: &Path) -> (usize, usize) {
    let mut dropped = 0;
    let mut held = 0;
    for aside in asides {
        let Some(from) = &aside.kept else {
            continue;
        };
        if !from.exists() {
            continue;
        }

        let identical = backup_holding(backups_root, &aside.rel)
            .map(|root| measure(from) == measure(&root.join("home").join(&aside.rel)))
            .unwrap_or(false);
        if !identical {
            crate::logging::info(&format!(
                "kept {}, which no backup reproduces byte for byte",
                from.display()
            ));
            held += 1;
            continue;
        }

        let removed = if from.is_dir() && !from.is_symlink() {
            std::fs::remove_dir_all(from)
        } else {
            std::fs::remove_file(from)
        };
        match removed {
            Ok(()) => dropped += 1,
            Err(err) => {
                crate::logging::error(&format!(
                    "cannot remove {} after a successful switch: {err}",
                    from.display()
                ));
                held += 1;
            }
        }
    }
    (dropped, held)
}

/// Take the outgoing shell's own files off the machine.
///
/// Runs only at the tail of a switch: the backup is written, read back and
/// re-checked, and the new shell has already proved itself alive. Nothing here
/// is allowed to delete on trust. A path goes only when the backup taken for
/// this very switch reproduces it byte for byte — the same test
/// [`discard_asides`] applies to the copies it drops, for the same reason: a
/// backup that no longer describes what is on disk is not a way back, it is a
/// reason to leave the file alone.
///
/// A mismatch is expected rather than exceptional. The shell was still running
/// when the backup was taken and was stopped a moment later, so it can easily
/// have written something in between. The file is then kept and said out loud,
/// which is the honest outcome: cleaner than it was, and not a delete on a
/// backup that has gone stale.
fn clean(
    outgoing: &Shell,
    target: &Shell,
    home: &Path,
    backup_root: &Path,
    progress: &mut dyn FnMut(&str),
) -> (usize, usize) {
    let mut removed = 0;
    let mut kept = 0;

    for rel in [outgoing.config_dir, outgoing.state_dir]
        .into_iter()
        .flatten()
    {
        // Cheap, and the only thing standing between two shells and one of them
        // deleting the other's live configuration if a future registry ever gave
        // them the same path.
        if target.config_dir == Some(rel) || target.state_dir == Some(rel) {
            crate::logging::warn(&format!(
                "not cleaning {rel}: {} owns the same path",
                target.display_name
            ));
            kept += 1;
            continue;
        }

        let live = home.join(rel);
        if !live.exists() {
            continue;
        }

        let copy = backup_root.join("home").join(rel);
        if !copy.exists() || measure(&live) != measure(&copy) {
            let line = format!(
                "kept {rel}: the backup taken for this switch does not hold it byte for byte"
            );
            crate::logging::info(&line);
            progress(&line);
            kept += 1;
            continue;
        }

        let result = if live.is_dir() && !live.is_symlink() {
            std::fs::remove_dir_all(&live)
        } else {
            std::fs::remove_file(&live)
        };
        match result {
            Ok(()) => {
                crate::logging::info(&format!("removed {rel} after a successful switch"));
                progress(&format!(
                    "removed {rel}; it is still in {}",
                    backup_root.display()
                ));
                removed += 1;
            }
            Err(err) => {
                let line = format!("cannot remove {rel} after a successful switch: {err}");
                crate::logging::error(&line);
                progress(&line);
                kept += 1;
            }
        }
    }

    (removed, kept)
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

    // Which branch to take comes from `mechanism`, not from the presence of a unit
    // name. They agree today, and a_mechanism_agrees_with_its_unit_name keeps
    // them honest, but the enum is the one that also knows about the compositor
    // launchers — where there is no unit because the compositor's own config is
    // what runs the shell. Reading the unit instead would make those shells look
    // like direct processes, which is the same answer for the wrong reason.
    if shell.mechanism.uses_systemd_unit() {
        let unit = shell.systemd_user_unit.ok_or_else(|| {
            (
                "start",
                format!("{} claims a systemd unit but names none", shell.id),
            )
        })?;
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

    // Niri waits for a `spawn-at-startup` command to return before it continues,
    // so a shell started that way has to detach or the session stalls behind it.
    // Hyprland's `exec_on` does not wait, and leaving the flag off there keeps the
    // process a child this program can account for.
    let mut argv: Vec<&str> = shell.start_argv.to_vec();
    if shell.mechanism.needs_detached_start() {
        argv.push("--daemon");
    }

    progress(&format!(
        "starting {} ({})",
        shell.display_name,
        argv.join(" ")
    ));
    Command::new(argv[0])
        .args(&argv[1..])
        .spawn()
        .map_err(|e| ("start", format!("cannot run {}: {e}", argv.join(" "))))?;
    Ok(())
}

/// Check the new shell is really up, not merely launched.
fn verify(
    shell: &Shell,
    bus_owner_before: Option<u32>,
    progress: &mut dyn FnMut(&str),
) -> Result<(), (&'static str, String)> {
    let deadline = Instant::now() + START_GRACE;

    if shell.mechanism.uses_systemd_unit() {
        // Asking systemd whether the unit is active is a stronger answer than
        // looking for a process: a unit can be active while the shell inside it is
        // still failing to draw, and a process can be alive while the unit that
        // should supervise it is not. See `a_mechanism_agrees_with_its_unit_name`
        // for why the unit name is guaranteed to be there here.
        let unit = shell.systemd_user_unit.ok_or_else(|| {
            (
                "verify",
                format!("{} claims a systemd unit but names none", shell.id),
            )
        })?;
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
    //
    // Directly spawned shells (like noctalia) take a moment to claim the bus, so
    // "nobody owns it yet" is treated as transient while we are still inside the
    // grace period, rather than an immediate failure.
    loop {
        match (bus_owner_before, notification_owner()) {
            (_, None) => {
                if Instant::now() >= deadline {
                    return Err((
                        "verify",
                        format!(
                            "nobody owns {NOTIFICATION_BUS} by the end of the grace period; the new shell never claimed the notification bus"
                        ),
                    ));
                }
                std::thread::sleep(Duration::from_millis(500));
                continue;
            }
            (Some(before), Some(after)) if before == after => {
                return Err((
                    "verify",
                    format!(
                        "{NOTIFICATION_BUS} is still owned by pid {after}, the process that owned it before, so the old shell is still serving notifications"
                    ),
                ));
            }
            (_, Some(after)) => {
                progress(&format!(
                    "{NOTIFICATION_BUS} now belongs to pid {after}, not to the one it had before"
                ));
                break;
            }
        }
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
#[path = "tests/apply.rs"]
mod tests;
