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

    /// Where verified backups are written.
    ///
    /// `share`, not `state`, and the distinction is deliberate. `$XDG_STATE_HOME`
    /// is defined as the place for things that are not important enough to
    /// synchronise and that are safe to delete between runs, which is the
    /// opposite of a backup: it is a copy of the user's configuration, kept
    /// precisely so it can be put back. Putting them in `state` also means the
    /// routine `rm -rf ~/.local/state/*` that people run to clear caches throws
    /// away the recovery copies along with them.
    pub fn backups_dir() -> PathBuf {
        data_dir().join("backups").join(crate::shell::PICKER_DIR)
    }
}

/// `$XDG_DATA_HOME`, or `~/.local/share`.
fn data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default();
            home.join(".local").join("share")
        })
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
#[path = "tests/session.rs"]
mod tests;
