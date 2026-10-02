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
}

impl State {
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
        fs::write(path, raw).with_context(|| format!("writing {}", path.display()))
    }

    pub fn record_switch(&mut self, to: &Shell) {
        self.previous = self.active.clone();
        self.active = Some(to.id.to_string());
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
}
