//! Shells runnable under Hyprland.
//!
//! One folder per shell. Everything specific to a shell lives inside its own
//! folder and nowhere else:
//!
//! ```text
//! shells/dms/mod.rs       the Shell record, and what DMS does about backup,
//!                         install, start and verify, spelled out at the top
//! ```
//!
//! The split is per shell rather than per step because the steps are not the
//! same for any two shells: Noctalia is in the base image and started from a Lua
//! autostart, DMS is installed from a repository and started by a systemd user
//! unit, Caelestia has no package at all and runs on a runtime it shares with
//! another shell, and Ambxst records no config location. What they have in
//! common, such as the six shared config paths a backup always saves, stays in
//! `src/switch.rs` so there is one copy of it rather than four that can drift.

pub mod ambxst;
pub mod caelestia;
pub mod dms;
pub mod noctalia;

use crate::shell::Shell;

/// Card order in the grid.
pub static ALL: &[Shell] = &[noctalia::SHELL, dms::SHELL, caelestia::SHELL, ambxst::SHELL];
