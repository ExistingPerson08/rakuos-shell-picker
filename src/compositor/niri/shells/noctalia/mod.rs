//! Noctalia on Niri.
//!
//! Only what is specific to this compositor. The package, the binary and the base
//! image fact are in [`crate::shells::noctalia`].
//!
//! - **start**: Niri runs it from `spawn-at-startup "noctalia"` in `config.kdl`,
//!   so [`StartMechanism::SpawnAtStartup`] rather than `Autostart`. It is kept apart
//!   because the backend has to write a different syntax into a different file.
//! - **backup**: same two directories as on Hyprland.
//!
//! `--daemon` matters here and not on Hyprland: Niri waits for a startup command to
//! return before continuing, so a foreground shell would stall the session.
//!
//! There is no Niri skel yet, so a switch on Niri is refused by `apply::plan` with a
//! message saying the compositor config is missing — which is the honest answer
//! rather than a switch that quietly leaves the old configuration in place.

use crate::shell::{Port, Shell, StartMechanism};
use crate::shells::noctalia::IDENTITY;

const PORT: Port = Port {
    config_dir: Some(".config/noctalia"),
    state_dir: Some(".local/state/noctalia"),
    systemd_user_unit: None,
    mechanism: StartMechanism::SpawnAtStartup,
    skel_extra_paths: &[],
    backup_extra_paths: &[],
};

pub static SHELL: Shell = Shell::join(IDENTITY, PORT);
