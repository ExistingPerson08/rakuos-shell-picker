//! Ambxst on Niri.
//!
//! Only what is specific to this compositor, and as on Hyprland there is very
//! little: the config location is unknown, so the switch refuses it.
//!
//! Ambxst gained official Niri support in 1.3.6. Its Niri config is generated into
//! `~/.local/share/ambxst/niri.kdl` and pulled in by an include block, which is
//! probably why no config directory is recorded here — there may be nothing
//! hand-written to point at even once that is established.

use crate::shell::{Port, Shell, StartMechanism};
use crate::shells::ambxst::IDENTITY;

const PORT: Port = Port {
    config_dir: None,
    state_dir: None,
    systemd_user_unit: None,
    mechanism: StartMechanism::DirectProcess,
    skel_extra_paths: &[],
    backup_extra_paths: &[],
    health_socket: None,
};

pub static SHELL: Shell = Shell::join(IDENTITY, PORT);
