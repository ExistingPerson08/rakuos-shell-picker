//! Shells runnable under Niri.
//!
//! Same shape as `compositor::hyprland::shells`: one file per shell, each stating
//! only what is specific to Niri, with everything true of the shell anywhere left
//! in `crate::shells`. No folder imports another.
//!
//! Niri's two are the ones the `rakuos-niri` image knows: DMS in the base image,
//! and Noctalia installed from the same repository the other shells come from.
//! Caelestia is absent because it is Hyprland-only upstream — only third-party
//! forks exist — and Ambxst has no Niri port. A card existing on one compositor
//! and not another is the reason these registries are per compositor rather than
//! one shared list.
//!
//! Both Niri skels come from the image's own `/etc/skel`, so they are the files a
//! fresh Niri install starts from rather than a hand-written set.

pub mod dms;
pub mod noctalia;

use crate::shell::Shell;

/// Card order in the grid. DMS is first because it is the shell the image ships
/// running.
pub static ALL: &[Shell] = &[dms::SHELL, noctalia::SHELL];
