//! Noctalia on Niri.
//!
//! Only what is specific to this compositor. Everything about Noctalia itself —
//! its package, its binary, its install and removal plans — is in
//! [`crate::shells::noctalia`], and is not repeated here.
//!
//! - **start**: the compositor runs it from `spawn-at-startup "noctalia"` in the
//!   skel's `config.kdl`, so switching to Noctalia means starting the process and
//!   switching away means ending it. Niri waits for a startup command to return
//!   before it carries on, so the mechanism is
//!   [`StartMechanism::SpawnAtStartup`](crate::shell::StartMechanism::SpawnAtStartup)
//!   and the command is detached — a foreground shell here would stall the session.
//! - **backup**: `~/.config/noctalia` plus `~/.local/state/noctalia`, because most
//!   of a Noctalia setup lives in the second one.
//! - **clean**: those same two paths, plus the four rendered themes once the
//!   backup taken for that switch reproduces them byte for byte.
//! - **verify**: a process named `noctalia` is alive.
//!
//! The Niri skel is the image's own `etc/skel`: the compositor config under
//! `~/.config/niri/noctalia/` and the shell's `~/.config/noctalia/config.toml`.
//! Neither the four rendered themes nor the launcher entry the Hyprland skel
//! carries is here — those replace shared paths a Hyprland session has, and a
//! Niri session has no Noctalia-on-Hyprland launcher to hide. The compositor
//! config, `config_dir` and `state_dir` cover every file the skel ships, so both
//! extras lists are empty.

use crate::shell::{Port, Shell, StartMechanism};
use crate::shells::noctalia::IDENTITY;

const PORT: Port = Port {
    config_dir: Some(".config/noctalia"),
    state_dir: Some(".local/state/noctalia"),
    systemd_user_unit: None,
    mechanism: StartMechanism::SpawnAtStartup,
    skel_extra_paths: &[],
    backup_extra_paths: &[],
};

pub static SHELL: Shell = Shell::join(IDENTITY, PORT);
