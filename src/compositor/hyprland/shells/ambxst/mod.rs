//! Ambxst on Hyprland.
//!
//! Only what is specific to this compositor, and here there is very little: the
//! config location is unknown, so there is nothing to back up and nothing to seed.
//! That is why `Port::config_dir` is `None`, and it is why the switch refuses it —
//! see `docs/SAFETY.md`, rule 4.
//!
//! - **start**: a direct process, `ambxst`.
//! - **clean**: nothing. With no `config_dir` there is nothing to remove, which
//!   is the same gap that makes the switch refuse it — see `docs/SAFETY.md`,
//!   rule 4.
//! - **verify**: a process named `ambxst` is alive.

use crate::shell::{Port, Shell, StartMechanism};
use crate::shells::ambxst::IDENTITY;

const PORT: Port = Port {
    config_dir: None,
    state_dir: None,
    systemd_user_unit: None,
    mechanism: StartMechanism::DirectProcess,
    skel_extra_paths: &[],
    backup_extra_paths: &[],
};

pub static SHELL: Shell = Shell::join(IDENTITY, PORT);
