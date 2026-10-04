//! DMS on Niri.
//!
//! Only what is specific to this compositor. The two install stages, the seven
//! packages and the `dcal` companion are in [`crate::shells::dms`].
//!
//! DankMaterialShell lists Niri first among its supported compositors, and it keeps
//! its own systemd user unit on either one — so unlike Noctalia, nothing about the
//! start mechanism changes here.
//!
//! There is no launcher entry here, unlike on Hyprland: there is no
//! Noctalia-on-Hyprland launcher to undo on this compositor.

use crate::shell::{Port, Shell, StartMechanism};
use crate::shells::dms::IDENTITY;

const PORT: Port = Port {
    config_dir: Some(".config/DankMaterialShell"),
    state_dir: None,
    systemd_user_unit: Some("dms"),
    mechanism: StartMechanism::SystemdUserUnit,
    skel_extra_paths: &[
        ".config/gtk-3.0/settings.ini",
        ".config/gtk-4.0/settings.ini",
        ".config/gtk-4.0/gtk.css",
        ".local/state/DankMaterialShell",
    ],
    backup_extra_paths: &[".local/state/DankMaterialShell"],
};

pub static SHELL: Shell = Shell::join(IDENTITY, PORT);
