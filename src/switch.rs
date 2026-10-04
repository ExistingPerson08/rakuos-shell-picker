//! The switch, in the order `docs/SAFETY.md` fixes.
//!
//! Only the first steps live here so far: deciding what a switch would touch and
//! copying it. Nothing in this module stops or starts anything.
//!
//! The order the user settled on is backup, then dependencies, then
//! `pkexec rum install`, and it is deliberate. A backup is read-only, so doing
//! it first costs nothing and means the switch can refuse later without having
//! changed anything. Installing is the first step that writes to the system, so
//! it goes after everything that can still say no.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::compositor::Compositor;
use crate::shell::{InstallPlan, Shell};

/// Paths a switch disturbs no matter which shell is running.
///
/// The toolkit settings decide how every GTK and Qt program on the desktop
/// looks. They belong to the session rather than to a shell, which is why they
/// are listed here instead of coming from a registry entry.
///
/// Nothing else belongs in this list. `uwsm` used to be here on the assumption
/// that it decided how the new shell gets started. It does not: UWSM launches
/// the compositor and then leaves. What launches the shell is the compositor's
/// own autostart — for Hyprland that is `hl.exec_cmd("noctalia")` in
/// `~/.config/hypr/config/autostart.lua`.
///
/// And `~/.config/uwsm` is not per desktop anyway. The per-desktop half of
/// UWSM lives in the system's `/usr/share/uwsm/plugins/<desktop>.sh`, which
/// supplies the session's finalization variables; the user half is a single
/// global `env` script shared by every session. A fragment written there would
/// have been identical for every compositor, so it would not have been about
/// switching shells at all, only about risking a file other desktops read.
///
/// The compositor's own configuration is not in this list either: its path
/// follows the compositor, so it comes from [`Compositor::config_dir`] instead.
/// See [`backup_sources`].
pub const SHARED_PATHS: &[&str] = &[
    ".config/gtk-3.0",
    ".config/gtk-4.0",
    ".config/qt5ct",
    ".config/qt6ct",
];

/// Everything to copy before switching away from `current`.
///
/// Shared paths first, then the outgoing shell's own config, then its state, then
/// whatever else that shell's own declaration says a switch will overwrite.
/// Paths that do not exist stay in the list on purpose: the backup records them
/// as absent, which is what later lets a restore tell "was never there" apart
/// from "was lost".
///
/// The last group is per shell and comes from
/// [`Shell::backup_extra_paths`](crate::shell::Shell::backup_extra_paths), because
/// what a switch destroys is not the same for every shell: Noctalia's launcher
/// entry and DMS's runtime state are written by a switch and belong to nobody
/// else on the desktop, so nothing else would have saved them. Deriving this list
/// from `skel_extra_paths` instead would be wrong in the other direction — a file
/// being installed is not the same question as a file needing to be kept.
///
/// `home` is passed in rather than read from the environment so the list can be
/// checked without depending on whose session is running.
pub fn backup_sources(
    home: &Path,
    compositor: &dyn Compositor,
    current: Option<&'static Shell>,
) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = SHARED_PATHS.iter().map(|p| home.join(p)).collect();
    // The compositor's config is saved on every switch even though a switch only
    // ever writes fragments into it, because those fragments are edits to files
    // the user also wrote by hand.
    paths.push(home.join(compositor.config_dir()));
    if let Some(shell) = current {
        for relative in [shell.config_dir, shell.state_dir]
            .into_iter()
            .flatten()
            .chain(shell.backup_extra_paths.iter().copied())
        {
            paths.push(home.join(relative));
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compositor::hyprland::shells::{
        caelestia::SHELL as CAELESTIA, dms::SHELL as DMS, noctalia::SHELL as NOCTALIA,
    };

    fn home() -> PathBuf {
        PathBuf::from("/home/tester")
    }

    /// The Hyprland list, which is the one actually exercised today.
    fn relative_list(shell: Option<&'static Shell>) -> Vec<String> {
        backup_sources(&home(), &crate::compositor::HYPRLAND, shell)
            .iter()
            .map(|p| {
                p.strip_prefix(home())
                    .expect("every backup path is inside home")
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    #[test]
    fn switching_away_from_noctalia_backs_up_exactly_these_paths() {
        assert_eq!(
            relative_list(Some(&NOCTALIA)),
            [
                // shared: toolkit settings, which follow nothing
                ".config/gtk-3.0",
                ".config/gtk-4.0",
                ".config/qt5ct",
                ".config/qt6ct",
                // the compositor's own config, which follows the compositor
                ".config/hypr",
                // the outgoing shell's own config and its state
                ".config/noctalia",
                ".local/state/noctalia",
                // and its launcher entry, which DMS's skel overwrites on the way
                // out. This one was missing until the backup set was declared per
                // shell rather than derived: a switch destroyed it with no copy.
                ".local/share/applications/dev.noctalia.Noctalia.desktop",
            ]
        );
    }

    #[test]
    fn switching_away_from_dms_backs_up_what_dms_would_lose() {
        // DMS is the case the shared derivation could not express. Its state
        // directory is not `state_dir` — it arrives through the skel as an extra
        // path — and the launcher it overwrites belongs to the shell being left,
        // not to DMS. Both are destroyed by a switch, so both have to be saved.
        assert_eq!(
            relative_list(Some(&DMS)),
            [
                ".config/gtk-3.0",
                ".config/gtk-4.0",
                ".config/qt5ct",
                ".config/qt6ct",
                ".config/hypr",
                ".config/DankMaterialShell",
                ".local/state/DankMaterialShell",
                ".local/share/applications/dev.noctalia.Noctalia.desktop",
            ]
        );
    }

    #[test]
    fn caelestia_needs_no_backup_of_its_own() {
        // Its config directory is the whole of it: the skel installs nothing else
        // on its behalf, so there is nothing else to save. Declaring a path that is
        // not installed would be harmless but misleading, and this pins the reason.
        assert_eq!(
            relative_list(Some(&CAELESTIA)),
            [
                ".config/gtk-3.0",
                ".config/gtk-4.0",
                ".config/qt5ct",
                ".config/qt6ct",
                ".config/hypr",
                ".config/quickshell/caelestia",
            ]
        );
        assert!(CAELESTIA.skel_extra_paths.is_empty());
        assert!(CAELESTIA.backup_extra_paths.is_empty());
    }

    #[test]
    fn a_switch_with_nothing_running_still_saves_the_shared_paths() {
        // The first switch has no outgoing shell, but the toolkit settings are
        // still about to be touched.
        assert_eq!(
            relative_list(None),
            [
                ".config/gtk-3.0",
                ".config/gtk-4.0",
                ".config/qt5ct",
                ".config/qt6ct",
                ".config/hypr",
            ]
        );
    }

    #[test]
    fn a_shell_with_no_recorded_directories_adds_nothing() {
        // Ambxst records neither, so switching away from it cannot promise to
        // bring anything back. Its preflight refuses it for the same reason.
        let ambxst = crate::shell::find("ambxst").expect("ambxst registered");
        assert_eq!(relative_list(Some(ambxst)), relative_list(None));
    }

    #[test]
    fn every_shared_path_is_relative_and_cannot_climb_out() {
        for path in SHARED_PATHS {
            assert!(!path.starts_with('/'), "{path} is absolute");
            assert!(
                !path.split('/').any(|part| part == ".."),
                "{path} climbs out of home"
            );
        }
    }

    #[test]
    fn no_path_is_listed_twice() {
        // A duplicate would mean copying the same tree twice under two keys,
        // and the second copy is the one a restore would find.
        let mut seen = relative_list(Some(&NOCTALIA));
        let before = seen.len();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), before, "duplicate in the backup list");
    }

    #[test]
    fn the_shell_being_left_behind_is_the_one_that_gets_backed_up() {
        // Only the outgoing shell's directories. DMS's config is not backed up
        // while Noctalia is running, because nothing is touching it yet.
        let dms_only = relative_list(Some(&DMS));
        assert!(dms_only.contains(&".config/DankMaterialShell".to_string()));
        assert!(!dms_only.contains(&".config/noctalia".to_string()));

        let caelestia_only = relative_list(Some(&CAELESTIA));
        assert!(caelestia_only.contains(&".config/quickshell/caelestia".to_string()));
        assert!(!caelestia_only.contains(&".config/DankMaterialShell".to_string()));
    }
}

use crate::backup::Backup;

/// Why a switch may not go any further.
///
/// These are the step 1 and step 2 refusals. Each one has to name what went
/// wrong, because a refusal the user cannot act on is indistinguishable from a
/// broken app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The compositor moved, or was never identified.
    Compositor {
        target: String,
        current: Option<String>,
    },
    /// The shell asked for is the one already on screen.
    AlreadyRunning { shell: String },
    /// Nothing to switch to and nothing to install from.
    Unavailable { shell: String },
    /// The backup could not be taken or could not be read back.
    BackupFailed { reason: String },
}

impl Refusal {
    pub fn message(&self) -> String {
        match self {
            Refusal::Compositor {
                target,
                current: Some(current),
            } => format!("{target} is not the running compositor; {current} is"),
            Refusal::Compositor { current: None, .. } => {
                "the running compositor could not be identified".to_string()
            }
            Refusal::AlreadyRunning { shell } => format!("{shell} is already running"),
            Refusal::Unavailable { shell } => {
                format!("{shell} is not installed and has no package to install it from")
            }
            Refusal::BackupFailed { reason } => {
                format!("backup failed, nothing was changed: {reason}")
            }
        }
    }
}

/// What the first three steps produced.
#[derive(Debug)]
pub struct Prepared {
    pub from: Option<&'static Shell>,
    pub to: &'static Shell,
    pub backup: Backup,
}

/// Work out whether a switch may proceed, and back up what it would change.
///
/// Refuses first, before touching anything: a refusal costs the user one line,
/// whereas finding out afterwards costs them a half-finished switch. `docs/SAFETY.md`
/// is the contract this implements.
///
/// Which shells are running is passed in rather than looked up. The caller has
/// already had to work that out to draw the window, and a function that quietly
/// reads the process table behind its caller's back cannot be tested without
/// arranging for a particular shell to be running on the machine.
/// order; this is where that order currently stops.
///
/// `compositor_id` and `current` come from detection, which the caller re-reads
/// rather than trusting the value it started with.
pub fn prepare(
    compositor: &'static dyn Compositor,
    current: Option<&str>,
    target: &'static Shell,
    running: Option<&'static Shell>,
    target_running: bool,
    home: &Path,
    backups_root: &Path,
) -> Result<Prepared, Refusal> {
    let compositor_id = compositor.id();
    // Step 1: refuse.
    match current {
        Some(id) if id != compositor_id => {
            return Err(Refusal::Compositor {
                target: compositor_id.to_string(),
                current: Some(id.to_string()),
            })
        }
        None => {
            return Err(Refusal::Compositor {
                target: compositor_id.to_string(),
                current: None,
            })
        }
        _ => {}
    }
    if target_running {
        return Err(Refusal::AlreadyRunning {
            shell: target.display_name.to_string(),
        });
    }

    // Step 2: preflight. A shell with no package and nothing on disk has no way
    // to exist; refusing here means the backup is never taken for a target that
    // could not be started anyway.
    if target.package.is_none() && !target.binary_present() {
        return Err(Refusal::Unavailable {
            shell: target.display_name.to_string(),
        });
    }

    // Step 3: back up. Only the shell being left behind contributes its own
    // paths; the shared ones are saved on every switch.
    let stamp = timestamp();
    let name = format!(
        "{stamp}-{compositor_id}-{}",
        running.map(|s| s.id).unwrap_or("none")
    );
    let sources = backup_sources(home, compositor, running);
    let backup = Backup::create(backups_root, &name, &sources, home).map_err(|err| {
        Refusal::BackupFailed {
            reason: format!("{err:#}"),
        }
    })?;

    Ok(Prepared {
        from: running,
        to: target,
        backup,
    })
}

/// What the install step actually did.
///
/// Split out as an enum because "you dismissed the polkit prompt" and "the
/// transaction failed" are different problems for the user, and collapsing them
/// into one error would tell them to look in the wrong place. The stage number
/// is carried on every failure because the plan is not one transaction: knowing
/// that stage 1 failed says something very different from stage 2 failing after
/// stage 1 already succeeded.
#[derive(Debug, PartialEq)]
pub enum Install {
    /// Every stage ran and succeeded.
    Finished { stages: usize, output: String },
    /// Every package in the plan was already installed, so nothing ran.
    AlreadyThere { packages: Vec<&'static str> },
    /// The polkit prompt was dismissed. Nothing beyond earlier stages changed.
    Dismissed { stage: usize },
    /// A stage ran and failed. Later stages were not attempted.
    Failed {
        stage: usize,
        status: String,
        output: String,
    },
}

/// Install the plan's packages one stage at a time, asking polkit each time.
///
/// There is deliberately no dependency pre-flight here. `rum install` resolves
/// the transaction itself: packages that are already present are skipped and the
/// rest are pulled in, so a separate check would repeat that work, add seconds
/// to the click, and read metadata that can go stale between the check and the
/// install. Where a provider had to be chosen, the choice is made by naming the
/// package on the command line instead — see [`InstallPlan::stages`].
///
/// A stage that fails stops the plan. Later stages are not attempted, so a
/// failure in the first one leaves nothing of the shell itself installed.
///
/// `progress` is called with the package manager's own output as it arrives,
/// not after the command returns. Without it the panel shows nothing at all for
/// the length of the download, and a working install is indistinguishable from a
/// hung one.
pub fn install(plan: InstallPlan, progress: &mut dyn FnMut(&str)) -> Install {
    if plan
        .packages()
        .iter()
        .all(|p| crate::shell::package_origin(p).installed())
    {
        return Install::AlreadyThere {
            packages: plan.packages(),
        };
    }

    let mut output = String::new();
    let mut ran = 0usize;

    for (index, stage) in plan.stages.iter().enumerate() {
        let stage = *stage;
        let number = index + 1;
        let missing: Vec<&str> = stage
            .iter()
            .copied()
            .filter(|p| !crate::shell::package_origin(p).installed())
            .collect();

        if missing.is_empty() {
            progress(&format!(
                "stage {number} of {}: {} already installed, skipping",
                plan.stages.len(),
                stage.join(", ")
            ));
            continue;
        }

        let line = plan.stage_command_line(&missing);
        crate::logging::info(&format!("stage {number} of {}: {line}", plan.stages.len()));
        progress(&format!("stage {number} of {}: {line}", plan.stages.len()));
        progress(&format!(
            "authorizing stage {number}; a password prompt should appear"
        ));

        let mut command = Command::new(plan.program);
        command.args(plan.args).args(&missing);
        command.env("LANG", "C");

        let (status, text) = match run_streaming(&mut command, &mut *progress) {
            Ok(result) => result,
            Err(error) => {
                progress(&format!("could not start the installer: {error}"));
                return Install::Failed {
                    stage: number,
                    status: format!("could not run {}", plan.program),
                    output: error.to_string(),
                };
            }
        };

        if !output.is_empty() && !text.is_empty() {
            output.push('\n');
        }
        output.push_str(&text);

        if status.success() {
            ran += 1;
            progress(&format!("stage {number} finished"));
            continue;
        }
        if looks_dismissed(status.code(), &text) {
            crate::logging::warn("the polkit prompt was dismissed");
            progress(&format!("stage {number} was not authorized"));
            return Install::Dismissed { stage: number };
        }
        crate::logging::error(&format!(
            "stage {number} of {} failed: {status}",
            plan.stages.len()
        ));
        return Install::Failed {
            stage: number,
            status: status.to_string(),
            output: text,
        };
    }

    Install::Finished {
        stages: ran,
        output,
    }
}

/// How often to say that something is still happening when the installer is
/// quiet. Long enough not to spam, short enough that silence never reads as a
/// hang.
const HEARTBEAT_SECS: u64 = 10;

/// Run `command`, forwarding its output to `progress` as it arrives.
///
/// `Command::output` would be simpler but it reads to the end, so nothing reaches
/// the panel until the whole transaction is done. Two reader threads and a
/// heartbeat all push into one channel, and the caller's callback is only ever
/// touched from this thread, which keeps the panel single-threaded.
fn run_streaming(
    command: &mut Command,
    progress: &mut dyn FnMut(&str),
) -> std::io::Result<(std::process::ExitStatus, String)> {
    use std::io::{BufRead, BufReader};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    let mut child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let (tx, rx) = mpsc::channel::<String>();
    let done = std::sync::Arc::new(AtomicBool::new(false));

    let spawn_reader = |stream: Box<dyn std::io::Read + Send>| {
        let tx = tx.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    return;
                }
            }
        });
    };
    if let Some(out) = child.stdout.take() {
        spawn_reader(Box::new(out));
    }
    if let Some(err) = child.stderr.take() {
        spawn_reader(Box::new(err));
    }
    // Says "still here" on a timer, because a download can go quiet for a while
    // and dnf only prints when it has something new to say.
    let beat = {
        let done = done.clone();
        let tx = tx.clone();
        std::thread::spawn(move || {
            let started = Instant::now();
            while !done.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_secs(HEARTBEAT_SECS));
                if done.load(Ordering::Relaxed) {
                    return;
                }
                if tx
                    .send(format!(
                        "  still installing ({}s)",
                        started.elapsed().as_secs()
                    ))
                    .is_err()
                {
                    return;
                }
            }
        })
    };

    drop(tx);

    let mut text = String::new();
    let mut silence = 0u32;
    let status = loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(line) => {
                silence = 0;
                if line.trim().is_empty() {
                    continue;
                }
                progress(&line);
                text.push_str(&line);
                text.push('\n');
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                silence += 1;
                if let Some(status) = child.try_wait()? {
                    break status;
                }
                // Ten seconds of nothing from the installer is worth mentioning
                // even when the heartbeat has not fired yet.
                if silence as u64 * 500 >= HEARTBEAT_SECS * 1000 {
                    silence = 0;
                    progress("  still installing");
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break child.wait()?;
            }
        }
    };

    done.store(true, Ordering::Relaxed);
    let _ = beat.join();
    while let Ok(line) = rx.try_recv() {
        progress(&line);
        text.push_str(&line);
        text.push('\n');
    }

    Ok((status, text))
}

/// Did this exit mean "the user said no", rather than "it broke"?
///
/// Checked against the status because that is the only part of pkexec's answer
/// that is reliable: the message it prints differs between agents, and a
/// cancelled prompt has to be told apart from a failed transaction so the user
/// is not sent looking for a broken package manager.
fn looks_dismissed(code: Option<i32>, output: &str) -> bool {
    if matches!(code, Some(126) | Some(127)) {
        return true;
    }
    let lowered = output.to_lowercase();
    lowered.contains("not authorized")
        || lowered.contains("authentication is required")
        || lowered.contains("dismissed")
}

#[cfg(test)]
mod install_tests {
    use super::*;

    #[test]
    fn a_cancelled_polkit_prompt_is_not_reported_as_a_broken_package_manager() {
        // pkexec's documented exit for a denied or cancelled request.
        assert!(looks_dismissed(Some(126), ""));
        assert!(looks_dismissed(Some(127), ""));
        assert!(looks_dismissed(Some(1), "Error: not authorized"));
        assert!(looks_dismissed(
            Some(1),
            "Authentication is required to run 'rum'"
        ));
    }

    #[test]
    fn a_failed_transaction_is_not_mistaken_for_a_refusal() {
        assert!(!looks_dismissed(Some(1), "No match for argument: dms"));
        assert!(!looks_dismissed(Some(1), "Error: GPG check FAILED"));
        assert!(!looks_dismissed(None, "killed by signal"));
    }

    #[test]
    fn the_logged_command_names_every_package_the_plan_stages() {
        let plan = crate::compositor::hyprland::shells::dms::SHELL
            .install_plan
            .unwrap();
        assert_eq!(
            plan.packages(),
            vec![
                "quickshell-git",
                "dms",
                "dms-cli",
                "dgop",
                "danksearch",
                "dankcalendar-git",
                "matugen",
            ]
        );
        // Stage 1 is the runtime alone, so a failure there leaves nothing of dms
        // on the machine.
        assert_eq!(
            plan.stage_command_line(&["quickshell-git"]),
            "pkexec rum install -y quickshell-git"
        );
        assert!(!plan.stage_command_line(plan.stages[0]).contains("dms"));
    }
}

/// Backup directory name stamp: `20261003-090000-412`, UTC.
///
/// This runs on the worker thread during a switch, which is why it comes from
/// [`crate::clock`] rather than from GLib. See that module for what happened
/// when it did not.
fn timestamp() -> String {
    crate::clock::stamp_with_millis()
}

#[cfg(test)]
mod prepare_tests {
    use super::*;
    use crate::compositor::hyprland::shells::{
        ambxst::SHELL as AMBXST, dms::SHELL as DMS, noctalia::SHELL as NOCTALIA,
    };

    /// Which shell is alive right now, if any.
    fn running_shell() -> Option<&'static Shell> {
        crate::ui::dialog::running_shell()
    }

    /// The backup list for one compositor, as paths relative to a fake home.
    fn list_for(compositor: &'static dyn Compositor, shell: Option<&'static Shell>) -> Vec<String> {
        backup_sources(&PathBuf::from("/home/tester"), compositor, shell)
            .iter()
            .map(|p| {
                p.strip_prefix("/home/tester")
                    .expect("every backup path is inside home")
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    fn scratch(name: &str) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir()
            .join("rakuos-shell-picker-tests")
            .join("switch")
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("home")).unwrap();
        let home = root.join("home");
        (root, home)
    }

    #[test]
    fn each_compositor_backs_up_its_own_config_and_not_the_other_one() {
        // The compositor's config directory is the one path in the list that
        // follows the compositor. Saving Hyprland's while running Niri would be a
        // backup of a file nothing touched, and missing Niri's would leave the
        // config the switch really edits unprotected.
        let hypr = list_for(&crate::compositor::HYPRLAND, Some(&DMS));
        assert!(
            hypr.contains(&".config/hypr".to_string()),
            "Hyprland list must include its own config: {hypr:?}"
        );
        assert!(
            !hypr.contains(&".config/niri".to_string()),
            "Hyprland list must not mention Niri: {hypr:?}"
        );

        let niri = crate::switch::backup_sources(
            &PathBuf::from("/home/tester"),
            &crate::compositor::NIRI,
            Some(&DMS),
        )
        .iter()
        .map(|p| {
            p.strip_prefix("/home/tester")
                .unwrap()
                .display()
                .to_string()
        })
        .collect::<Vec<_>>();
        assert!(
            niri.contains(&".config/niri".to_string()),
            "Niri list must include its own config: {niri:?}"
        );
        assert!(
            !niri.contains(&".config/hypr".to_string()),
            "Niri list must not mention Hyprland: {niri:?}"
        );
    }

    #[test]
    fn the_compositor_config_is_saved_even_with_no_shell_running() {
        // A switch writes fragments into that file, so it has to be recoverable
        // even when there is no outgoing shell config to save alongside it.
        let only = list_for(&crate::compositor::HYPRLAND, None);
        assert!(only.contains(&".config/hypr".to_string()), "got {only:?}");
    }

    #[test]
    fn the_wrong_compositor_is_refused_before_anything_is_copied() {
        let (root, home) = scratch("wrong-compositor");
        let err = prepare(
            &crate::compositor::NIRI,
            Some("hyprland"),
            &DMS,
            Some(&DMS),
            false,
            &home,
            &root.join("backups"),
        )
        .expect_err("a cross-compositor switch must be refused");

        assert_eq!(
            err,
            Refusal::Compositor {
                target: "niri".to_string(),
                current: Some("hyprland".to_string())
            }
        );
        assert!(
            !root.join("backups").exists(),
            "no backup directory may be created for a refused switch"
        );
    }

    #[test]
    fn an_undetected_compositor_is_refused() {
        let (root, home) = scratch("undetected");
        let err = prepare(
            &crate::compositor::HYPRLAND,
            None,
            &DMS,
            None,
            false,
            &home,
            &root.join("backups"),
        )
        .expect_err("an unidentified compositor must be refused");
        assert_eq!(
            err.message(),
            "the running compositor could not be identified"
        );
        assert!(!root.join("backups").exists());
    }

    #[test]
    fn a_shell_with_nothing_to_install_from_is_refused() {
        let (root, home) = scratch("unavailable");
        let err = prepare(
            &crate::compositor::HYPRLAND,
            Some("hyprland"),
            &AMBXST,
            None,
            false,
            &home,
            &root.join("backups"),
        )
        .expect_err("ambxst has no package and no binary");
        assert!(matches!(err, Refusal::Unavailable { .. }), "{err:?}");
        assert!(!root.join("backups").exists());
    }

    #[test]
    fn a_packaged_shell_is_backed_up_even_though_it_is_not_installed_yet() {
        // DMS is the case that matters: not installed, so nothing exists at
        // ~/.config/DankMaterialShell, and the backup still has to be taken.
        let (root, home) = scratch("dms");
        std::fs::create_dir_all(home.join(".config/noctalia")).unwrap();
        std::fs::write(home.join(".config/noctalia/a"), "a").unwrap();

        let prepared = prepare(
            &crate::compositor::HYPRLAND,
            Some("hyprland"),
            &DMS,
            Some(&NOCTALIA),
            false,
            &home,
            &root.join("backups"),
        )
        .expect("a packaged target is worth switching to");

        assert_eq!(prepared.to.id, "dms");
        // The outgoing shell's config is in there, the target's is not.
        assert!(prepared
            .backup
            .manifest
            .trees
            .contains_key(".config/noctalia"));
        assert!(!prepared
            .backup
            .manifest
            .trees
            .contains_key(".config/DankMaterialShell"));
        // Everything shared is accounted for: copied if it exists, recorded
        // absent if it does not. Neither may be silently missing, because that
        // is the difference between "was never there" and "was forgotten".
        for shared in SHARED_PATHS {
            let key = (*shared).to_string();
            assert!(
                prepared.backup.manifest.trees.contains_key(&key)
                    || prepared.backup.manifest.absent.contains(&key),
                "{shared} was neither copied nor recorded absent"
            );
        }
        assert!(prepared
            .backup
            .manifest
            .absent
            .contains(&".config/hypr".to_string()));
        assert!(prepared.backup.verify().is_ok());
    }

    #[test]
    fn the_backup_directory_is_named_for_the_switch() {
        let (root, home) = scratch("naming");
        let prepared = prepare(
            &crate::compositor::HYPRLAND,
            Some("hyprland"),
            &DMS,
            Some(&NOCTALIA),
            false,
            &home,
            &root.join("backups"),
        )
        .unwrap();

        let name = prepared
            .backup
            .root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(name.starts_with("202"), "{name} should start with a date");
        assert!(name.ends_with("-hyprland-noctalia"), "{name}");
        assert!(prepared.backup.root.join("manifest.toml").is_file());
    }

    #[test]
    fn two_switches_in_a_row_do_not_collide() {
        // Two clicks in quick succession used to land on the same directory name,
        // and the second was refused because the name was taken. That is the
        // worst possible outcome: the backup the user asked for is the one that
        // fails, and the reason on screen ("cannot create backup directory")
        // points at the disk rather than at the clock.
        let (root, home) = scratch("two-switches");
        let mut roots = Vec::new();
        for _ in 0..2 {
            let prepared = prepare(
                &crate::compositor::HYPRLAND,
                Some("hyprland"),
                &DMS,
                Some(&NOCTALIA),
                false,
                &home,
                &root.join("backups"),
            )
            .expect("each switch gets its own backup");
            roots.push(prepared.backup.root.clone());
        }
        assert_ne!(
            roots[0], roots[1],
            "two switches must not share a backup directory"
        );
        // A clash must still never write into the directory that is already there.
        // What changed is how it is resolved: stepping aside instead of refusing.
        // Refusing is exactly what made two switches inside one millisecond fail,
        // and the invariant SAFETY.md rule 2 cares about is the one below, not the
        // refusal — two backups never share a directory, and the one that was
        // already there is left as it was.
        let taken = root.join("backups");
        let first = crate::backup::Backup::create(&taken, "clash", &[], &home)
            .expect("the first use of a name is fine");
        let second = crate::backup::Backup::create(&taken, "clash", &[], &home)
            .expect("a taken name steps aside rather than failing");
        assert_ne!(
            first.root, second.root,
            "a clash must not resolve to a shared directory"
        );
        assert!(
            first.root.join("manifest.toml").is_file(),
            "the backup that was already there must be left intact"
        );
    }

    /// Runs the real prepare() against the real config and the real backup
    /// directory. Ignored so `cargo test` never writes there by accident; run
    /// with `cargo test -- --ignored`.
    ///
    /// It only copies. Nothing is installed, stopped or started, which is the
    /// whole point of where the order currently stops.
    #[test]
    #[ignore = "writes to the real backup directory"]
    fn a_real_switch_to_dms_stops_after_the_backup() {
        let home = PathBuf::from(std::env::var_os("HOME").expect("HOME"));
        let backups = crate::session::State::backups_dir();

        let prepared = prepare(
            &crate::compositor::HYPRLAND,
            Some("hyprland"),
            &DMS,
            running_shell(),
            false,
            &home,
            &backups,
        )
        .expect("prepare should reach the backup");

        println!("from: {:?}", prepared.from.map(|s| s.id));
        println!("to:   {}", prepared.to.id);
        println!("backup directory: {}", prepared.backup.root.display());
        println!("files: {}", prepared.backup.file_count());
        for (key, tree) in &prepared.backup.manifest.trees {
            println!(
                "  copied   {key}: {} files, {} bytes",
                tree.files, tree.bytes
            );
        }
        for key in &prepared.backup.manifest.absent {
            println!("  absent   {key}");
        }
        assert!(prepared.backup.verify().is_ok());
    }

    #[test]
    fn installer_output_arrives_before_the_command_finishes() {
        // The whole point: a user must see the installer working while it works.
        // With `Command::output` this assertion fails, because nothing is reported
        // until the process has already exited.
        let mut command = Command::new("sh");
        command.args(["-c", "echo first; sleep 0.4; echo second"]);
        let mut seen: Vec<String> = Vec::new();
        let (status, text) =
            run_streaming(&mut command, &mut |line| seen.push(line.to_string())).unwrap();

        assert!(status.success(), "sh exited {status}");
        assert_eq!(seen, vec!["first".to_string(), "second".to_string()]);
        assert!(text.contains("first") && text.contains("second"));
    }

    #[test]
    fn a_failing_installer_is_reported_with_its_own_words() {
        let mut command = Command::new("sh");
        command.args(["-c", "echo 'No match for argument: nope' >&2; exit 1"]);
        let mut seen: Vec<String> = Vec::new();
        let (status, text) =
            run_streaming(&mut command, &mut |line| seen.push(line.to_string())).unwrap();

        assert!(!status.success());
        assert!(
            seen.iter().any(|l| l.contains("No match")),
            "stderr must reach the panel, got {seen:?}"
        );
        assert!(text.contains("No match"));
    }

    #[test]
    fn an_installer_that_says_nothing_still_finishes() {
        let mut command = Command::new("sh");
        command.args(["-c", "exit 0"]);
        let mut seen: Vec<String> = Vec::new();
        let (status, _) =
            run_streaming(&mut command, &mut |line| seen.push(line.to_string())).unwrap();
        assert!(status.success());
        assert!(seen.is_empty(), "nothing to report is not an error");
    }
}

/// Where a whole switch reads and writes.
pub struct Roots {
    pub home: std::path::PathBuf,
    pub backups_root: std::path::PathBuf,
}

/// How a whole switch ended.
pub enum Outcome {
    /// It worked. Carries the line to show the user.
    Done(String),
    /// It did not. Carries the line to show the user.
    Failed(String),
}

/// Run a switch from beginning to end: refuse, back up, install, then apply.
///
/// One function, so the window and the command line cannot drift apart. The
/// interesting part is the order: nothing is installed before the backup has been
/// written and verified, and nothing is applied before the packages are there.
/// Everything after that is [`crate::apply`]'s business, including rolling back.
pub fn everything(
    compositor: &'static dyn crate::compositor::Compositor,
    current: Option<&str>,
    target: &'static Shell,
    running: Option<&'static Shell>,
    roots: &Roots,
    log_out: bool,
    progress: &mut dyn FnMut(&str),
) -> Outcome {
    let home = roots.home.as_path();
    let backups_root = roots.backups_root.as_path();

    // The caller's own answer to "is the shell being switched to already on
    // screen", which it needed anyway to draw the window.
    let target_running = running.is_some_and(|s| s.id == target.id);

    let prepared = match prepare(
        compositor,
        current,
        target,
        running,
        target_running,
        home,
        backups_root,
    ) {
        Ok(prepared) => prepared,
        Err(refusal) => return Outcome::Failed(refusal.message()),
    };

    let count = prepared.backup.file_count();
    let bytes: u64 = prepared
        .backup
        .manifest
        .trees
        .values()
        .map(|t| t.bytes)
        .sum();
    let absent = prepared.backup.manifest.absent.len();
    progress(&format!(
        "backed up {count} files ({bytes} bytes, {absent} paths absent) and verified them"
    ));
    progress(&format!("saved in {}", prepared.backup.root.display()));

    // The backup is verified, so the install can be allowed to write. Nothing after
    // this point is reversible by re-copying, which is why the log keeps saying
    // what is about to run.
    let plan = if let Some(plan) = target.install_plan {
        progress(&format!(
            "installing in {} stages: {}",
            plan.stages.len(),
            plan.command_line()
        ));
        Some(plan)
    } else {
        progress(&format!(
            "{} has no install plan, so no packages were touched",
            target.display_name
        ));
        None
    };

    match plan.as_ref().map(|p| install(*p, progress)) {
        Some(Install::Finished { stages, .. }) => {
            progress(&format!("{stages} stage(s) installed"));
        }
        Some(Install::AlreadyThere { packages }) => {
            progress(&format!("already installed: {}", packages.join(", ")));
        }
        Some(Install::Dismissed { stage }) => {
            return Outcome::Failed(format!(
                "the permission prompt for stage {stage} was dismissed. Your backup is still                  in place."
            ));
        }
        Some(Install::Failed {
            stage,
            status,
            output,
        }) => {
            return Outcome::Failed(format!(
                "stage {stage} failed ({status}); later stages were not run. Your backup is                  still in place. rum said: {}",
                output.lines().rev().take(3).collect::<Vec<_>>().join(" / ")
            ));
        }
        None => {}
    }
    let ctx = crate::apply::Context {
        home: roots.home.clone(),
        backups_root: roots.backups_root.clone(),
        skel: crate::apply::skel_for(compositor.id(), target.id),
    };
    match crate::apply::apply_with(
        target,
        running,
        compositor,
        &prepared.backup.root,
        &ctx,
        log_out,
        progress,
    ) {
        crate::apply::Apply::Switched { logged_out: true } => {
            Outcome::Done(format!("switched to {}", target.display_name))
        }
        crate::apply::Apply::Switched { logged_out: false } => Outcome::Done(format!(
            "switched to {}, but the session was not ended. Log out yourself to start it.",
            target.display_name
        )),
        crate::apply::Apply::Failed {
            step,
            detail,
            rolled_back,
        } => Outcome::Failed(format!(
            "{step}: {detail}. {}",
            if rolled_back {
                "Your previous configuration is back and the shell you were using has been \
                 started again."
            } else {
                "Not everything could be undone on its own; the backup listed above is \
                 intact and the log says what was left."
            }
        )),
    }
}
