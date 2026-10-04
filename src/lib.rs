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
mod registry_tests {
    use std::collections::{BTreeSet, HashMap, HashSet};
    use std::path::PathBuf;

    use super::compositor;
    use super::shells;
    use crate::compositor::Compositor;

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

    #[test]
    fn every_identity_is_offered_by_at_least_one_compositor() {
        // Only meaningful because an identity lives in its own file now. A shell
        // nothing registers is a file nobody reads: it compiles, it passes every
        // other test in this file, and it is not on any card. Listing the
        // identities here is what makes that visible.
        for identity in shells::every() {
            let offered = super::compositor::all()
                .iter()
                .any(|c| c.shells().iter().any(|s| s.id == identity.id));
            assert!(
                offered,
                "{} is declared in shells/ but no compositor offers it",
                identity.id
            );
        }
    }

    #[test]
    fn one_identity_serves_every_compositor_offering_that_shell() {
        // The drift the split exists to prevent. `Port` has no `package`,
        // `upstream`, `install_plan` or `companion_systemd_user_units` field, so
        // these cannot disagree today; this test is what stops someone adding one
        // back to `Port` and quietly reintroducing the second copy. A mismatch here
        // would mean one card checks `rpm -q` for a package another card installs.
        for compositor in super::compositor::all() {
            for shell in compositor.shells() {
                let identity = shells::every()
                    .into_iter()
                    .find(|i| i.id == shell.id)
                    .unwrap_or_else(|| panic!("{} has no identity file", shell.id));

                assert_eq!(shell.upstream, identity.upstream, "{} upstream", shell.id);
                assert_eq!(shell.package, identity.package, "{} package", shell.id);
                assert_eq!(shell.binary, identity.binary, "{} binary", shell.id);
                assert_eq!(
                    shell.start_argv, identity.start_argv,
                    "{} start argv",
                    shell.id
                );
                assert_eq!(
                    shell.companion_systemd_user_units, identity.companion_systemd_user_units,
                    "{} companion units",
                    shell.id
                );
                assert_eq!(shell.logo, identity.logo, "{} logo", shell.id);
                assert_eq!(
                    shell.install_plan.map(|p| p.package),
                    identity.install_plan.map(|p| p.package),
                    "{} install plan package",
                    shell.id
                );
                assert_eq!(
                    shell.install_plan.map(|p| (p.stages, p.program, p.args)),
                    identity.install_plan.map(|p| (p.stages, p.program, p.args)),
                    "{} install plan",
                    shell.id
                );
            }
        }
    }

    #[test]
    fn every_port_stays_inside_home() {
        // `config_dir`, `state_dir` and `skel_extra_paths` are all joined to `$HOME`
        // and then written to, so a leading slash or a `..` would put a switch
        // outside the user's home. SAFETY.md rule 1 depends on this holding.
        for compositor in super::compositor::all() {
            for shell in compositor.shells() {
                for rel in [shell.config_dir, shell.state_dir]
                    .into_iter()
                    .flatten()
                    .chain(shell.skel_extra_paths.iter().copied())
                {
                    assert!(
                        !rel.starts_with('/'),
                        "{}/{}: {rel} is absolute",
                        compositor.id(),
                        shell.id
                    );
                    assert!(
                        !rel.split('/').any(|part| part == ".."),
                        "{}/{}: {rel} climbs out of home",
                        compositor.id(),
                        shell.id
                    );
                }
            }
        }
    }

    #[test]
    fn every_declared_skel_extra_path_really_is_in_the_skel() {
        // `apply::plan` treats a missing extra path as a refinement it can skip, so
        // a path renamed or deleted inside a skel used to fail quietly: the switch
        // went through without it, and only the log said why.
        //
        // Scoped to shells that have a skel directory. One without is refused
        // outright by `plan`, before extras matter — see
        // `a_compositor_with_no_skel_yet_refuses_every_shell_on_it` — so there is
        // nothing to check. Niri is in that position today: its skel folder is
        // empty, which is why `niri/dms` can list four paths it has nowhere to put.
        for compositor in super::compositor::all() {
            for shell in compositor.shells() {
                let skel = crate::apply::skel_for(compositor.id(), shell.id);
                if !skel.is_dir() {
                    continue;
                }
                for rel in shell.skel_extra_paths {
                    assert!(
                        skel.join(rel).exists(),
                        "{}/{} lists {rel} but {} does not carry it",
                        compositor.id(),
                        shell.id,
                        skel.display()
                    );
                }
            }
        }
    }

    #[test]
    fn a_mechanism_agrees_with_its_unit_name() {
        // `apply::start` and `apply::verify` branch on the mechanism and then read
        // the unit name, so a shell could claim one and not supply the other. Both
        // directions are checked because `start` only consults the unit *after*
        // asking the mechanism, which is exactly why the two can drift apart.
        for compositor in super::compositor::all() {
            for shell in compositor.shells() {
                assert_eq!(
                    shell.mechanism.uses_systemd_unit(),
                    shell.systemd_user_unit.is_some(),
                    "{}/{}: mechanism {:?} and unit {:?} have to agree",
                    compositor.id(),
                    shell.id,
                    shell.mechanism,
                    shell.systemd_user_unit
                );
            }
        }
    }

    #[test]
    fn only_a_detached_start_detaches() {
        // Not "only niri": niri/dms is started by a systemd unit and never reaches
        // the spawn branch at all, so it has no business carrying the flag. The
        // property is that the flag appears for exactly the shells whose launcher
        // waits on the command returning, which today means niri's compositorside
        // launcher and Noctalia on niri — the one niri shell that is spawned
        // directly rather than through a unit.
        for compositor in super::compositor::all() {
            for shell in compositor.shells() {
                if shell.mechanism.uses_systemd_unit() {
                    assert!(
                        !shell.mechanism.needs_detached_start(),
                        "{}/{} goes through systemd, so a flag would never be seen",
                        compositor.id(),
                        shell.id
                    );
                }
            }
        }

        let niri = super::compositor::all()
            .into_iter()
            .find(|c| c.id() == "niri")
            .expect("niri registered");
        let noctalia = niri
            .shells()
            .iter()
            .find(|s| s.id == "noctalia")
            .expect("noctalia on niri");
        assert_eq!(
            noctalia.mechanism,
            super::shell::StartMechanism::SpawnAtStartup
        );
        assert!(noctalia.mechanism.needs_detached_start());

        // Nothing on hyprland detaches, whatever its mechanism: hyprland.lua's
        // exec_on does not wait for the command, so the flag buys nothing and
        // costs the process accounting `stop` relies on.
        for shell in super::compositor::HYPRLAND.shells() {
            assert!(
                !shell.mechanism.needs_detached_start(),
                "hyprland/{} should not detach",
                shell.id
            );
        }
    }

    #[test]
    fn every_installed_path_is_backed_up() {
        // The one that matters most, and the reason the backup set is declared per
        // shell instead of being derived.
        //
        // A switch writes every `skel_extra_paths` entry, replacing whatever is
        // there. When the list was derived from `config_dir` and `state_dir` alone,
        // three such writes were not covered by any backup: Noctalia's launcher
        // entry, and DMS's state directory. So a switch could destroy the user's
        // copy of a file with no way back — the one outcome rule 2 of SAFETY.md
        // exists to prevent, and it was shipping.
        //
        // Coverage is checked against the real backup list rather than against the
        // fields directly, because that list is what the switch actually uses. A
        // path counts as covered if some entry in the list is it, contains it, or
        // is contained by it — the last case because backing up a parent directory
        // covers a file inside it.
        let home = std::path::PathBuf::from("/home/tester");
        for compositor in super::compositor::all() {
            for shell in compositor.shells() {
                let saved: Vec<String> =
                    crate::switch::backup_sources(&home, compositor, Some(shell))
                        .iter()
                        .map(|p| {
                            p.strip_prefix(&home)
                                .expect("every backup path is inside home")
                                .to_string_lossy()
                                .into_owned()
                        })
                        .collect();
                let covers = |rel: &str| {
                    saved.iter().any(|s| {
                        rel == s
                            || rel.starts_with(&format!("{s}/"))
                            || s.starts_with(&format!("{rel}/"))
                    })
                };

                for rel in shell.skel_extra_paths {
                    assert!(
                        covers(rel),
                        "{}/{} writes {rel} on every switch but never backs it up: \
                         add it to backup_extra_paths, or to SHARED_PATHS if it \
                         belongs to the session rather than to this shell",
                        compositor.id(),
                        shell.id
                    );
                }
                for rel in shell.backup_extra_paths {
                    assert!(
                        covers(rel),
                        "{}/{} backs up {rel}, which its backup list already \
                         covers another way — drop the duplicate",
                        compositor.id(),
                        shell.id
                    );
                }
            }
        }
    }

    #[test]
    fn a_compositor_with_no_skel_yet_refuses_every_shell_on_it() {
        // Records what is true today rather than leaving it implied. Niri has no
        // skel, so a Niri switch cannot complete — and must say so rather than
        // leave the old configuration in place and call it a switch.
        let niri = super::compositor::all()
            .into_iter()
            .find(|c| c.id() == "niri")
            .expect("niri registered");
        let shell = niri
            .shells()
            .iter()
            .find(|s| s.id == "noctalia")
            .expect("noctalia registered for niri");

        let home = std::env::temp_dir()
            .join("rakuos-shell-picker-tests")
            .join("no-niri-skel");
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();

        let detail = crate::apply::dry_run(
            shell,
            niri,
            &crate::apply::Context {
                home: home.clone(),
                backups_root: home.join("backups"),
                skel: crate::apply::skel_for("niri", "noctalia"),
            },
            &mut |_| {},
        )
        .expect_err("a niri switch must refuse while there is no niri skel");

        assert!(
            detail.contains("nothing to switch to"),
            "the refusal has to explain itself: {detail}"
        );
        let _ = std::fs::remove_dir_all(&home);
    }
}
