//! DMS under Niri.
//!
//! Same backup and install facts as the Hyprland page. DMS keeps its own systemd
//! user unit on either compositor, so the start mechanism does not change.

use crate::shell::{InstallPlan, Shell, StartMechanism};

/// DankMaterialShell lists niri first among its supported compositors.
pub static SHELL: Shell = Shell {
    id: "dms",
    display_name: "DankMaterialShell",
    upstream: "AvengeMedia/DankMaterialShell",
    package: Some("dms"),
    binary: "dms",
    start_argv: &["dms", "run"],
    config_dir: Some(".config/DankMaterialShell"),
    state_dir: None,
    install_plan: Some(InstallPlan {
        package: "dms",
        stages: &[
            &["quickshell-git"],
            &[
                "dms",
                "dms-cli",
                "dgop",
                "danksearch",
                "dankcalendar-git",
                "matugen",
            ],
        ],
        program: "pkexec",
        args: &["rum", "install", "-y"],
    }),
    systemd_user_unit: Some("dms"),
    mechanism: StartMechanism::SystemdUserUnit,
    health_socket: None,
    logo: Some("danklogo2.svg"),
};
