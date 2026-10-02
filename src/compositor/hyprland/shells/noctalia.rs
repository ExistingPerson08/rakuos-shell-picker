use crate::shell::{Shell, StartMechanism};

pub static SHELL: Shell = Shell {
    id: "noctalia",
    display_name: "Noctalia",
    upstream: "noctalia-shell",
    package: Some("noctalia"),
    binary: "noctalia",
    start_argv: &["noctalia"],
    config_dir: Some(".config/noctalia"),
    systemd_user_unit: None,
    mechanism: StartMechanism::Autostart,
    health_socket: None,
    logo: Some("noctalia.svg"),
};
