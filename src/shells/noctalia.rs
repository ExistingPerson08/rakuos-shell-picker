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
    install_plan: None,
    // Never. It is part of the base image, so there is nothing to install and
    // nothing that may be removed: `rum remove` would refuse anyway, and a plan
    // that always fails is worse than no plan. Its config and state are still
    // cleaned on a switch away, because those are the user's files, not the
    // image's.
    uninstall_plan: None,
    companion_systemd_user_units: &[],
    logo: Some("noctalia.svg"),
};
