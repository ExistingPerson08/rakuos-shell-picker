//! Hyprland backend.
//!
//! Compositor-specific tooling for Hyprland (hyprctl wrappers, window rules,
//! autostart handling) belongs here, next to the shells it applies to.

pub mod shells;

use crate::compositor::Compositor;
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
}
