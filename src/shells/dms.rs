//! DMS (DankMaterialShell), and the only shell here that is not already in the
//! base image.
//!
//! What is the same wherever it runs:
//!
//! - **install**: two privileged stages, in this order:
//!
//!   ```text
//!   1. quickshell-git
//!   2. dms dms-cli dgop danksearch dankcalendar-git matugen
//!   ```
//!
//!   Both from the `rakuos` repository. Caelestia and Ambxst are in no
//!   repository, so this is the only install path that can currently be exercised.
//!
//!   `quickshell-git` is named explicitly rather than left to the solver: `dms`
//!   requires `(quickshell or quickshell-git)` and the same repository also carries
//!   `noctalia-qs`, which provides `quickshell` as well. That is the legacy
//!   Noctalia build, required only by the obsolete `noctalia-legacy` package;
//!   current Noctalia 5 is C++ and does not use quickshell at all.
//!
//!   `dankcalendar-git` is named for a different reason: nothing requires it. DMS
//!   calls it from `quickshell/Modules/DankDash`, so the calendar widget is simply
//!   broken without it, and no rpm dependency would ever have said so.
//!
//!   The runtime is a stage of its own because that ordering is a safety property,
//!   not a convenience: if the runtime fails there is then nothing of DMS on the
//!   machine at all. One transaction would take the shell's own packages down with
//!   it for no benefit.
//!
//! - **start**: a systemd user unit. Confirmed present in the package at
//!   `/usr/lib/systemd/user/dms.service`, so starting means
//!   `systemctl --user start dms` and not a bare `dms run`.
//! - **`dcal` first**: it supplies the calendar widgets DMS draws. Nothing makes
//!   DMS wait for it, and DMS looks merely incomplete without it, so it is declared
//!   rather than left to the skel autostart, which only runs on the next login. The
//!   live switch starts it in the current session too.
//!
//! What differs per compositor is the [`Port`]. DMS's own config directory is the
//! same on both; the Hyprland page additionally carries the launcher entry that
//! hides Noctalia while DMS is the shell.
//!
//! No state directory is recorded, because whether DMS keeps one has not been
//! established. A rollback therefore restores the config and nothing more, which
//! is the honest limit rather than a guess.

use crate::shell::{Identity, InstallPlan};

pub const IDENTITY: Identity = Identity {
    id: "dms",
    display_name: "DankMaterialShell",
    upstream: "AvengeMedia/DankMaterialShell",
    package: Some("dms"),
    binary: "dms",
    start_argv: &["dms", "run"],
    install_plan: Some(InstallPlan {
        package: "dms",
        stages: &[
            // The runtime first and on its own: it is small, it is the piece most
            // likely to fail on its own, and if it does there is then nothing of
            // dms installed.
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
    companion_systemd_user_units: &["dcal"],
    logo: Some("danklogo2.svg"),
};
