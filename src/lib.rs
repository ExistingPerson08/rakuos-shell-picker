//! rakuos-shell-picker — choose the desktop shell running on top of a
//! compositor.
//!
//! Layout, and where each kind of change belongs:
//!
//! ```text
//! src/main.rs      entry point only
//! src/app.rs       GTK/libadwaita application and window
//! src/ui/          widgets, one file per part; knows no shell or compositor
//!   mod.rs         the "index": builds the page, owns selection state
//!   tabbar.rs  grid.rs  card.rs  logo.rs  log.rs
//! src/shells/      one file per shell: what that shell IS, on any compositor
//!   noctalia.rs  dms.rs  caelestia.rs  ambxst.rs
//! src/compositor/  one folder per compositor
//!   mod.rs         the Compositor trait and the tab registry
//!   hyprland/      Hyprland-specific tooling
//!     shells/      one folder per shell: everything about it ON HYPRLAND
//!       mod.rs     the card order
//!       dms/
//!         mod.rs   PORT — where this shell writes, what gets saved
//!         skel/    the files a switch installs
//!   niri/          Niri-specific tooling, same shape
//! src/shell.rs     the Shell type: Identity + Port, and how to inspect one
//! src/logging.rs   channel 1: the application log file
//! src/session.rs   active/previous shell state under XDG
//! src/backup.rs    verified copy of the config a switch will replace
//! src/switch.rs    the ordered steps of a switch, and what they touch
//! ```
//!
//! The rule of thumb: adding a shell touches its file in `shells/` plus one folder
//! per compositor that offers it. Adding a compositor touches only that
//! compositor's folder. Neither touches `ui/`.
//!
//! Each `shells/<shell>/` folder is deliberately self-contained — the `PORT` that
//! says where this shell's files go and which of them a switch would destroy, and
//! the `skel/` with the files themselves. A change to how one shell behaves on one
//! compositor stays inside that folder and cannot reach any other shell.

pub mod app;
pub mod apply;
pub mod backup;
pub mod clock;
pub mod compositor;
pub mod logging;
pub mod session;
pub mod shell;
pub mod shells;
pub mod switch;
pub mod ui;

#[cfg(test)]
#[path = "tests/lib_registry.rs"]
mod registry_tests;
