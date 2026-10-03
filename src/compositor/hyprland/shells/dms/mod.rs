//! DMS, and the only shell here that is not already in the base image.
//!
//! What this shell owns:
//!
//! - **backup**: `~/.config/DankMaterialShell`. No state directory is recorded,
//!   because whether DMS keeps one has not been established. A rollback
//!   therefore restores the config and nothing more, which is the honest limit
//!   rather than a guess.
//! - **install**: two privileged stages, `pkexec rum install -y quickshell-git`
//!   and then `pkexec rum install -y dms dms-cli dgop danksearch
//!   dankcalendar-git matugen`, from the `rakuos` repository. Caelestia and Ambxst
//!   are in no repository, so this is the only install path that can currently be
//!   exercised. `quickshell-git` is named explicitly rather than left to the
//!   solver: `dms` requires `(quickshell or quickshell-git)` and the same
//!   repository also carries `noctalia-qs`, which provides `quickshell` as well.
//!   That is the legacy Noctalia build, required only by the obsolete
//!   `noctalia-legacy` package; current Noctalia 5 is C++ and does not use
//!   quickshell at all. `dankcalendar-git` is named for the same reason even
//!   though nothing requires it: dms calls it from `quickshell/Modules/DankDash`
//!   and the calendar widget is simply broken without it, which no rpm
//!   dependency would have revealed.
//! - **start**: a systemd user unit. Confirmed present in the package at
//!   `/usr/lib/systemd/user/dms.service`, so starting means
//!   `systemctl --user start dms` and not a bare `dms run`.
//! - **verify**: a process named `dms` is alive.

use crate::shell::{InstallPlan, Shell, StartMechanism};

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
            // The runtime first and on its own: it is small, it is the piece
            // most likely to fail on its own, and if it does there is then
            // nothing of dms installed.
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
    // DMS's runtime state, its theme tweaks, and one launcher entry.
    //
    // That last one is how the Noctalia launcher is hidden while DMS is the shell: a
    // single file is replaced, so the rest of `~/.local/share/applications` is left
    // alone. The Noctalia skel carries the same path without `NoDisplay`, so switching
    // back brings the launcher back instead of leaving it hidden.
    //
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
        ".local/share/applications/dev.noctalia.Noctalia.desktop",
    ],
    mechanism: StartMechanism::SystemdUserUnit,
    health_socket: None,
    logo: Some("danklogo2.svg"),
};
