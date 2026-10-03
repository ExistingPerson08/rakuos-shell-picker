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
    // `dcal` supplies the calendar widgets DMS draws. Nothing makes DMS wait for it,
    // and DMS looks merely incomplete without it, so it is declared here rather than
    // left to the skel autostart, which only runs on the next login. The live switch
    // starts it in the current session too.
    companion_systemd_user_units: &["dcal"],
    // Theme tweaks are listed as individual files, not as `~/.config/gtk-3.0` or
    // `~/.config/gtk-4.0`. Those two directories are shared with every other GTK
    // program on the desktop, and the swap replaces a path outright rather than
    // merging into it, so listing the directory would take the user's own
    // stylesheets with it. The DMS skel has no `gtk-3.0/gtk.css`, so that one is
    // deliberately left as the user has it.
    skel_extra_paths: &[
        ".config/gtk-3.0/settings.ini",
        ".config/gtk-4.0/settings.ini",
        ".config/gtk-4.0/gtk.css",
        ".local/state/DankMaterialShell",
    ],
    mechanism: StartMechanism::SystemdUserUnit,
    health_socket: None,
    logo: Some("danklogo2.svg"),
};
