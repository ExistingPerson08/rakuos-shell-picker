//! Caelestia Shell, a quickshell shell that no repository carries.
//!
//! - **install**: none. It has no package at all, so there is nothing for the
//!   picker to install and nothing to uninstall afterwards, and that is one of
//!   the reasons the card is refused. It runs on `quickshell` — the same runtime
//!   `noctalia-qs` installs — which is why `Shell::installed` will not accept a
//!   live `quickshell` process as proof that Caelestia is set up.
//!
//! It has no official Niri port: upstream ships a Hyprland shell, and the Niri
//! versions that exist are third party forks. That was why it was left out of the
//! Niri registry, which `compositor::niri` records for whoever brings it back.
//!
//! Verification is not settled either, and is deliberately left unresolved here.
//! `quickshell` is a shared runtime, so a live process with that name proves
//! nothing about which shell it is. That needs a real probe before a switch to
//! Caelestia could be trusted, and it is one of the reasons it is refused today.
//!
//! What differs per compositor is the [`Port`].

use crate::shell::Identity;

pub const IDENTITY: Identity = Identity {
    id: "caelestia",
    display_name: "Caelestia Shell",
    upstream: "caelestia-dots/shell",
    package: None,
    binary: "caelestia",
    start_argv: &["caelestia", "shell", "-d"],
    install_plan: None,
    uninstall_plan: None,
    companion_systemd_user_units: &[],
    generated_paths: &[],
    logo: Some("caelestia.svg"),
};
