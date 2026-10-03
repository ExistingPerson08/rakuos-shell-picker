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
struct Entry {
    /// Path relative to `$HOME`, e.g. `.config/hypr`.
    rel: String,
    src: PathBuf,
    origin: Origin,
}

/// A path that was renamed aside, so a rollback knows where to find it.
struct Aside {
    /// Where it used to live, relative to `$HOME`.
    rel: String,
    /// Where it was moved to, absolute.
    kept: PathBuf,
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
            let rolled_back = restore(&asides, home);
            let _ = std::fs::remove_dir_all(&staging);
            return Apply::Failed {
                step: "swap",
                detail,
                rolled_back,
            };
        }
    };
    let _ = std::fs::remove_dir_all(&staging);

    if let Err((step, detail)) = start(target, progress).and_then(|()| verify(target, progress)) {
        progress(&format!("{step} did not work: {detail}"));
        progress("putting the previous configuration back");
        let rolled_back = restore(&asides, home) && resume(current, progress);
        return Apply::Failed {
            step,
            detail,
            rolled_back,
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
        let (src, origin) = match newest_backup_holding(backups_root, &rel) {
            Some(root) => (root.join("home").join(&rel), Origin::Backup(root)),
            None => (skel.join(&rel), Origin::Skel),
        };
        if !src.is_dir() {
            return Err(format!(
                "no configuration for {}: neither a backup nor {} holds {rel}",
                target.display_name,
                skel.display()
            ));
        }
        progress(&format!(
            "{rel} comes from the {} ({})",
            match &origin {
                Origin::Skel => "skel".to_string(),
                Origin::Backup(_) => "newest backup".to_string(),
            },
            origin.label()
        ));
        entries.push(Entry { rel, src, origin });
    }

    // The compositor's configuration decides how the shell gets started, so it
    // is taken from the target's skel and never from a backup. See above.
    let comp_rel = compositor.config_dir().trim_start_matches("./").to_string();
    let comp_src = skel.join(&comp_rel);
    if comp_src.is_dir() {
        progress(&format!("{comp_rel} comes from the skel, not a backup"));
        entries.push(Entry {
            rel: comp_rel,
            src: comp_src,
            origin: Origin::Skel,
        });
    } else {
        progress(&format!(
            "the skel has no {comp_rel}, leaving your compositor configuration alone"
        ));
    }

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

        let aside = if to.exists() {
            let kept = parent.join(format!(
                ".{}.pre-{target_id}-{stamp}",
                entry.rel.rsplit('/').next().unwrap_or(&entry.rel)
            ));
            if let Err(e) = std::fs::rename(&to, &kept) {
                return Err((format!("cannot move your {} aside: {e}", entry.rel), asides));
            }
            Some(Aside {
                rel: entry.rel.clone(),
                kept,
            })
        } else {
            None
        };

        if let Err(e) = std::fs::rename(&from, &to) {
            // Put the original back before reporting, so the caller is not left
            // having to reason about a half-finished swap.
            if let Some(a) = &aside {
                let _ = std::fs::rename(&a.kept, home.join(&a.rel));
            }
            return Err((format!("cannot put {} in place: {e}", entry.rel), asides));
        }

        if let Some(a) = aside {
            progress(&format!(
                "{} replaced from the {}; the old one is at {}",
                entry.rel,
                entry.origin.label(),
                a.kept.display()
            ));
            asides.push(a);
        } else {
            progress(&format!(
                "{} created from the {}",
                entry.rel,
                entry.origin.label()
            ));
        }
    }

    Ok(asides)
}

/// Put the renamed-aside paths back.
fn restore(asides: &[Aside], home: &Path) -> bool {
    let mut ok = true;
    for aside in asides {
        let to = home.join(&aside.rel);
        if to.exists() {
            let _ = std::fs::remove_dir_all(&to);
        }
        match std::fs::rename(&aside.kept, &to) {
            Ok(()) => crate::logging::info(&format!("restored {}", aside.rel)),
            Err(e) => {
                crate::logging::error(&format!("cannot restore {}: {e}", aside.rel));
                ok = false;
            }
        }
    }
    ok
}

/// Start a shell the way it says it starts.
fn start(shell: &Shell, progress: &mut dyn FnMut(&str)) -> Result<(), (&'static str, String)> {
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
fn verify(shell: &Shell, progress: &mut dyn FnMut(&str)) -> Result<(), (&'static str, String)> {
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
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err((
                "verify",
                format!("{} never appeared as a process", shell.binary),
            ));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
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
    let out = run("systemctl", &["--user", "logout"])
        .map_err(|e| format!("cannot run systemctl --user logout: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "systemctl --user logout said: {}",
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
        let _ = run("kill", &[sig, &pid.to_string()]);
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
