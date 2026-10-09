//! Noctalia — a native C++ shell, and the one in the RakuOS base image.
//!
//! What is the same wherever it runs:
//!
//! - **install**: none. It is part of the base image, so `rum origin` answers
//!   `Base`. There is nothing to install, and a switch must never try to remove
//!   it either — which is why its `uninstall_plan` is `None`. Its config and
//!   state are still cleaned when switching away, because those are the user's
//!   files rather than the image's.
//! - **binary**: `/usr/bin/noctalia`, a native binary rather than a quickshell
//!   process, so the process name is unambiguous.
//! - **logo**: `noctalia.svg`.
//!
//! What differs per compositor is the [`Port`]: Hyprland starts it from
//! `config/autostart.lua`, Niri from `spawn-at-startup`, and the Hyprland page also
//! carries the launcher entry and the theme files its skel replaces.
//!
//! Backup is worth spelling out, because it is easy to get wrong: most of a real
//! Noctalia setup lives in `~/.local/state/noctalia` — 389 of the 445 files a real
//! backup takes. Backing up only the config directory would throw most of it away.

use crate::shell::Identity;

pub const IDENTITY: Identity = Identity {
    id: "noctalia",
    display_name: "Noctalia",
    upstream: "noctalia-shell",
    package: Some("noctalia"),
    binary: "noctalia",
    start_argv: &["noctalia"],
    process_match: &[],
    install_plan: None,
    // Never. It is part of the base image, so there is nothing to install and
    // nothing that may be removed: `rum remove` would refuse anyway, and a plan
    // that always fails is worse than no plan. Its config and state are still
    // cleaned on a switch away, because those are the user's files, not the
    // image's.
    uninstall_plan: None,
    companion_systemd_user_units: &[],
    // No companions, so nothing of theirs to save or bring back.
    companion_paths: &[],
    // Rendered by Noctalia itself from its own templates, not installed by the
    // switch — `[theme.templates] builtin_ids = ["gtk3", "gtk4", "hyprland",
    // "qt"]` writes all four on every start. DMS uses none of them, so they are
    // what is left over after a switch to DMS, and `clean` takes them away again.
    //
    // The other files Noctalia can render are deliberately absent: they go to
    // `btop`, `cava`, `foot`, `ghostty` and the rest, and those templates are not
    // in `builtin_ids` on this machine. What is listed here is what actually
    // lands in a directory the backup already walks.
    generated_paths: &[
        ".config/gtk-3.0/noctalia.css",
        ".config/gtk-4.0/noctalia.css",
        ".config/qt5ct/colors/noctalia.conf",
        ".config/qt6ct/colors/noctalia.conf",
    ],
    // Only the cache: every theme Noctalia renders lands in one of the four
    // directories above, which are in `SHARED_PATHS` and therefore backed up
    // before `clean` touches them. What is left is a megabyte of log and
    // regenerated state under `.cache/noctalia`, which no backup should carry
    // and no switch has a use for afterwards.
    discard_paths: &[".cache/noctalia"],
    // Nothing. Noctalia 5 is a native C++ binary and needs no runtime the rest
    // of the desktop does not already have, so a switch to it keeps nothing back
    // — which is exactly why leaving DMS takes quickshell with it.
    runtime_packages: &[],
    logo: Some("noctalia.svg"),
};
