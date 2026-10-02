//! Hyprland backend.
//!
//! Compositor-specific tooling for Hyprland (hyprctl wrappers, window rules,
//! autostart handling) belongs here, next to the shells it applies to.

pub mod shells;

use crate::compositor::{Compositor, EnvSource};
use crate::shell::Shell;

pub struct Hyprland;

impl Compositor for Hyprland {
    fn id(&self) -> &'static str {
        "hyprland"
    }

    fn label(&self) -> &'static str {
        "hyprland"
    }

    fn shells(&self) -> &'static [Shell] {
        shells::ALL
    }

    fn marker_env(&self) -> &'static str {
        "HYPRLAND_INSTANCE_SIGNATURE"
    }

    fn is_running_with(&self, env: &dyn EnvSource) -> bool {
        let Some(signature) = env.var(self.marker_env()) else {
            return false;
        };
        // The signature is set for every child of Hyprland, so it is the signal
        // we trust. The socket is only a soft confirmation: its exact path has
        // moved between Hyprland releases, and a wrong path would turn a
        // running Hyprland into "not running". When the path cannot be worked
        // out at all, the marker stands on its own.
        match self.socket_path(env, &signature) {
            Some(path) => path.exists(),
            None => true,
        }
    }
}

impl Hyprland {
    /// `<runtime dir>/hypr/<signature>/.socket.sock`
    fn socket_path(&self, env: &dyn EnvSource, signature: &str) -> Option<std::path::PathBuf> {
        let runtime_dir = env.var("XDG_RUNTIME_DIR")?;
        let base = std::path::Path::new(&runtime_dir)
            .join("hypr")
            .join(signature);
        for candidate in [".socket.sock", ".socket2.sock"] {
            let path = base.join(candidate);
            if path.exists() {
                return Some(path);
            }
        }
        // Signature directory exists but neither socket name does: let the
        // caller see the directory so we do not fall back to trusting the
        // marker over contradicting evidence.
        base.exists().then_some(base.join(".socket.sock"))
    }
}
