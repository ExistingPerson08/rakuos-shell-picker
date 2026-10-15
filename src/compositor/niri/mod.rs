//! Niri backend.
//!
//! Detection is not the marker alone. `NIRI_SOCKET` is the variable Niri sets
//! for everything it spawns, but a shell that copied the value out of a template
//! unexpanded (`%NIRI_SOCKET%`) or a session that has since ended both leave the
//! variable behind. The socket path *is* the connection, so what is checked is
//! that the path names a real unix socket — [`crate::compositor::socket_alive`].
//! A copied-but-unexpanded value and a stale path both fail that check, which a
//! bare `env.var` would have accepted.
//!
//! Reload and quit go through `niri msg action`, the compositor's own control
//! channel: `reload-config` re-reads the configuration a switch replaced, and
//! `quit` ends the session at the end of a switch. This is the same reasoning as
//! Hyprland's `hyprctl`, and it is what lets the picker end a session it does not
//! own — see [`crate::compositor::Compositor::exit`].
//!
//! The shells Niri offers are in [`shells`], each a [`crate::shell::Port`] that
//! states only what differs from Hyprland; everything true of the shell anywhere
//! is in [`crate::shells`] and is not repeated there.

pub mod shells;

use std::path::Path;

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

    fn config_dir(&self) -> &'static str {
        ".config/niri"
    }

    fn marker_env(&self) -> &'static str {
        "NIRI_SOCKET"
    }

    fn is_running_with(&self, env: &dyn EnvSource) -> bool {
        let Some(socket) = env.var(self.marker_env()) else {
            return false;
        };
        socket_alive(Path::new(&socket))
    }

    fn reload(&self) -> Result<(), String> {
        self.action("reload-config")
    }

    fn exit(&self) -> Result<(), String> {
        self.action("quit")
    }
}

impl Niri {
    /// Ask the running compositor to do one thing over its own socket.
    ///
    /// Reported in the same shape Hyprland uses, so a failed reload prints the
    /// same kind of message whichever compositor is running.
    fn action(&self, action: &str) -> Result<(), String> {
        let out = std::process::Command::new("niri")
            .args(["msg", "action", action])
            .output()
            .map_err(|e| format!("cannot run niri msg action {action}: {e}"))?;
        if out.status.success() {
            return Ok(());
        }
        let detail = String::from_utf8_lossy(&out.stderr);
        let detail = detail.trim();
        Err(if detail.is_empty() {
            format!("niri msg action {action} exited with {}", out.status)
        } else {
            format!("niri msg action {action}: {detail}")
        })
    }
}
