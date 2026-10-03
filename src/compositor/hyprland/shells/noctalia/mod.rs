//! Noctalia, started by the compositor's own autostart.
//!
//! What this shell owns:
//!
//! - **backup**: `~/.config/noctalia` and `~/.local/state/noctalia`. Most of a
//!   Noctalia setup lives in the second one: 389 of the 445 files a real backup
//!   takes. Backing up only the config directory would throw most of it away.
//! - **install**: none. It is part of the RakuOS base image, so `rum origin`
//!   answers `Base`. There is nothing to install, and a switch must never try
//!   to remove it either.
//! - **start**: the compositor runs it from `config/autostart.lua`. Switching to
//!   Noctalia means starting the process; switching away means ending it.
//! - **verify**: a process named `noctalia` is alive. On this system it is a
//!   native binary at `/usr/bin/noctalia`, not a quickshell process, so the
//!   process name is unambiguous.

use crate::shell::{Shell, StartMechanism};

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
    mechanism: StartMechanism::Autostart,
    health_socket: None,
    logo: Some("noctalia.svg"),
};
