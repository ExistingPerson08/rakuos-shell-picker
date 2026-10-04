//! Niri, deliberately empty.
//!
//! Niri was registered here with three shell ports and no skel. It has been
//! removed, and this file is what is left: the shape of the work, written down
//! while it is still cheap to be wrong about.
//!
//! **Why it was removed rather than left registered.** The three ports were real
//! and the registry was honest about them — but every card on the page refused,
//! because `skel/` held no files at all and a switch with nothing to install has
//! nothing to switch to. A registered compositor promises a working page. The
//! Niri tab showed three shells and could complete none of them, which is worse
//! than not offering Niri at all: it looked like a bug in the picker rather than
//! a decision about Niri.
//!
//! **What bringing it back involves**, in the order the work actually goes:
//!
//! 1. `skel/`, with `config.kdl` and the theme files a Niri session needs. Until
//!    that exists, `apply` refuses every shell on the compositor — see the
//!    `a_compositor_with_no_skel_yet_refuses_every_shell_on_it` test, which now
//!    covers the rule through a compositor that is registered nowhere.
//! 2. A `Niri` struct implementing [`crate::compositor::Compositor`]: id `niri`,
//!    label `niri`, `config_dir` `.config/niri`, and `marker_env` `NIRI_SOCKET`.
//!    Detection needs more than the variable: a copied-but-unexpanded
//!    `NIRI_SOCKET` looks like a live session and is not one, and the socket path
//!    is the signal itself, so it has to be checked for being a real socket. That
//!    check is [`crate::compositor::socket_alive`] and it is still here.
//! 3. `shells/<shell>/mod.rs` per shell, each a `Port`. The three that were here
//!    are in the history of this file; the answers that differ from Hyprland are
//!    Noctalia's `StartMechanism::SpawnAtStartup` and the absence of a launcher
//!    entry, because there is no Noctalia-on-Hyprland launcher to hide.
//! 4. Adding it to [`crate::compositor::all`], which is the only registry the UI
//!    walks. Nothing in the UI changes.
//!
//! **What was kept, and why.** The cross-compositor refusal and its tests stay in
//! [`crate::ui::dialog`], and `StartMechanism::SpawnAtStartup` stays in
//! [`crate::shell`]. With one compositor registered neither can be reached from
//! the running app, but both are knowledge about how to do this correctly, and
//! re-deriving them when the next compositor arrives would be the expensive way
//! to learn them again.
