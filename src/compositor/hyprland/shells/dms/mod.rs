//! DMS on Hyprland.
//!
//! Only what is specific to this compositor. The package list, the two install
//! stages, the `dcal` companion and the systemd unit are in
//! [`crate::shells::dms`], and are not repeated here.
//!
//! - **start**: `systemctl --user start dms`, for the unit shipped in the package.
//! - **verify**: a process named `dms` is alive.
//!
//! The Hyprland page also carries the Noctalia launcher entry with `NoDisplay`, the
//! theme files, and DMS's runtime state directory. That last launcher entry is how
//! the Noctalia launcher is hidden while DMS is the shell: a single file is
//! replaced, so the rest of `~/.local/share/applications` is left alone. The
//! Noctalia skel carries the same path without `NoDisplay`, so switching back brings
//! the launcher back instead of leaving it hidden.
//!
//! The DMS skel has no `gtk-3.0/gtk.css`, so that one is deliberately left as the
//! user has it.

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
        ".local/share/applications/dev.noctalia.Noctalia.desktop",
    ],
    backup_extra_paths: &[
        ".local/state/DankMaterialShell",
        ".local/share/applications/dev.noctalia.Noctalia.desktop",
    ],
};

pub static SHELL: Shell = Shell::join(IDENTITY, PORT);
