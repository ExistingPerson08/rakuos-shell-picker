//! Noctalia under Niri.
//!
//! Same backup and install facts as the Hyprland page: `~/.config/noctalia` plus
//! `~/.local/state/noctalia`, and nothing to install because it is in the base
//! image. Only the start differs: Niri runs it from `spawn-at-startup` rather
//! than from a Lua autostart file.

use crate::shell::{Shell, StartMechanism};

/// Niri runs this with `spawn-at-startup "noctalia"` in `config.kdl`.
/// `--daemon` exists because Niri waits for a startup command to return before
/// continuing, so a foreground shell would stall the session.
pub static SHELL: Shell = Shell {
    id: "noctalia",
    display_name: "Noctalia",
    upstream: "noctalia-shell",
    package: Some("noctalia"),
    binary: "noctalia",
    start_argv: &["noctalia"],
    config_dir: Some(".config/noctalia"),
    state_dir: Some(".local/state/noctalia"),
    install_plan: None,
    systemd_user_unit: None,
    mechanism: StartMechanism::SpawnAtStartup,
    health_socket: None,
    logo: Some("noctalia.svg"),
};
