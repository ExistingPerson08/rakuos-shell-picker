//! Niri backend.
//!
//! Separate from Hyprland on purpose: Niri has its own session lifecycle, its
//! own `niri msg` tooling and its own config layout, so nothing here is shared
//! with the Hyprland backend.

use crate::compositor::Compositor;
use crate::shell::Shell;

pub struct Niri;

impl Compositor for Niri {
    fn id(&self) -> &'static str {
        "niri"
    }

    fn label(&self) -> &'static str {
        "niri"
    }

    /// Nothing registered yet. The UI renders a "coming soon" placeholder for
    /// an empty list instead of failing or borrowing Hyprland's shells.
    fn shells(&self) -> &'static [Shell] {
        &[]
    }
}
