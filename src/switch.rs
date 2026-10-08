//! The switch, in the order `docs/SAFETY.md` fixes.
//!
//! Deciding what a switch would touch, copying it, installing the packages that
//! are missing, and — once the new shell has been proved alive — taking the
//! outgoing shell's packages away again. Nothing in this module stops or starts
//! anything; that stays in `apply`.
//!
//! The order the user settled on is backup, then dependencies, then
//! `pkexec rum install`, and it is deliberate. A backup is read-only, so doing
//! it first costs nothing and means the switch can refuse later without having
//! changed anything. Installing is the first step that writes to the system, so
//! it goes after everything that can still say no. Uninstalling is the last, and
//! only ever runs once the backup exists and the new shell has started, because
//! it is the one step that cannot be undone by putting a file back.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::compositor::Compositor;
use crate::shell::{InstallPlan, Shell, UninstallPlan};

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
    // The compositor's config is saved on every switch. Unlike the rest of this
    // list it is not preserved by being sourced from a backup later: `plan`
    // sources `~/.config/hypr` from the target shell's skel, so the switch
    // replaces it outright. This copy is the only thing that can ever bring
    // back what the user had in there, which is why it is taken unconditionally.
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
#[path = "tests/switch.rs"]
mod tests;

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
    /// The plan names no packages, so there was nothing to install.
    ///
    /// Not reachable from a declared shell: `every_installed_plan_can_do_something`
    /// rejects such a plan. This exists because the check that follows is
    /// vacuously true for an empty list, and a switch must never go on to start a
    /// shell that was never installed — not even on the strength of an install
    /// step that did nothing.
    Malformed { package: &'static str },
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
/// The plan also passes `--refresh`, so the metadata is re-fetched instead of
/// trusted. `metadata_expire` bounds how old a cached copy may be; it says
/// nothing about whether the repository moved underneath it. A copy fetched the
/// day before is inside its window and still names a package the repository no
/// longer carries, which does not look like a metadata problem at all: the
/// transaction resolves happily and then the download finds no file to fetch.
/// Refreshing costs a few seconds on every switch; resolving against metadata
/// that is young but wrong costs the whole stage, and the backup has already
/// been taken by then.
///
/// A stage that fails stops the plan. Later stages are not attempted, so a
/// failure in the first one leaves nothing of the shell itself installed.
///
/// `progress` is called with the package manager's own output as it arrives,
/// not after the command returns. Without it the panel shows nothing at all for
/// the length of the download, and a working install is indistinguishable from a
/// hung one.
pub fn install(plan: InstallPlan, progress: &mut dyn FnMut(&str)) -> Install {
    let packages = plan.packages();

    // Before the "already there" question, which an empty list answers yes.
    if packages.is_empty() {
        crate::logging::error(&format!(
            "{} has an install plan that names no packages",
            plan.package
        ));
        progress(&format!(
            "the install plan for {} names no packages, so there is nothing to install",
            plan.package
        ));
        return Install::Malformed {
            package: plan.package,
        };
    }

    if packages
        .iter()
        .all(|p| crate::shell::package_origin(p).installed())
    {
        return Install::AlreadyThere { packages };
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
        // One line, not two: `progress` already reaches the application log on
        // both paths, and logging it here as well printed the stage twice.
        progress(&format!("stage {number} of {}: {line}", plan.stages.len()));
        progress(&format!(
            "authorizing stage {number}; a password prompt should appear"
        ));

        let mut command = Command::new(plan.program);
        command.args(plan.args).args(&missing);
        command.env("LANG", "C");

        let (status, text) = match run_streaming(&mut command, &mut *progress, "installing") {
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

/// What came of taking a shell's packages off the machine.
///
/// Mirrors [`Install`] rather than reusing it: the two are reported differently
/// on screen, and a single enum would have to grow a variant that only ever
/// appears for one of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Removed {
    /// Every stage ran and succeeded.
    Finished { stages: usize, output: String },
    /// No package in the plan was installed, so nothing ran.
    AlreadyGone { packages: Vec<&'static str> },
    /// The polkit prompt was dismissed. Nothing beyond earlier stages changed.
    Dismissed { stage: usize },
    /// A stage ran and failed. Later stages were not attempted, so the shell is
    /// still on the machine — which is the safe half-removed position.
    Failed {
        stage: usize,
        status: String,
        output: String,
    },
    /// The plan names no packages, so there was nothing to remove.
    Malformed { package: &'static str },
    /// Every package the plan names is one the shell being switched to still
    /// runs on, so nothing was removed. Not a failure and not "already gone":
    /// the packages are there on purpose, and saying otherwise would leave a
    /// reader looking for a package that was never taken away.
    Kept { packages: Vec<&'static str> },
}

/// Remove the plan's packages one stage at a time, asking polkit each time.
///
/// Runs only from the tail of a switch: the backup is already written and
/// verified, and the new shell has already proved itself alive. Nothing here can
/// be undone by putting a file back, so a failure is reported rather than rolled
/// back — the switch has succeeded by this point, and the packages that are
/// still present are simply still there for a later attempt.
///
/// Packages already absent are skipped rather than sent to the package manager,
/// which is the mirror of the install's behaviour and for the same reason: a
/// stage that has nothing to do should not ask for a password.
///
/// `keep` is what makes it safe to remove more than the outgoing shell's own
/// packages: any package the **incoming** shell declares as something it runs on
/// is dropped from every stage before the package manager sees it. That is the
/// only reason `quickshell` can be in a removal plan at all — see
/// [`crate::shell::Identity::runtime_packages`].
pub fn uninstall(
    plan: UninstallPlan,
    keep: &[&'static str],
    progress: &mut dyn FnMut(&str),
) -> Removed {
    let packages = plan.packages();

    if packages.is_empty() {
        crate::logging::error(&format!(
            "{} has an uninstall plan that names no packages",
            plan.package
        ));
        progress(&format!(
            "the removal plan for {} names no packages, so there is nothing to remove",
            plan.package
        ));
        return Removed::Malformed {
            package: plan.package,
        };
    }

    let kept: Vec<&'static str> = packages
        .iter()
        .copied()
        .filter(|p| keep.contains(p))
        .collect();
    let removable: Vec<&'static str> = packages
        .iter()
        .copied()
        .filter(|p| !keep.contains(p))
        .collect();

    if !kept.is_empty() {
        let line = format!(
            "keeping {}: the shell that is starting runs on them, so this switch \
             will not remove them",
            kept.join(", ")
        );
        progress(&line);
    }

    if removable.is_empty() {
        return Removed::Kept { packages: kept };
    }

    if removable
        .iter()
        .all(|p| !crate::shell::package_origin(p).installed())
    {
        return Removed::AlreadyGone {
            packages: removable,
        };
    }

    let mut output = String::new();
    let mut ran = 0usize;

    for (index, stage) in plan.stages.iter().enumerate() {
        let stage = *stage;
        let number = index + 1;
        let present: Vec<&str> = stage
            .iter()
            .copied()
            .filter(|p| !keep.contains(p))
            .filter(|p| crate::shell::package_origin(p).installed())
            .collect();

        if present.is_empty() {
            progress(&format!(
                "stage {number} of {}: {} already removed, skipping",
                plan.stages.len(),
                stage.join(", ")
            ));
            continue;
        }

        let line = plan.stage_command_line(&present);
        progress(&format!("stage {number} of {}: {line}", plan.stages.len()));
        progress(&format!(
            "authorizing stage {number}; a password prompt should appear"
        ));

        let mut command = Command::new(plan.program);
        command.args(plan.args).args(&present);
        command.env("LANG", "C");

        let (status, text) = match run_streaming(&mut command, &mut *progress, "removing") {
            Ok(result) => result,
            Err(error) => {
                progress(&format!("could not start the package manager: {error}"));
                return Removed::Failed {
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
            return Removed::Dismissed { stage: number };
        }
        crate::logging::error(&format!(
            "stage {number} of {} failed: {status}",
            plan.stages.len()
        ));
        return Removed::Failed {
            stage: number,
            status: status.to_string(),
            output: text,
        };
    }

    Removed::Finished {
        stages: ran,
        output,
    }
}

/// How often to say that something is still happening when the package manager
/// is quiet. Long enough not to spam, short enough that silence never reads as a
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
    verb: &'static str,
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
                    .send(format!("  still {verb} ({}s)", started.elapsed().as_secs()))
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
                    progress(&format!("  still {verb}"));
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
#[path = "tests/switch_install.rs"]
mod install_tests;

#[cfg(test)]
#[path = "tests/switch_uninstall.rs"]
mod uninstall_tests;

/// Backup directory name stamp: `20261003`, UTC — the date and nothing more.
///
/// A day of switching therefore writes `20261003-hyprland-noctalia` once and
/// replaces it on the next switch with the same outgoing shell, instead of
/// leaving one directory per click. See [`crate::clock::day`] for why the time
/// is deliberately left out.
///
/// This runs on the worker thread during a switch, which is why it comes from
/// [`crate::clock`] rather than from GLib. See that module for what happened
/// when it did not.
fn timestamp() -> String {
    crate::clock::day()
}

#[cfg(test)]
#[path = "tests/switch_prepare.rs"]
mod prepare_tests;

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
                "the permission prompt for stage {stage} was dismissed. Your backup is still in place."
            ));
        }
        Some(Install::Malformed { package }) => {
            return Outcome::Failed(format!(
                "the install plan for {package} names no packages, so nothing was installed. \\
                 This is a bug in the picker, not a problem with your machine. Your backup is still \\
                 in place."
            ));
        }
        Some(Install::Failed {
            stage,
            status,
            output,
        }) => {
            return Outcome::Failed(format!(
                "stage {stage} failed ({status}); later stages were not run. \
                 Your backup is still in place. rum said: {}",
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
