use crate::shell::{Shell, StartMechanism};

/// Ambxst gained official Niri support in 1.3.6. Its Niri config is generated
/// into `~/.local/share/ambxst/niri.kdl` and pulled in by an include block, so
/// there is no hand-written config directory to point at.
pub static SHELL: Shell = Shell {
    id: "ambxst",
    display_name: "Ambxst",
    upstream: "Axenide/Ambxst",
    package: None,
    binary: "ambxst",
    start_argv: &["ambxst"],
    config_dir: None,
    state_dir: None,
    systemd_user_unit: None,
    mechanism: StartMechanism::DirectProcess,
    health_socket: None,
    logo: Some("ambxst-logo-color.svg"),
};
