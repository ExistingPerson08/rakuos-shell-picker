//! Niri backend.
//!
//! Separate from Hyprland on purpose: Niri has its own session lifecycle, its
//! own `niri msg` tooling and its own config layout, so nothing here is shared
//! with the Hyprland backend.

pub mod shells;

use crate::compositor::{socket_alive, Compositor, EnvSource};
use crate::shell::Shell;

pub struct Niri;

impl Compositor for Niri {
    fn id(&self) -> &'static str {
        "niri"
    }

    fn label(&self) -> &'static str {
        "niri"
    }

    fn shells(&self) -> &'static [Shell] {
        shells::ALL
    }

    fn marker_env(&self) -> &'static str {
        "NIRI_SOCKET"
    }

    fn is_running_with(&self, env: &dyn EnvSource) -> bool {
        let Some(socket) = env.var(self.marker_env()) else {
            return false;
        };

        // A copied-but-unexpanded variable looks like this and must not count.
        if socket.contains('%') {
            return false;
        }

        // Here the socket path is the signal itself, so unlike Hyprland it has
        // to be a real, live socket. Niri is not installed in this image yet,
        // so this path is currently always false.
        socket_alive(std::path::Path::new(&socket))
    }
}
