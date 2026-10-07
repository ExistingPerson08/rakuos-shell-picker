//! Ambxst, started as a plain process.
//!
//! What is the same wherever it runs: no package, so nothing to install and
//! nothing to uninstall, and a direct process.
//!
//! It records **no config directory at all**, which is the reason it is refused
//! today. With nowhere recorded for the config there is nothing to back up and
//! nothing to seed, so a switch could neither be undone nor completed — see
//! `docs/SAFETY.md`, rule 4. That refusal is a missing fact rather than a policy:
//! once the config location is established, it belongs in the [`Port`].
//!
//! Upstream note, for whoever adds a Niri port: Ambxst generates its Niri config
//! into `~/.local/share/ambxst/niri.kdl` and pulls it in with an include block, so
//! there may be no hand-written config directory to point at. It would not be the
//! Hyprland port with a different path.

use crate::shell::Identity;

pub const IDENTITY: Identity = Identity {
    id: "ambxst",
    display_name: "Ambxst",
    upstream: "Axenide/Ambxst",
    package: None,
    binary: "ambxst",
    start_argv: &["ambxst"],
    install_plan: None,
    uninstall_plan: None,
    companion_systemd_user_units: &[],
    generated_paths: &[],
    logo: Some("ambxst-logo-color.svg"),
};
