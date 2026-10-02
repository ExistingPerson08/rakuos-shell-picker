use crate::shell::{Shell, StartMechanism};

pub static SHELL: Shell = Shell {
    id: "ambxst",
    display_name: "Ambxst",
    upstream: "Axenide/Ambxst",
    package: None,
    binary: "ambxst",
    start_argv: &["ambxst"],
    config_dir: None,
    systemd_user_unit: None,
    mechanism: StartMechanism::DirectProcess,
    health_socket: None,
    logo: Some("ambxst-logo-color.svg"),
};
