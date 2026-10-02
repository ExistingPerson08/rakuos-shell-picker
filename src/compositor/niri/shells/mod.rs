//! Shells runnable under Niri.
//!
//! Deliberately its own list, not a copy of Hyprland's. What a shell supports
//! is a property of the shell and the compositor together, and the two lists are
//! not the same: Caelestia Shell is absent here because upstream only ships a
//! Hyprland shell, and the Niri versions that exist are third party forks
//! (caelestia-dots/shell has no official Niri port). Noctalia, DMS and Ambxst
//! each document Niri support upstream.
//!
//! Listing the difference is the point: it is how a user can tell what they
//! would get after switching image.

pub mod ambxst;
pub mod dms;
pub mod noctalia;

use crate::shell::Shell;

/// Card order in the grid.
pub static ALL: &[Shell] = &[noctalia::SHELL, dms::SHELL, ambxst::SHELL];
