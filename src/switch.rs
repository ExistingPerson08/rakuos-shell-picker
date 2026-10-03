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
/// looks, and the compositor and UWSM settings decide how the new shell is
/// started at all. They belong to the session rather than to a shell, which is
/// why they are listed here instead of coming from a registry entry.
///
/// The compositor's own configuration is not in this list: its path follows the
/// compositor, so it comes from [`Compositor::config_dir`] instead. See
/// [`backup_sources`]. `uwsm` is compositor-independent and stays.
pub const SHARED_PATHS: &[&str] = &[
    ".config/gtk-3.0",
    ".config/gtk-4.0",
    ".config/qt5ct",
    ".config/qt6ct",
    ".config/uwsm",
];

/// Everything to copy before switching away from `current`.
///
/// Shared paths first, then the outgoing shell's own config, then its state.
/// Paths that do not exist stay in the list on purpose: the backup records them
/// as absent, which is what later lets a restore tell "was never there" apart
/// from "was lost".
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
        for relative in [shell.config_dir, shell.state_dir].into_iter().flatten() {
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
                // shared: toolkit settings and UWSM, which follow nothing
                ".config/gtk-3.0",
                ".config/gtk-4.0",
                ".config/qt5ct",
                ".config/qt6ct",
                ".config/uwsm",
                // the compositor's own config, which follows the compositor
                ".config/hypr",
                // the outgoing shell's own config and its state
                ".config/noctalia",
                ".local/state/noctalia",
            ]
        );
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
                ".config/uwsm",
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

/// Run steps 1 to 3: refuse, preflight, back up.
///
/// Stops on purpose. The remaining steps — dependencies, `pkexec rum install`,
/// skel, stop, start, verify — are not written yet, so nothing here touches the
/// running shell, the target's config, or any package. `docs/SAFETY.md` is the
/// order; this is where that order currently stops.
///
/// `compositor_id` and `current` come from detection, which the caller re-reads
/// rather than trusting the value it started with.
pub fn prepare(
    compositor: &'static dyn Compositor,
    current: Option<&str>,
    target: &'static Shell,
    running: Option<&'static Shell>,
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
    if target.running() {
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
pub fn install(plan: InstallPlan) -> Install {
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
        let missing: Vec<&str> = stage
            .iter()
            .copied()
            .filter(|p| !crate::shell::package_origin(p).installed())
            .collect();
        if missing.is_empty() {
            crate::logging::info(&format!(
                "stage {} already satisfied: {}",
                index + 1,
                stage.join(", ")
            ));
            continue;
        }

        let line = plan.stage_command_line(&missing);
        crate::logging::info(&format!(
            "stage {} of {}: {line}",
            index + 1,
            plan.stages.len()
        ));

        let mut command = Command::new(plan.program);
        command.args(plan.args).args(&missing);
        command.env("LANG", "C");

        let result = match command.output() {
            Ok(result) => result,
            Err(error) => {
                return Install::Failed {
                    stage: index + 1,
                    status: format!("could not run {}", plan.program),
                    output: error.to_string(),
                }
            }
        };

        let text = String::from_utf8_lossy(&result.stdout).to_string()
            + &String::from_utf8_lossy(&result.stderr);
        if !output.is_empty() && !text.is_empty() {
            output.push('\n');
        }
        output.push_str(&text);

        if result.status.success() {
            ran += 1;
            continue;
        }
        if looks_dismissed(result.status.code(), &text) {
            crate::logging::warn("the polkit prompt was dismissed");
            return Install::Dismissed { stage: index + 1 };
        }
        crate::logging::error(&format!(
            "stage {} of {} failed: {}",
            index + 1,
            plan.stages.len(),
            result.status
        ));
        return Install::Failed {
            stage: index + 1,
            status: result.status.to_string(),
            output: text,
        };
    }

    Install::Finished {
        stages: ran,
        output,
    }
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
        assert!(!plan.stage_command_line(&plan.stages[0]).contains("dms"));
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
        // The refusal on reuse stays as the backstop for a real clash: if two
        // names ever did coincide, the second must fail rather than write into
        // the first one's directory.
        let taken = root.join("backups");
        crate::backup::Backup::create(&taken, "clash", &[], &home)
            .expect("the first use of a name is fine");
        assert!(
            crate::backup::Backup::create(&taken, "clash", &[], &home).is_err(),
            "an existing name must be refused, never overwritten"
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
}
