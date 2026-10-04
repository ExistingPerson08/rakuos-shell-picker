//! Shells runnable under Niri.
//!
//! One file per shell, and each one states only what is specific to Niri: where the
//! shell's config lives here, how it starts here, which skel paths its Niri skel
//! carries. Everything true of the shell on any compositor lives in `crate::shells`.
//!
//! Deliberately its own list, not a copy of Hyprland's. What a shell supports is a
//! property of the shell and the compositor together, and the two lists are not the
//! same: Caelestia Shell is absent here because upstream only ships a Hyprland
//! shell, and the Niri versions that exist are third party forks
//! (`caelestia-dots/shell` has no official Niri port). Noctalia, DMS and Ambxst each
//! document Niri support upstream.
//!
//! Listing the difference is the point: it is how a user can tell what they would
//! get after switching image.

pub mod ambxst;
pub mod dms;
pub mod noctalia;

use crate::shell::Shell;

/// Card order in the grid.
pub static ALL: &[Shell] = &[noctalia::SHELL, dms::SHELL, ambxst::SHELL];
