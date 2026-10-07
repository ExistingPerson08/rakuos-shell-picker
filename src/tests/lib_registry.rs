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
}

#[test]
fn a_compositor_only_offers_shells_it_really_supports() {
    // Hyprland's four are the ones upstream documents for it. Caelestia is
    // present because upstream ships a Hyprland shell for it.
    //
    // The Niri list this used to check — Noctalia, DMS and Ambxst, without
    // Caelestia, because only third party forks exist — is recorded in
    // `compositor::niri`, which is the note for whoever brings it back. There
    // is no second registered compositor to check, so the rule is stated
    // rather than exercised: a new entry gets reviewed against upstream, not
    // against this test.
    assert_eq!(compositor::all().len(), 1, "only Hyprland is registered");
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
        "pkexec rum install -y --refresh quickshell-git dms dms-cli dgop \
         danksearch dankcalendar-git matugen khal"
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
fn every_installed_plan_can_do_something() {
    // A plan with no stages, an empty stage, or a `package` that appears in
    // none of its stages installs nothing while looking deliberate in the
    // source. `packages()` on such a plan is empty, and "is every one of
    // these packages installed?" over an empty list is answered yes, so the
    // switch would report the shell as already installed and then try to
    // start it. That is the one place where a typo becomes a lie told to the
    // user after their backup was taken.
    //
    // DMS is the only plan there is today, so this reads as a formality. It
    // is here for whoever adds the next shell and mistypes a stage.
    for compositor in compositor::all() {
        for shell in compositor.shells() {
            let Some(plan) = shell.install_plan else {
                continue;
            };
            assert!(
                plan.is_well_formed(),
                "{}/{} declares an install plan that cannot install anything: \
                 stages {:?}",
                compositor.id(),
                shell.id,
                plan.stages
            );
            assert!(
                !plan.packages().is_empty(),
                "{}/{} has a well-formed plan naming no packages",
                compositor.id(),
                shell.id
            );
        }
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
            assert_eq!(
                shell.generated_paths, identity.generated_paths,
                "{} generated paths",
                shell.id
            );
            assert_eq!(
                shell.discard_paths, identity.discard_paths,
                "{} discard paths",
                shell.id
            );
            assert_eq!(
                shell.runtime_packages, identity.runtime_packages,
                "{} runtime packages",
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
    // `config_dir`, `state_dir`, `skel_extra_paths` and `generated_paths` are all
    // joined to `$HOME` and then written to, so a leading slash or a `..` would
    // put a switch outside the user's home. SAFETY.md rule 1 depends on this
    // holding.
    for compositor in super::compositor::all() {
        for shell in compositor.shells() {
            for rel in [shell.config_dir, shell.state_dir]
                .into_iter()
                .flatten()
                .chain(shell.skel_extra_paths.iter().copied())
                .chain(shell.generated_paths.iter().copied())
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
fn every_file_in_a_skel_reaches_the_machine() {
    // The mirror of the test above, and the one that catches a file added to a
    // skel without a declaration to go with it.
    //
    // `apply::plan` writes exactly four things: the compositor's config_dir, the
    // target's config_dir, its state_dir, and `skel_extra_paths`. A fifth file
    // dropped into a skel is shipped and never installed — it passes the test
    // above precisely because nothing declares it, and a switch then leaves the
    // previous shell's copy of that path in place. Two files were in exactly
    // that position before this test existed, each with a comment in the skel
    // explaining why the switch has to replace it.
    //
    // `generated_paths` counts as coverage: a skel may seed a file the shell
    // refreshes itself, which is what Noctalia's rendered themes are.
    fn collect(dir: &std::path::Path, base: &std::path::Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect(&path, base, out);
            } else {
                let rel = path
                    .strip_prefix(base)
                    .expect("every skel file is inside the skel")
                    .to_string_lossy()
                    .into_owned();
                out.push(rel);
            }
        }
    }

    for compositor in super::compositor::all() {
        for shell in compositor.shells() {
            let skel = crate::apply::skel_for(compositor.id(), shell.id);
            if !skel.is_dir() {
                continue;
            }
            let mut files = Vec::new();
            collect(&skel, &skel, &mut files);
            files.sort();
            for rel in files {
                let owned = [
                    Some(compositor.config_dir()),
                    shell.config_dir,
                    shell.state_dir,
                ]
                .into_iter()
                .flatten()
                .chain(shell.skel_extra_paths.iter().copied())
                .chain(shell.generated_paths.iter().copied())
                .any(|prefix| rel == prefix || rel.starts_with(&format!("{prefix}/")));
                assert!(
                    owned,
                    "{}/{} ships {rel} but nothing makes a switch write it: plan only \
                     writes config_dir, state_dir and skel_extra_paths, and this file \
                     is under none of them",
                    compositor.id(),
                    shell.id
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
    // The flag belongs to the shells whose launcher waits on the command
    // returning. Every other mechanism either has a unit that never reaches
    // the spawn branch at all, or a compositor start hook that does not wait,
    // so carrying the flag would cost the process accounting `stop` relies on
    // and buy nothing.
    //
    // Niri was the only thing that ever needed the flag. It is gone, so the
    // loop below no longer has a shell to catch — which is exactly why the
    // property is now asserted on the mechanisms themselves, further down.
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

    // `SpawnAtStartup` is the one mechanism that has to detach, and Niri was
    // the only thing using it. It is kept for the next project, so the rule is
    // checked on the mechanism itself rather than on a port that no longer
    // exists — a variant that stopped detaching would be caught here, while one
    // nobody uses yet is not caught by the loop above.
    assert!(super::shell::StartMechanism::SpawnAtStartup.needs_detached_start());
    for mechanism in [
        super::shell::StartMechanism::Autostart,
        super::shell::StartMechanism::StartEvent,
        super::shell::StartMechanism::SystemdUserUnit,
        super::shell::StartMechanism::DirectProcess,
    ] {
        assert!(
            !mechanism.needs_detached_start(),
            "{mechanism:?} does not wait for the command, so detaching buys nothing"
        );
    }

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
            let saved: Vec<String> = crate::switch::backup_sources(&home, compositor, Some(shell))
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
fn every_generated_path_is_backed_up() {
    // The mirror of `every_installed_path_is_backed_up`, for the paths a shell
    // writes for itself rather than the ones a switch writes for it.
    //
    // `clean` will not delete a generated path unless this switch's backup
    // reproduces it byte for byte, so a path outside every directory
    // `backup_sources` walks would be kept forever and reported as such. That is
    // merely untidy. The reason it is still a test is the guard itself: the day
    // somebody loosens the byte-for-byte check, an uncovered entry becomes a
    // delete with no copy anywhere — which is exactly what the rule-2 test above
    // exists to prevent, reached from the other direction.
    let home = std::path::PathBuf::from("/home/tester");
    for compositor in super::compositor::all() {
        for shell in compositor.shells() {
            let saved: Vec<String> = crate::switch::backup_sources(&home, compositor, Some(shell))
                .iter()
                .map(|p| {
                    p.strip_prefix(&home)
                        .expect("every backup path is inside home")
                        .to_string_lossy()
                        .into_owned()
                })
                .collect();
            for rel in shell.generated_paths {
                let covered = saved.iter().any(|s| {
                    rel == s
                        || rel.starts_with(&format!("{s}/"))
                        || s.starts_with(&format!("{rel}/"))
                });
                assert!(
                    covered,
                    "{}/{} removes {rel} after a switch but never backs it up: \
                     it has to sit inside a path backup_sources already walks \
                     (SHARED_PATHS, config_dir or state_dir)",
                    compositor.id(),
                    shell.id
                );
            }
        }
    }
}

#[test]
fn every_discard_path_is_never_preserved() {
    // The inverse of the two tests above, and the reason they can stay strict.
    //
    // `generated_paths` may only live where a backup already walks, because
    // `clean` refuses to delete it otherwise. `discard_paths` may not live there
    // at all: it is the list of things with no data in them, `clean` deletes it
    // without asking the backup, and a path the backup reproduces is by
    // definition a path somebody could have written to. Letting the two overlap
    // would turn "there is nothing here to lose" from a property into a hope.
    //
    // `covers` is the same three-way containment the tests above use, run the
    // other way round: it fails if a discard path is a backed-up path, sits
    // inside one, or contains one. The last case matters most — a discard path
    // that were a parent of a backed-up directory would delete on trust
    // everything under it, including the copy the switch just took.
    let home = std::path::PathBuf::from("/home/tester");
    for compositor in super::compositor::all() {
        for shell in compositor.shells() {
            let saved: Vec<String> = crate::switch::backup_sources(&home, compositor, Some(shell))
                .iter()
                .map(|p| {
                    p.strip_prefix(&home)
                        .expect("every backup path is inside home")
                        .to_string_lossy()
                        .into_owned()
                })
                .collect();
            for rel in shell.discard_paths {
                assert!(
                    !rel.starts_with('/') && !rel.contains(".."),
                    "{}/{} discards {rel}, which is not a path inside home",
                    compositor.id(),
                    shell.id
                );
                let covered = saved.iter().any(|s| {
                    rel == s
                        || rel.starts_with(&format!("{s}/"))
                        || s.starts_with(&format!("{rel}/"))
                });
                assert!(
                    !covered,
                    "{}/{} discards {rel}, which a switch backs up: it belongs in \
                     generated_paths, where the backup has to reproduce it byte \
                     for byte before it is removed",
                    compositor.id(),
                    shell.id
                );
            }
            for (i, rel) in shell.discard_paths.iter().enumerate() {
                assert!(
                    !shell.discard_paths[..i].contains(rel),
                    "{}/{} discards {rel} twice",
                    compositor.id(),
                    shell.id
                );
            }
        }
    }
}

#[test]
fn a_compositor_with_no_skel_yet_refuses_every_shell_on_it() {
    // A compositor with no skel has nothing to install, so a switch under it
    // cannot complete — and has to say so rather than leave the old
    // configuration in place and call it a switch.
    //
    // This was tested through Niri, which had three ports and an empty skel.
    // Niri is gone, and the rule it was covering is the one that will be
    // needed again when the next compositor is added, so it is tested through
    // a compositor with no skel directory at all. That is the more general
    // shape of the same hole: any compositor whose skel has not been written
    // yet hits exactly this.
    let shell = super::compositor::HYPRLAND
        .shells()
        .iter()
        .find(|s| s.id == "noctalia")
        .expect("noctalia registered for hyprland");

    let home = std::env::temp_dir()
        .join("rakuos-shell-picker-tests")
        .join("no-skel");
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();

    let detail = crate::apply::dry_run(
        shell,
        &super::compositor::fake::FAKE,
        &crate::apply::Context {
            home: home.clone(),
            backups_root: home.join("backups"),
            // Nothing has ever written a skel for this id.
            skel: crate::apply::skel_for("fake", "noctalia"),
        },
        &mut |_| {},
    )
    .expect_err("a switch must refuse while there is no skel to install");

    assert!(
        detail.contains("nothing to switch to"),
        "the refusal has to explain itself: {detail}"
    );
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_skel_is_missing_only_where_the_shell_cannot_be_installed() {
    // Hyprland's Caelestia and Ambxst have no skel, and that is deliberate:
    // neither is in any repository, so the dialog refuses them as not
    // installed before a skel is ever consulted. Their cards explain
    // themselves rather than failing halfway through a switch.
    //
    // Niri had three ports and no skel at all, and that was not deliberate.
    // Two of its shells were installable, so the dialog offered them, and the
    // switch then refused with a message about an empty skel — which reads as
    // a bug in the picker rather than a decision about Niri.
    //
    // So the rule is not that every shell has a skel. It is that a missing
    // skel is only allowed where the shell cannot be installed anyway. This is
    // what would have caught Niri, and it is the check that has to exist for
    // the next compositor.
    for compositor in compositor::all() {
        for shell in compositor.shells() {
            if crate::apply::skel_for(compositor.id(), shell.id).is_dir() {
                continue;
            }
            assert!(
                shell.package.is_none() && shell.install_plan.is_none(),
                "{}/{} has no skel, so a switch to it cannot complete — and it \
                 is installable, so the dialog will offer it and the user will \
                 be refused only after their backup was taken. Write the skel, \
                 or take the shell out of the registry until there is one.",
                compositor.id(),
                shell.id
            );
        }
    }
}

#[test]
fn the_two_shells_without_a_skel_are_the_two_that_cannot_be_installed() {
    // Records the deliberate state rather than leaving it implied, because
    // "no skel" and "no package" are the same fact here and one of them
    // changing should be noticed.
    //
    // Noctalia is the counter-example that keeps this honest: it has a
    // package and no install plan, and that is correct, because it is in the
    // base image and never needs installing. It also has a skel, which is why
    // the rule above leaves it alone.
    for id in ["caelestia", "ambxst"] {
        let shell = super::shell::find(id).expect("registered");
        assert!(
            shell.package.is_none(),
            "{id} is in no repository, so it must not claim a package"
        );
        assert!(
            shell.install_plan.is_none(),
            "{id} has nothing to install from, so it must not claim a plan"
        );
        assert!(
            !crate::apply::skel_for("hyprland", id).is_dir(),
            "{id} cannot be installed and so is refused before the backup; a \
             skel for it would imply a switch that can be completed"
        );
    }
}

#[test]
fn every_uninstall_plan_can_do_something() {
    // The mirror of `every_installed_plan_can_do_something`, and it matters for
    // a reason that is easy to miss: an uninstall stage with nothing in it runs
    // successfully and removes nothing, so the switch would announce the shell
    // was gone while its packages were still on the machine. Half-removed and
    // reported as clean is worse than not having offered it.
    for compositor in compositor::all() {
        for shell in compositor.shells() {
            let Some(plan) = shell.uninstall_plan else {
                continue;
            };
            assert!(
                plan.is_well_formed(),
                "{}/{} declares an uninstall plan that cannot remove anything: \
                 stages {:?}",
                compositor.id(),
                shell.id,
                plan.stages
            );
            assert!(!plan.packages().is_empty());
        }
    }
}

#[test]
fn a_shell_that_can_be_installed_can_also_be_removed() {
    // Otherwise the picker would put a shell on the machine that only the
    // package manager could ever take off again, which is exactly the
    // accumulation this feature exists to prevent. Noctalia escapes the rule
    // because it cannot be installed either: it is in the base image, and both
    // plans are `None` together.
    for compositor in compositor::all() {
        for shell in compositor.shells() {
            if shell.install_plan.is_none() {
                continue;
            }
            assert!(
                shell.uninstall_plan.is_some(),
                "{}/{} can be installed but never removed, so switching away \
                 from it would leave it behind",
                compositor.id(),
                shell.id
            );
        }
    }
}

#[test]
fn an_uninstall_plan_only_removes_packages_the_shell_was_installed_with() {
    // Removing something the install plan never put in is how one shell takes a
    // package away from another. Both plans are declared in the same file, so
    // the pair can be checked against each other directly.
    for compositor in compositor::all() {
        for shell in compositor.shells() {
            let (Some(install), Some(uninstall)) = (shell.install_plan, shell.uninstall_plan)
            else {
                continue;
            };
            let installed = install.packages();
            for package in uninstall.packages() {
                assert!(
                    installed.contains(&package),
                    "{}/{} removes {package}, which its install plan never installs",
                    compositor.id(),
                    shell.id
                );
            }
        }
    }
}

#[test]
fn the_shell_in_the_base_image_is_never_uninstalled() {
    // Noctalia ships in the image, so `rum remove` would refuse it anyway. A
    // plan that always fails is worse than no plan, and the card that showed a
    // dead button would be the visible half of that.
    let noctalia = super::shell::find("noctalia").expect("registered");
    assert!(noctalia.uninstall_plan.is_none());
    // Its files are another matter: those are the user's, and cleaning them
    // when switching away is exactly what `clean` does.
    assert!(noctalia.config_dir.is_some() || noctalia.state_dir.is_some());
}

#[test]
fn a_shell_that_runs_on_a_shared_runtime_says_so_and_the_others_do_not() {
    // The declaration `switch::uninstall` filters against, pinned to what the
    // shells actually are. Getting it backwards in either direction costs
    // something real: a shell that runs on quickshell and does not declare it
    // loses its runtime the moment DMS leaves, and Noctalia declaring it would
    // mean switching to Noctalia never cleaned anything of DMS's up.
    for id in ["dms", "caelestia", "ambxst"] {
        let shell = super::shell::find(id).expect("registered");
        assert!(
            shell.runtime_packages.contains(&"quickshell-git"),
            "{id} runs on quickshell, so it has to say so or DMS will take it away"
        );
        assert!(
            shell.runtime_packages.contains(&"quickshell"),
            "{id} accepts either spelling of the runtime"
        );
    }
    let noctalia = super::shell::find("noctalia").expect("registered");
    assert!(
        noctalia.runtime_packages.is_empty(),
        "Noctalia is a native binary and needs no runtime, which is what makes \
         switching to it the direction that cleans quickshell up"
    );
}
