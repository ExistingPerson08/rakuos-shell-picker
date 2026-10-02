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
//! src/compositor/  one folder per compositor
//!   mod.rs         the Compositor trait and the tab registry
//!   hyprland/      Hyprland-specific tooling
//!     shells/      one file per shell, this compositor's registry
//!       noctalia.rs  dms.rs  caelestia.rs  ambxst.rs
//!   niri/          Niri-specific tooling
//! src/shell.rs     the Shell type and how to inspect one
//! src/session.rs   active/previous shell state under XDG
//! ```
//!
//! The rule of thumb: adding a shell touches only that shell's file. Adding a
//! compositor touches only that compositor's folder. Neither touches `ui/`.

pub mod app;
pub mod compositor;
pub mod session;
pub mod shell;
pub mod ui;

#[cfg(test)]
mod registry_tests {
    use std::collections::HashSet;
    use std::path::PathBuf;

    use super::compositor;

    fn assets() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")
    }

    #[test]
    fn every_compositor_id_is_unique() {
        let ids: HashSet<_> = compositor::all().iter().map(|c| c.id()).collect();
        assert_eq!(ids.len(), compositor::all().len());
    }

    #[test]
    fn every_compositor_has_a_unique_id_and_label() {
        for a in compositor::all() {
            assert!(!a.id().is_empty());
            assert!(!a.label().is_empty());
        }
    }

    #[test]
    fn shell_ids_are_unique_across_compositors() {
        let mut seen = HashSet::new();
        for compositor in compositor::all() {
            for shell in compositor.shells() {
                assert!(seen.insert(shell.id), "duplicate shell id: {}", shell.id);
            }
        }
    }

    #[test]
    fn every_registered_shell_has_its_logo_on_disk() {
        for compositor in compositor::all() {
            for shell in compositor.shells() {
                let file = shell
                    .logo
                    .unwrap_or_else(|| panic!("{} has no logo", shell.id));
                let path = assets().join(file);
                assert!(
                    path.is_file(),
                    "missing logo for {}: {}",
                    shell.id,
                    path.display()
                );
            }
        }
    }

    #[test]
    fn hyprland_has_its_four_shells_and_niri_is_still_empty() {
        let hyprland = compositor::all()
            .into_iter()
            .find(|c| c.id() == "hyprland")
            .expect("hyprland registered");
        assert_eq!(
            hyprland.shells().iter().map(|s| s.id).collect::<Vec<_>>(),
            ["noctalia", "dms", "caelestia", "ambxst"]
        );

        let niri = compositor::all()
            .into_iter()
            .find(|c| c.id() == "niri")
            .expect("niri registered");
        assert!(
            niri.shells().is_empty(),
            "niri has shells now, drop the placeholder"
        );
    }

    #[test]
    fn unknown_id_does_not_resolve() {
        assert!(super::shell::find("does-not-exist").is_none());
    }
}
