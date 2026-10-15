//! DMS on Niri.
//!
//! Only what is specific to this compositor. The package list, the two install
//! stages, the `dcal` companion and the systemd unit are in
//! [`crate::shells::dms`], and are not repeated here.
//!
//! - **start**: `systemctl --user start dms`, the same unit Hyprland uses. The
//!   `rakuos-niri` image enables it globally (`systemctl --global enable dms`), so
//!   it also comes up on its own at login and a switch reaching for it again is
//!   harmless.
//! - **clean**: `~/.config/DankMaterialShell` and
//!   `~/.local/state/DankMaterialShell`, the same two the backup takes.
//! - **verify**: a process named `dms` is alive.
//!
//! The Niri skel is the image's own `etc/skel` — the compositor config under
//! `~/.config/niri/dms/`, the shell's config, and its first-run state. It carries
//! no theme file and no launcher entry, unlike the Hyprland port: nothing here
//! hides another shell, because on Niri it is the Noctalia skel that owns those
//! shared paths. That is why both `skel_extra_paths` and `backup_extra_paths` are
//! empty — the compositor config, `config_dir` and `state_dir` cover every file it
//! ships.

use crate::shell::{Port, Shell, StartMechanism};
use crate::shells::dms::IDENTITY;

const PORT: Port = Port {
    config_dir: Some(".config/DankMaterialShell"),
    state_dir: Some(".local/state/DankMaterialShell"),
    systemd_user_unit: Some("dms"),
    mechanism: StartMechanism::SystemdUserUnit,
    skel_extra_paths: &[],
    backup_extra_paths: &[],
};

pub static SHELL: Shell = Shell::join(IDENTITY, PORT);
