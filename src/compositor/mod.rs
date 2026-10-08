//! Compositors the picker knows how to switch shells for.
//!
//! This is the only registry the UI walks: each tab comes from here, and each
//! tab brings its own shell list. Hyprland is the only entry for now. Niri was
//! registered here and has been removed — see `compositor::niri` for what
//! bringing it back involves, and why it was not a tab with an empty skel.
//!
//! Each compositor also knows how to recognise itself, because the picker must
//! never act on a compositor that is not the one actually running. See
//! `docs/SAFETY.md` for the rules this implements.
//!
//! With one compositor registered, the cross-compositor refusal is unreachable
//! from the running app: no card can belong to a compositor that is not the one
//! on screen. The rule and its tests stay, because they are the reason adding a
//! second compositor later is a matter of registering it and writing a skel.

pub mod hyprland;
pub mod niri;

#[cfg(test)]
pub mod fake;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::shell::Shell;

/// Read-only view of the process environment.
///
/// Detection reads through this instead of `std::env` directly so it can be
/// tested without mutating the real environment, which is process-global and
/// would make parallel tests racy.
pub trait EnvSource {
    fn var(&self, key: &str) -> Option<String>;
}

/// The real process environment. Empty and whitespace-only values count as
/// absent: a compositor that sets an empty marker is not running.
pub struct SystemEnv;

impl EnvSource for SystemEnv {
    fn var(&self, key: &str) -> Option<String> {
        match std::env::var(key) {
            Ok(value) if !value.trim().is_empty() => Some(value),
            _ => None,
        }
    }
}

impl EnvSource for HashMap<String, String> {
    fn var(&self, key: &str) -> Option<String> {
        self.get(key).filter(|v| !v.trim().is_empty()).cloned()
    }
}

/// `Sync` because a switch runs on a worker thread and the compositor it is
/// switching under has to be readable from there. Every implementation is a
/// stateless zero-sized struct, so this costs nothing and rules out a
/// compositor that quietly keeps state.
pub trait Compositor: Sync {
    fn id(&self) -> &'static str;
    fn label(&self) -> &'static str;
    /// Shells registered for this compositor. May legitimately be empty.
    fn shells(&self) -> &'static [Shell];

    /// Environment variable this compositor sets for everything it spawns.
    /// This is the primary detection signal.
    fn marker_env(&self) -> &'static str;

    /// The compositor's own configuration directory, relative to `$HOME`.
    ///
    /// Declared here rather than listed among the shared paths because it is the
    /// one path whose name follows the compositor: `~/.config/hypr` on Hyprland,
    /// `~/.config/niri` on Niri. A switch touches it, so a backup has to save it,
    /// and hardcoding either one in shared code would make the other compositor a
    /// second-class citizen the moment it is actually used.
    fn config_dir(&self) -> &'static str;

    /// Whether this compositor is the one currently running.
    fn is_running_with(&self, env: &dyn EnvSource) -> bool;

    /// Ask the compositor to pick up whatever is on disk now.
    ///
    /// A switch replaces [`config_dir`](Self::config_dir) while the compositor
    /// is still running the copy it read at login, and nothing else reloads it
    /// — the session is restarted at the *end* of a switch, after the stages
    /// that run as root. Re-reading the configuration first is what makes the
    /// new shell's rules the ones in effect for the rest of the switch.
    ///
    /// Reported, never fatal: a compositor that cannot reload still has the
    /// restart at the end, and losing that would turn a cosmetic step into a
    /// reason to abandon a switch that has already succeeded.
    fn reload(&self) -> Result<(), String>;

    fn is_running(&self) -> bool {
        self.is_running_with(&SystemEnv)
    }
}

pub static HYPRLAND: hyprland::Hyprland = hyprland::Hyprland;

/// Tab order is the order of this list.
///
/// One entry, so the app opens on a tab bar of one. That is the whole point of
/// removing Niri rather than leaving it registered: a registered compositor
/// promises a working page, and an empty skel could only ever refuse.
pub fn all() -> Vec<&'static dyn Compositor> {
    vec![&HYPRLAND]
}

/// The tab shown when the app opens.
///
/// The running compositor's tab, never a fixed one, because opening on a
/// compositor's blocked page would greet the user with a refusal and no hint
/// that another tab was the usable one. Falls back to the first registered
/// compositor when nothing is detected, because a tab still has to be selected.
pub fn default_id(detection: &Detection) -> &'static str {
    if let Some(current) = detection.current {
        if all().iter().any(|c| c.id() == current) {
            return current;
        }
    }
    all()[0].id()
}

/// Is there a Wayland session we can trust?
///
/// Both variables must be present. Anything less and the picker refuses to
/// act, rather than guessing: starting the wrong shell is worse than starting
/// nothing.
pub fn wayland_session(env: &dyn EnvSource) -> Option<WaylandSession> {
    let display = env.var("WAYLAND_DISPLAY")?;
    let runtime_dir = env.var("XDG_RUNTIME_DIR")?;
    if !Path::new(&runtime_dir).is_absolute() {
        return None;
    }
    Some(WaylandSession {
        display,
        runtime_dir: PathBuf::from(runtime_dir),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaylandSession {
    pub display: String,
    pub runtime_dir: PathBuf,
}

/// Per-compositor detection result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    pub id: &'static str,
    pub label: &'static str,
    pub detected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    /// A usable Wayland session exists.
    pub wayland: bool,
    pub runtime_dir: Option<PathBuf>,
    /// The running compositor, if confidently identified.
    pub current: Option<&'static str>,
    /// Per-compositor detail, for the app log and the UI badges.
    pub statuses: Vec<Status>,
}

impl Detection {
    pub fn is_current(&self, compositor: &dyn Compositor) -> bool {
        self.current == Some(compositor.id())
    }

    /// Why nothing was detected, in a form that can go straight into the UI.
    pub fn refusal(&self) -> Option<String> {
        if !self.wayland {
            return Some(
                "no usable Wayland session (WAYLAND_DISPLAY / XDG_RUNTIME_DIR not set)."
                    .to_string(),
            );
        }
        if self.current.is_some() {
            return None;
        }
        let checked: Vec<_> = self.statuses.iter().map(|s| s.label).collect();
        Some(format!(
            "could not identify the running compositor (checked: {}). refusing to guess.",
            checked.join(", ")
        ))
    }
}

pub fn detect() -> Detection {
    detect_with(&SystemEnv)
}

pub fn detect_with(env: &dyn EnvSource) -> Detection {
    let session = wayland_session(env);

    let statuses: Vec<Status> = all()
        .iter()
        .map(|c| Status {
            id: c.id(),
            label: c.label(),
            detected: c.is_running_with(env),
        })
        .collect();

    // A marker only counts when there is a real session to go with it. Without
    // this gate a stale marker left over from a previous session could make the
    // picker act on a compositor that is long gone.
    let current = session
        .as_ref()
        .and_then(|_| statuses.iter().find(|s| s.detected).map(|s| s.id));

    Detection {
        wayland: session.is_some(),
        runtime_dir: session.map(|s| s.runtime_dir),
        current,
        statuses,
    }
}

/// The running compositor, if it was identified with confidence.
pub fn current() -> Option<&'static dyn Compositor> {
    current_with(&SystemEnv)
}

pub fn current_with(env: &dyn EnvSource) -> Option<&'static dyn Compositor> {
    let id = detect_with(env).current?;
    all().into_iter().find(|c| c.id() == id)
}

/// A unix socket that is actually connectable-looking.
pub fn socket_alive(path: &Path) -> bool {
    use std::os::unix::fs::FileTypeExt;
    std::fs::metadata(path)
        .map(|m| m.file_type().is_socket())
        .unwrap_or(false)
}

#[cfg(test)]
#[path = "../tests/compositor.rs"]
mod tests;
