//! Shells runnable under Hyprland.
//!
//! One file per shell: everything specific to that shell (start command,
//! config location, supervisor, later the stop/backup logic) stays in its own
//! file so backend work does not collide between shells.

pub mod ambxst;
pub mod caelestia;
pub mod dms;
pub mod noctalia;

use crate::shell::Shell;

/// Card order in the grid.
pub static ALL: &[Shell] = &[noctalia::SHELL, dms::SHELL, caelestia::SHELL, ambxst::SHELL];
