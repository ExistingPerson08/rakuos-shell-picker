use crate::shell::{Shell, StartMechanism};

pub static SHELL: Shell = Shell {
    id: "dms",
    display_name: "DankMaterialShell",
    upstream: "AvengeMedia/DankMaterialShell",
    package: Some("dms"),
    binary: "dms",
    start_argv: &["dms", "run"],
    config_dir: Some(".config/DankMaterialShell"),
    state_dir: None,
    systemd_user_unit: Some("dms"),
    mechanism: StartMechanism::SystemdUserUnit,
    health_socket: None,
    logo: Some("danklogo2.svg"),
};
