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

use crate::shell::Shell;

/// Paths a switch disturbs no matter which shell is running.
///
/// The toolkit settings decide how every GTK and Qt program on the desktop
/// looks, and the compositor and UWSM settings decide how the new shell is
/// started at all. They belong to the session rather than to a shell, which is
/// why they are listed here instead of coming from a registry entry.
///
/// `.config/hypr` is here even though a switch never replaces it wholesale: the
/// autostart shim is migrated into it once, so it has to be recoverable.
pub const SHARED_PATHS: &[&str] = &[
    ".config/gtk-3.0",
    ".config/gtk-4.0",
    ".config/hypr",
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
pub fn backup_sources(home: &Path, current: Option<&'static Shell>) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = SHARED_PATHS.iter().map(|p| home.join(p)).collect();
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

    fn relative_list(shell: Option<&'static Shell>) -> Vec<String> {
        backup_sources(&home(), shell)
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
                // shared: toolkit settings, the compositor, and UWSM
                ".config/gtk-3.0",
                ".config/gtk-4.0",
                ".config/hypr",
                ".config/qt5ct",
                ".config/qt6ct",
                ".config/uwsm",
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
                ".config/hypr",
                ".config/qt5ct",
                ".config/qt6ct",
                ".config/uwsm",
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
