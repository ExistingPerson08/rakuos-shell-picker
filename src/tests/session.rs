use super::*;
use crate::compositor::hyprland::shells::{
    ambxst::SHELL as AMBXST, dms::SHELL as DMS, noctalia::SHELL as NOCTALIA,
};

fn tmp_path(name: &str) -> PathBuf {
    std::env::temp_dir()
        .join("rakuos-shell-picker-tests")
        .join(name)
}

#[test]
fn missing_file_is_default() {
    let state = State::from_path(&tmp_path("does-not-exist.toml")).unwrap();
    assert!(state.active.is_none());
    assert!(state.previous.is_none());
}

#[test]
fn roundtrip_keeps_active_and_previous() {
    let path = tmp_path("roundtrip.toml");
    let _ = fs::remove_file(&path);

    let mut state = State::default();
    state.record_switch(&NOCTALIA);
    state.record_switch(&DMS);
    state.save_to(&path).unwrap();

    let loaded = State::from_path(&path).unwrap();
    assert_eq!(loaded.active.as_deref(), Some("dms"));
    assert_eq!(loaded.previous.as_deref(), Some("noctalia"));
    assert_eq!(loaded.active_shell().map(|s| s.id), Some("dms"));
    assert_eq!(loaded.previous_shell().map(|s| s.id), Some("noctalia"));

    let _ = fs::remove_file(&path);
}

#[test]
fn record_switch_moves_active_into_previous() {
    let mut state = State::default();
    assert!(state.previous.is_none());
    state.record_switch(&AMBXST);
    assert_eq!(state.previous, None);
    assert_eq!(state.active.as_deref(), Some("ambxst"));
}

#[test]
fn state_lives_in_xdg_state_and_backups_live_in_xdg_data() {
    let state = State::state_dir();
    assert!(state.ends_with("rakuos/shell-picker"), "got {state:?}");

    let backups = State::backups_dir();
    assert!(
        backups.ends_with("backups/rakuos-shell-picker"),
        "got {backups:?}"
    );
    // Not under state on purpose: state is the disposable tree, and a backup
    // is the one thing here that must not be swept away with a cache clean.
    assert!(
        !backups.starts_with(&state),
        "backups must not sit inside the state tree: {backups:?}"
    );
}

#[test]
fn backups_follow_xdg_data_home_when_it_is_set() {
    // Set in this process only; the assert is about the code path, and other
    // tests read the real environment.
    let dir = State::backups_dir();
    assert!(dir.is_absolute());
    if let Some(custom) = std::env::var_os("XDG_DATA_HOME") {
        let custom = PathBuf::from(custom);
        if custom.is_absolute() {
            assert!(dir.starts_with(custom), "got {dir:?}");
        }
    }
}

#[test]
fn saving_leaves_no_temporary_file_behind() {
    let path = tmp_path("atomic.toml");
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(path.with_extension("toml.tmp"));

    State::default().save_to(&path).unwrap();

    let survivors: Vec<_> = fs::read_dir(path.parent().unwrap())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("atomic.toml."))
        .collect();
    assert!(
        survivors.is_empty(),
        "temporary file survived: {survivors:?}"
    );
    let _ = fs::remove_file(&path);
}

#[test]
fn a_stale_temporary_file_does_not_hide_the_real_state() {
    // What a crash between write and rename leaves behind.
    let path = tmp_path("stale.toml");
    let tmp = path.with_extension("toml.tmp");
    let _ = fs::remove_file(&path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&tmp, "active = \"half").unwrap();

    let state = State {
        active: Some("noctalia".to_string()),
        ..State::default()
    };
    state.save_to(&path).unwrap();

    assert_eq!(
        State::from_path(&path).unwrap().active.as_deref(),
        Some("noctalia")
    );
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(&tmp);
}

#[test]
fn an_unfinished_switch_is_recorded_and_then_cleared() {
    let mut state = State::default();
    let attempt = Attempt {
        from: "noctalia".to_string(),
        to: "dms".to_string(),
        backup: Some(PathBuf::from("/tmp/backup")),
        started: 1_000,
    };
    state.attempt = Some(attempt.clone());

    // Visible while in flight.
    assert_eq!(state.attempt_in_flight(1_010), Some(&attempt));

    // Once the switch is confirmed, the debt is paid.
    state.record_switch(&DMS);
    assert!(state.attempt.is_none());
    assert!(state.attempt_in_flight(1_010).is_none());
}

#[test]
fn an_attempt_too_old_to_be_live_is_ignored() {
    let state = State {
        attempt: Some(Attempt {
            from: "noctalia".to_string(),
            to: "dms".to_string(),
            backup: None,
            started: 1_000,
        }),
        ..State::default()
    };
    assert!(state
        .attempt_in_flight(1_000 + ATTEMPT_STALE_AFTER)
        .is_some());
    assert!(state
        .attempt_in_flight(1_000 + ATTEMPT_STALE_AFTER + 1)
        .is_none());
}

#[test]
fn an_unfinished_switch_survives_a_restart() {
    let path = tmp_path("attempt.toml");
    let _ = fs::remove_file(&path);

    let state = State {
        attempt: Some(Attempt {
            from: "noctalia".to_string(),
            to: "dms".to_string(),
            backup: Some(PathBuf::from("/tmp/backup-of-noctalia")),
            started: 4_242,
        }),
        ..State::default()
    };
    state.save_to(&path).unwrap();

    let loaded = State::from_path(&path).unwrap();
    let attempt = loaded.attempt_in_flight(4_300).expect("still owed");
    assert_eq!(attempt.from, "noctalia");
    assert_eq!(attempt.to, "dms");
    assert_eq!(
        attempt.backup,
        Some(PathBuf::from("/tmp/backup-of-noctalia"))
    );

    let _ = fs::remove_file(&path);
}
