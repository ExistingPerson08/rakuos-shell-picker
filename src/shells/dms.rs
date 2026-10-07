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
//! - **uninstall**: one stage, `pkexec rum remove -y` over DMS's own packages and
//!   nothing else. It runs at the very end of a switch to something else, once
//!   the new shell has proved itself alive, and it deliberately leaves
//!   `quickshell-git` behind: that is a runtime rather than DMS, another shell
//!   can run on it, and taking it out from under the shell just switched to is
//!   the exact failure this list is here to prevent. An orphaned runtime stays a
//!   candidate for `rum autoremove`, which `docs/SAFETY.md` keeps as a separate
//!   explicit choice.
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
//! Its state directory is `~/.local/state/DankMaterialShell`, declared as
//! [`Port::state_dir`] rather than as one more extra path: it is DMS's own state
//! rather than something the skel hands out, so it belongs to the same question as
//! the config — saved before a switch, cleaned after one, and read back from the
//! newest backup on the way home.

use crate::shell::{Identity, InstallPlan, UninstallPlan};

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
                "khal",
            ],
        ],
        program: "pkexec",
        // `--refresh` because a cached copy of the metadata is only known to be
        // young, not known to be current. See `switch::install`.
        args: &["rum", "install", "-y", "--refresh"],
    }),
    uninstall_plan: Some(UninstallPlan {
        package: "dms",
        // DMS's own packages and nothing else. `quickshell-git` is deliberately
        // left out even though it was installed first: it is a runtime rather
        // than the shell, another shell can run on it, and taking it out from
        // under the shell just switched to is exactly the failure this list is
        // here to prevent. An orphaned runtime is a candidate for
        // `rum autoremove`, which SAFETY.md keeps as a separate, explicit choice
        // rather than something a switch does on its own.
        //
        // `rum remove` also refuses any package another package still requires,
        // so `matugen` in particular cannot be pulled out from under something
        // that uses it — the solver is the second line of defence, not the first.
        stages: &[&[
            "dms",
            "dms-cli",
            "dgop",
            "danksearch",
            "dankcalendar-git",
            "matugen",
            "khal",
        ]],
        program: "pkexec",
        args: &["rum", "remove", "-y"],
    }),
    companion_systemd_user_units: &["dcal"],
    generated_paths: &[],
    logo: Some("danklogo2.svg"),
};
