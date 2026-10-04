//! Caelestia Shell, a quickshell shell that no repository carries.
//!
//! What is the same wherever it runs: it has no package at all, so there is nothing
//! for the picker to install, and it runs on `quickshell` — the same runtime
//! `noctalia-qs` installs — which is why `Shell::installed` will not accept a live
//! `quickshell` process as proof that Caelestia is set up.
//!
//! It has no official Niri port: upstream ships a Hyprland shell, and the Niri
//! versions that exist are third party forks. Its absence from the Niri registry is
//! therefore deliberate and is asserted by a test.
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
    companion_systemd_user_units: &[],
    logo: Some("caelestia.svg"),
};
