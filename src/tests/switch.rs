use super::*;
use crate::compositor::hyprland::shells::{
    ambxst::SHELL as AMBXST, caelestia::SHELL as CAELESTIA, dms::SHELL as DMS,
    noctalia::SHELL as NOCTALIA,
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
            // and what its companions keep. Nothing installs a calendar
            // database, so nothing else on the machine would have saved it —
            // and leaving DMS unannounced would take it with the packages.
            ".config/dankcal",
            ".config/dgop",
            ".local/share/dankcal",
            ".local/state/dankcal",
        ]
    );
}

#[test]
fn caelestia_brings_only_its_own_paths() {
    // Its config and state are both the shell's own, so both are saved. The one
    // path beyond them is the launcher entry — the same file DMS and Noctalia
    // each declare — because a switch that left it visible would keep offering
    // the shell it just left in the app menu. That is the only extra, and it is
    // listed as both installed and kept for the two separate questions a switch
    // asks of it.
    assert_eq!(
        relative_list(Some(&CAELESTIA)),
        [
            ".config/gtk-3.0",
            ".config/gtk-4.0",
            ".config/qt5ct",
            ".config/qt6ct",
            ".config/hypr",
            ".config/caelestia",
            ".local/state/caelestia",
            ".local/share/applications/dev.noctalia.Noctalia.desktop",
        ]
    );
    assert_eq!(
        CAELESTIA.skel_extra_paths,
        [".local/share/applications/dev.noctalia.Noctalia.desktop"],
        "the launcher is the one path the skel installs beyond its own directories"
    );
    assert_eq!(
        CAELESTIA.backup_extra_paths,
        [".local/share/applications/dev.noctalia.Noctalia.desktop"],
        "and nothing else would back it up, so it is declared kept as well"
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
            ".config/hypr",
        ]
    );
}

#[test]
fn switching_away_from_ambxst_backs_up_its_config_and_state() {
    // Ambxst is unpackaged, so nothing would reinstall it on the way back — the
    // backup is the only way home. Its config and state are saved like every
    // other shell's, and the launcher entry it overwrites is kept for the same
    // reason DMS and Caelestia keep it.
    //
    // `~/.local/share/ambxst` is deliberately absent: it is user data (chats, the
    // pinned clipboard) that no switch overwrites, so nothing needs to bring it
    // back — it simply stays.
    assert_eq!(
        relative_list(Some(&AMBXST)),
        [
            ".config/gtk-3.0",
            ".config/gtk-4.0",
            ".config/qt5ct",
            ".config/qt6ct",
            ".config/hypr",
            ".config/ambxst",
            ".local/state/ambxst",
            ".local/share/applications/dev.noctalia.Noctalia.desktop",
        ]
    );
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
    assert!(caelestia_only.contains(&".config/caelestia".to_string()));
    assert!(!caelestia_only.contains(&".config/DankMaterialShell".to_string()));
}
