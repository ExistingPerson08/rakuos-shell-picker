//! Ambxst, started as a plain process.
//!
//! What is the same wherever it runs: no package, so nothing to install and
//! nothing to uninstall, and a direct process. It is not in any repository, so
//! the only way it exists on a machine is that somebody put it there — which is
//! why it declares a package of `None` and no install plan. A switch to it is
//! only possible when it is already present (its binary *and* its config
//! directory are on disk), and `switch::prepare` refuses it otherwise rather
//! than taking a backup for a shell that could not be started.
//!
//! It shares the `quickshell` runtime the other QML shells are built on, so a
//! switch to it must keep that runtime back — see `runtime_packages`.
//!
//! Upstream note, for whoever adds a Niri port: Ambxst generates its Niri config
//! into `~/.local/share/ambxst/niri.kdl` and pulls it in with an include block,
//! so there may be no hand-written config directory to point at. It would not be
//! the Hyprland port with a different path.

use crate::shell::Identity;

pub const IDENTITY: Identity = Identity {
    id: "ambxst",
    display_name: "Ambxst",
    upstream: "Axenide/Ambxst",
    package: None,
    binary: "ambxst",
    start_argv: &["ambxst"],
    process_match: &[],
    install_plan: None,
    uninstall_plan: None,
    companion_systemd_user_units: &[],
    // No companions, so nothing of theirs to save or bring back.
    companion_paths: &[],
    // Written by Ambxst itself on every start. Ambxst's own Qt template
    // (`modules/theme/QtCtGenerator.qml`) renders the `[ColorScheme]` palette
    // into both of these; the skel ships them so a fresh machine has the colours
    // before the shell has run once, and `clean` takes them away again once this
    // switch's backup reproduces them. Both sit in `~/.config/qt5ct` and
    // `~/.config/qt6ct`, which `switch::SHARED_PATHS` backs up.
    generated_paths: &[
        ".config/qt5ct/colors/ambxst.colors",
        ".config/qt6ct/colors/ambxst.colors",
    ],
    // Rendered output and cached state, none of it written by the user: colour
    // schemes, wallpapers, usage counters and icon caches Ambxst rebuilds on the
    // next start. They live under `~/.cache`, which no backup walks —
    // `every_discard_path_is_never_preserved` fails the suite if that ever
    // changes.
    discard_paths: &[".cache/ambxst"],
    runtime_packages: &["quickshell", "quickshell-git"],
    logo: Some("ambxst-logo-color.svg"),
};
