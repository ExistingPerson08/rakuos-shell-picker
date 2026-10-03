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
    systemd_user_unit: None,
    mechanism: StartMechanism::StartEvent,
    health_socket: None,
    logo: Some("celestia.svg"),
};
