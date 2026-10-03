//! Noctalia, started by the compositor's own autostart.
//!
//! What this shell owns:
//!
//! - **backup**: `~/.config/noctalia` and `~/.local/state/noctalia`. Most of a
//!   Noctalia setup lives in the second one: 389 of the 445 files a real backup
//!   takes. Backing up only the config directory would throw most of it away.
//! - **install**: none. It is part of the RakuOS base image, so `rum origin`
//!   answers `Base`. There is nothing to install, and a switch must never try
//!   to remove it either.
//! - **start**: the compositor runs it from `config/autostart.lua`. Switching to
//!   Noctalia means starting the process; switching away means ending it.
//! - **verify**: a process named `noctalia` is alive. On this system it is a
//!   native binary at `/usr/bin/noctalia`, not a quickshell process, so the
//!   process name is unambiguous.

use crate::shell::{Shell, StartMechanism};

pub static SHELL: Shell = Shell {
    id: "noctalia",
    display_name: "Noctalia",
    upstream: "noctalia-shell",
    package: Some("noctalia"),
    binary: "noctalia",
    start_argv: &["noctalia"],
    config_dir: Some(".config/noctalia"),
    state_dir: Some(".local/state/noctalia"),
    install_plan: None,
    systemd_user_unit: None,
    companion_systemd_user_units: &[],
    // `config_dir` is handled separately, since it comes from a backup when one holds
    // it. The launcher entry is the counterpart to the one the DMS skel replaces:
    // without it, switching back would leave Noctalia installed and invisible.
    //
    // Theme tweaks are listed as individual files, not as `~/.config/gtk-3.0` or
    // `~/.config/gtk-4.0`. Those two directories are shared with every other GTK
    // program on the desktop, and the swap replaces a path outright rather than
    // merging into it, so listing the directory would take the user's own
    // stylesheets with it. Noctalia's own stylesheets are listed by name so they come back
    // with the launcher rather than being left to chance.
    skel_extra_paths: &[
        ".config/gtk-3.0/noctalia.css",
        ".config/gtk-3.0/settings.ini",
        ".config/gtk-4.0/noctalia.css",
        ".config/gtk-4.0/settings.ini",
        ".config/gtk-4.0/gtk.css",
        ".local/share/applications/dev.noctalia.Noctalia.desktop",
    ],
    mechanism: StartMechanism::Autostart,
    health_socket: None,
    logo: Some("noctalia.svg"),
};
