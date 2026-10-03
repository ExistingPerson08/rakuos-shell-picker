use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::shell::Shell;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct State {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<String>,
    /// A switch that started and has not been confirmed yet.
    ///
    /// Written before anything is stopped. If the process dies mid-switch the
    /// next start finds this and knows a rollback is owed, which is the only
    /// reason it is written this early.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempt: Option<Attempt>,
}

/// A switch in flight.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attempt {
    /// The shell being left behind.
    pub from: String,
    /// The shell being started.
    pub to: String,
    /// Where the outgoing config was copied, if it was.
    pub backup: Option<PathBuf>,
    /// Unix seconds, so a stale attempt can be told from a live one.
    pub started: u64,
}

/// How long an attempt may sit in the state file before it is treated as debris.
pub const ATTEMPT_STALE_AFTER: u64 = 900;

impl State {
    /// Is there an unfinished switch, and is it recent enough to still matter?
    pub fn attempt_in_flight(&self, now: u64) -> Option<&Attempt> {
        let attempt = self.attempt.as_ref()?;
        (now.saturating_sub(attempt.started) <= ATTEMPT_STALE_AFTER).then_some(attempt)
    }

    pub fn load() -> Self {
        Self::from_path(&path()).unwrap_or_default()
    }

    pub fn from_path(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&path())
    }

    /// Explicit path, so tests never touch the real state dir.
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        let raw = toml::to_string_pretty(self)?;

        // Written beside the target and renamed over it. A plain write can leave
        // a half-written state file if the machine goes down mid-write, and a
        // state file that cannot be parsed is read as "no shell recorded",
        // which loses the record of the shell the user is actually looking at.
        let tmp = path.with_extension("toml.tmp");
        fs::write(&tmp, &raw).with_context(|| format!("writing {}", tmp.display()))?;
        fs::rename(&tmp, path)
            .with_context(|| format!("replacing {} with {}", path.display(), tmp.display()))?;
        Ok(())
    }

    pub fn record_switch(&mut self, to: &Shell) {
        self.previous = self.active.clone();
        self.active = Some(to.id.to_string());
        self.attempt = None;
    }

    pub fn active_shell(&self) -> Option<&'static Shell> {
        self.active.as_deref().and_then(crate::shell::find)
    }

    pub fn previous_shell(&self) -> Option<&'static Shell> {
        self.previous.as_deref().and_then(crate::shell::find)
    }

    pub fn state_dir() -> PathBuf {
        state_dir()
    }

    pub fn backups_dir() -> PathBuf {
        state_dir().join("backups")
    }
}

fn state_dir() -> PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default();
            home.join(".local/state")
        });
    base.join("rakuos/shell-picker")
}

fn path() -> PathBuf {
    state_dir().join("state.toml")
}

#[cfg(test)]
mod tests {
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
    fn state_dir_is_under_xdg_state() {
        let dir = State::state_dir();
        assert!(dir.ends_with("rakuos/shell-picker"));
        assert!(State::backups_dir().starts_with(&dir));
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
}
