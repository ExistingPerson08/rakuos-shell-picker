//! Compositors the picker knows how to switch shells for.
//!
//! This is the only registry the UI walks: each tab comes from here, and each
//! tab brings its own shell list. Adding Niri shells later means adding files
//! under `compositor::niri::shells`, with no change to the UI.

pub mod hyprland;
pub mod niri;

use crate::shell::Shell;

pub trait Compositor {
    /// Stable key used for the stack child name and the session state.
    fn id(&self) -> &'static str;
    /// Text shown on the tab.
    fn label(&self) -> &'static str;
    /// Shells registered for this compositor. May legitimately be empty.
    fn shells(&self) -> &'static [Shell];
}

pub static HYPRLAND: hyprland::Hyprland = hyprland::Hyprland;
pub static NIRI: niri::Niri = niri::Niri;

/// Tab order is the order of this list.
pub fn all() -> Vec<&'static dyn Compositor> {
    vec![&HYPRLAND, &NIRI]
}

/// The tab shown when the app opens.
pub fn default_id() -> &'static str {
    HYPRLAND.id()
}
