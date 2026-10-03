//! Ambxst, started as a plain process.
//!
//! What this shell owns:
//!
//! - **backup**: nothing, because no config directory is recorded.
//! - **install**: none; no repository carries it.
//! - **start**: a direct process, `ambxst`.
//! - **verify**: a process named `ambxst` is alive.
//!
//! Refused at preflight, and the missing config location is why: with nowhere
//! recorded for the config there is nothing to back up and nothing to seed, so a
//! switch could neither be undone nor completed.

use crate::shell::{Shell, StartMechanism};

pub static SHELL: Shell = Shell {
    id: "ambxst",
    display_name: "Ambxst",
    upstream: "Axenide/Ambxst",
    package: None,
    binary: "ambxst",
    start_argv: &["ambxst"],
    config_dir: None,
    state_dir: None,
    install_plan: None,
    systemd_user_unit: None,
    companion_systemd_user_units: &[],
    skel_extra_paths: &[],
    mechanism: StartMechanism::DirectProcess,
    health_socket: None,
    logo: Some("ambxst-logo-color.svg"),
};
