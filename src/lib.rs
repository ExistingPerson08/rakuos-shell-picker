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
//! src/logging.rs   channel 1: the application log file
//! src/session.rs   active/previous shell state under XDG
//! src/backup.rs    verified copy of the config a switch will replace
//! src/switch.rs    the ordered steps of a switch, and what they touch
//! ```
//!
//! The rule of thumb: adding a shell touches only that shell's file. Adding a
//! compositor touches only that compositor's folder. Neither touches `ui/`.

pub mod app;
pub mod apply;
pub mod backup;
pub mod clock;
pub mod compositor;
pub mod logging;
pub mod session;
pub mod shell;
pub mod switch;
pub mod ui;

#[cfg(test)]
mod registry_tests {
    use std::collections::{BTreeSet, HashMap, HashSet};
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
    fn shell_ids_are_unique_within_a_compositor() {
        // Not across compositors: the same shell appears in more than one list
        // on purpose, because which shells exist is a property of the shell plus
        // the compositor. What must never happen is the same id twice on one
        // page, which would make two cards indistinguishable.
        for compositor in compositor::all() {
            let mut seen = HashSet::new();
            for shell in compositor.shells() {
                assert!(
                    seen.insert(shell.id),
                    "{}: duplicate shell id {}",
                    compositor.id(),
                    shell.id
                );
            }
        }
    }

    #[test]
    fn a_shell_id_always_refers_to_the_same_shell() {
        // The id is what gets written to the session file, so the same id must
        // not mean two different upstreams.
        let mut upstream: HashMap<&str, &str> = HashMap::new();
        for compositor in compositor::all() {
            for shell in compositor.shells() {
                match upstream.get(shell.id) {
                    Some(previous) => assert_eq!(
                        *previous,
                        shell.upstream,
                        "{}: id {} maps to two upstreams",
                        compositor.id(),
                        shell.id
                    ),
                    None => {
                        upstream.insert(shell.id, shell.upstream);
                    }
                }
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
    fn each_compositor_lists_the_shells_it_really_supports() {
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
        // Noctalia, DMS and Ambxst document Niri support upstream. Caelestia is
        // absent on purpose: only third party Niri forks exist, so putting it
        // here would promise something upstream does not ship.
        assert_eq!(
            niri.shells().iter().map(|s| s.id).collect::<Vec<_>>(),
            ["noctalia", "dms", "ambxst"]
        );
    }

    #[test]
    fn niri_does_not_claim_shells_it_has_no_port_for() {
        let niri = compositor::all()
            .into_iter()
            .find(|c| c.id() == "niri")
            .expect("niri registered");
        let ids: Vec<_> = niri.shells().iter().map(|s| s.id).collect();
        assert!(
            !ids.contains(&"caelestia"),
            "caelestia has no official niri port; only community forks"
        );
    }

    #[test]
    fn an_install_plan_names_the_same_package_as_the_shell() {
        // The availability check is done against Shell::package, and the install
        // is done with InstallPlan::package. If they drift apart the picker would
        // check one package and install another.
        for compositor in compositor::all() {
            for shell in compositor.shells() {
                if let Some(plan) = shell.install_plan {
                    assert_eq!(
                        shell.package,
                        Some(plan.package),
                        "{}: install plan installs {} but the shell is checked as {:?}",
                        shell.id,
                        plan.package,
                        shell.package
                    );
                }
            }
        }
    }

    #[test]
    fn only_dms_can_be_installed_and_it_asks_for_polkit() {
        // Deliberately a fact about today, not a general rule: when Caelestia
        // and Ambxst get packaged this test is what should be updated.
        let with_plan: BTreeSet<_> = compositor::all()
            .iter()
            .flat_map(|c| c.shells())
            .filter(|s| s.install_plan.is_some())
            .map(|s| s.id)
            .collect();
        assert_eq!(with_plan, BTreeSet::from(["dms"]));

        let dms = super::shell::find("dms").expect("dms registered");
        let plan = dms.install_plan.expect("dms installs from a repository");
        assert_eq!(
            plan.command_line(),
            "pkexec rum install -y quickshell-git dms dms-cli dgop danksearch \
             dankcalendar-git matugen"
        );
        // The password must go to the polkit agent, never through this process.
        assert_eq!(plan.program, "pkexec");
        // quickshell-git is named so the solver cannot hand back noctalia-qs,
        // which is the legacy Noctalia build and not a dependency of anything
        // current.
        assert!(!plan.command_line().contains("noctalia-qs"));
    }

    #[test]
    fn a_shell_without_an_install_plan_has_nothing_to_install() {
        // Noctalia is in the base image; Caelestia and Ambxst are in no repo.
        for id in ["noctalia", "caelestia", "ambxst"] {
            let shell = super::shell::find(id).unwrap_or_else(|| panic!("{id} registered"));
            assert!(
                shell.install_plan.is_none(),
                "{id} should have no install plan"
            );
        }
    }

    #[test]
    fn every_shell_states_where_its_config_lives() {
        // A shell with no config location cannot be backed up or seeded, which
        // is why preflight refuses it. Recorded here so adding such a shell is a
        // deliberate act rather than an oversight.
        for compositor in compositor::all() {
            for shell in compositor.shells() {
                if shell.config_dir.is_none() {
                    assert!(
                        shell.install_plan.is_none(),
                        "{} has no config directory, so it must also be uninstallable",
                        shell.id
                    );
                }
            }
        }
    }

    #[test]
    fn unknown_id_does_not_resolve() {
        assert!(super::shell::find("does-not-exist").is_none());
    }
}
