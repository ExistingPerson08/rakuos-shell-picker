//! Caelestia, a quickshell shell that no repository carries.
//!
//! What this shell owns:
//!
//! - **backup**: `~/.config/quickshell/caelestia`, once it exists.
//! - **install**: none. Nothing in `rakuos`, `fedora` or `updates` has it, so
//!   there is nothing for the picker to install and the card is refused rather
//!   than offered.
//! - **start**: the compositor's start event runs `quickshell -c caelestia`.
//! - **verify**: not settled, and deliberately left unresolved. `quickshell` is
//!   a shared runtime that noctalia-qs also installs, so a live process with
//!   that name proves nothing about which shell it is. This needs a real probe
//!   before a switch to Caelestia could be trusted, which is one reason it is
//!   refused today.

use crate::shell::{Shell, StartMechanism};

pub static SHELL: Shell = Shell {
    id: "caelestia",
    display_name: "Caelestia Shell",
    upstream: "caelestia-dots/shell",
    package: None,
    binary: "caelestia",
    start_argv: &["caelestia", "shell", "-d"],
    config_dir: Some(".config/quickshell/caelestia"),
    state_dir: None,
    install_plan: None,
    systemd_user_unit: None,
    mechanism: StartMechanism::StartEvent,
    health_socket: None,
    logo: Some("celestia.svg"),
};
