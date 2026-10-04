//! Caelestia Shell on Hyprland.
//!
//! Only what is specific to this compositor. The fact that it has no package, and
//! that its verification is unsolved, are in [`crate::shells::caelestia`].
//!
//! - **backup**: `~/.config/quickshell/caelestia`, once it exists.
//! - **start**: the compositor's start event runs `quickshell -c caelestia`.
//!
//! The card is refused rather than offered, because there is no package to install
//! it from and no settled way to tell it apart from any other quickshell shell.

use crate::shell::{Port, Shell, StartMechanism};
use crate::shells::caelestia::IDENTITY;

const PORT: Port = Port {
    config_dir: Some(".config/quickshell/caelestia"),
    state_dir: None,
    systemd_user_unit: None,
    mechanism: StartMechanism::StartEvent,
    skel_extra_paths: &[],
    backup_extra_paths: &[],
};

pub static SHELL: Shell = Shell::join(IDENTITY, PORT);
