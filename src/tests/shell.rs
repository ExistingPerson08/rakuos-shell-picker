use super::*;

#[test]
fn a_package_in_the_base_image_wins() {
    assert_eq!(Origin::from_flags(true, false), Origin::Base);
    assert_eq!(Origin::from_flags(true, true), Origin::Base);
    assert!(Origin::Base.installed());
}

#[test]
fn an_overlay_only_package_counts_as_installed() {
    assert_eq!(Origin::from_flags(false, true), Origin::Overlay);
    assert!(Origin::Overlay.installed());
}

#[test]
fn a_package_in_neither_database_is_not_installed() {
    assert_eq!(Origin::from_flags(false, false), Origin::NotInstalled);
    assert!(!Origin::NotInstalled.installed());
}

#[test]
fn the_overlay_database_path_comes_from_rum_paths() {
    // Real output of `rum paths` on a split RakuOS system.
    let text = "mode: split (RakuOS overlay)\n\
                base snapshot rpmdb: /usr/share/rpm\n\
                overlay rpmdb: /var/lib/rakuos/rum-rpmdb\n\
                overlay upperdir: /var/lib/rakuos/overlay/upper\n";
    assert_eq!(
        parse_overlay_dbpath(text),
        Some(PathBuf::from("/var/lib/rakuos/rum-rpmdb"))
    );
}

#[test]
fn a_non_rakuos_system_yields_no_overlay_path() {
    // Without an overlay line we must not guess a path: querying the wrong
    // database would report every overlay package as missing.
    assert_eq!(parse_overlay_dbpath("mode: normal\n"), None);
    assert_eq!(parse_overlay_dbpath(""), None);
}

#[test]
fn a_relative_overlay_path_is_not_trusted() {
    assert_eq!(parse_overlay_dbpath("overlay rpmdb: rum-rpmdb\n"), None);
    assert_eq!(parse_overlay_dbpath("overlay rpmdb:\n"), None);
}

#[test]
fn an_install_plan_reads_as_one_command_line() {
    let plan = InstallPlan {
        package: "dms",
        stages: &[&["dms"]],
        program: "pkexec",
        args: &["rum", "install", "-y"],
    };
    assert_eq!(plan.command_line(), "pkexec rum install -y dms");
}

#[test]
fn an_install_plan_with_no_arguments_still_names_its_stage() {
    let plan = InstallPlan {
        package: "x",
        stages: &[&["x"]],
        program: "pkexec",
        args: &[],
    };
    assert_eq!(plan.command_line(), "pkexec x");
}

#[test]
fn the_whole_plan_reads_as_one_line_but_runs_stage_by_stage() {
    let plan = InstallPlan {
        package: "dms",
        stages: &[&["quickshell-git"], &["dms", "dms-cli"]],
        program: "pkexec",
        args: &["rum", "install", "-y"],
    };
    assert_eq!(
        plan.command_line(),
        "pkexec rum install -y quickshell-git dms dms-cli"
    );
    assert_eq!(
        plan.stage_command_line(&["quickshell-git"]),
        "pkexec rum install -y quickshell-git"
    );
    assert_eq!(
        plan.stage_command_line(&["dms", "dms-cli"]),
        "pkexec rum install -y dms dms-cli"
    );
}

#[test]
fn a_plan_that_cannot_install_anything_is_not_well_formed() {
    let base = InstallPlan {
        package: "dms",
        stages: &[&["quickshell-git"], &["dms"]],
        program: "pkexec",
        args: &["rum", "install", "-y"],
    };
    assert!(base.is_well_formed(), "the shape DMS actually uses");
    assert!(base.packages().len() == 2, "{}", base.packages().len());

    // The three ways, each of which looks fine written down.
    for (what, plan) in [
        (
            "no stages",
            InstallPlan {
                stages: &[],
                ..base
            },
        ),
        (
            "an empty stage",
            InstallPlan {
                stages: &[&[], &["dms"]],
                ..base
            },
        ),
        (
            "a package in no stage",
            InstallPlan {
                package: "somewhere-else",
                stages: &[&["quickshell-git"]],
                ..base
            },
        ),
    ] {
        assert!(!plan.is_well_formed(), "{what} must not pass");
    }
}

#[test]
fn an_empty_plan_installs_nothing_rather_than_everything() {
    // The reason the guard above exists. `all` over an empty iterator is
    // vacuously true, so an empty plan used to answer "already installed"
    // and send the switch on to start a shell that was never installed.
    let empty = InstallPlan {
        package: "dms",
        stages: &[],
        program: "pkexec",
        args: &["rum", "install", "-y"],
    };
    assert!(empty.packages().is_empty());
    assert!(
        empty.packages().iter().all(|_| true),
        "this is the trap: the old check answered yes here"
    );
    assert!(
        !empty.is_well_formed(),
        "so the check has to be refused before it is asked"
    );
}

#[test]
fn a_package_is_never_named_twice_across_stages() {
    let plan = InstallPlan {
        package: "dms",
        stages: &[&["quickshell-git"], &["dms", "quickshell-git"]],
        program: "pkexec",
        args: &["rum", "install", "-y"],
    };
    assert_eq!(plan.packages(), vec!["quickshell-git", "dms"]);
}

#[test]
fn the_dms_plan_installs_the_runtime_before_the_shell() {
    let plan = crate::compositor::hyprland::shells::dms::SHELL
        .install_plan
        .expect("dms is installable from the rakuos repository");
    assert_eq!(plan.stages.len(), 2);
    assert_eq!(plan.stages[0], &["quickshell-git"]);
    assert!(plan.stages[1].contains(&"dms"));
    assert!(plan.stages[1].contains(&"dankcalendar-git"));
}

#[test]
fn the_dms_plan_asks_for_quickshell_git_and_not_noctalia_qs() {
    let plan = crate::compositor::hyprland::shells::dms::SHELL
        .install_plan
        .expect("dms is installable from the rakuos repository");
    let line = plan.command_line();
    assert!(line.contains("quickshell-git"), "{line}");
    assert!(!line.contains("noctalia-qs"), "{line}");
}

#[test]
fn every_origin_says_where_it_came_from() {
    assert_eq!(Origin::Base.label(), "base image");
    assert_eq!(Origin::Overlay.label(), "overlay");
    assert_eq!(Origin::NotInstalled.label(), "not installed");
}

#[test]
fn a_packaged_shell_is_installed_when_its_package_says_so() {
    assert!(installed_from(Some(Origin::Base), false, false));
    assert!(installed_from(Some(Origin::Overlay), false, false));
}

#[test]
fn a_packaged_shell_with_a_stale_binary_still_counts() {
    // The package was removed but the binary is still on disk. Treated as
    // installed so a click attempts a switch instead of claiming there is
    // nothing there.
    assert!(installed_from(Some(Origin::NotInstalled), true, false));
}

#[test]
fn an_unpackaged_shell_needs_more_than_its_runtime() {
    // quickshell exists because DMS dragged in noctalia-qs. Caelestia has no
    // package and no config, so it is not installed.
    assert!(!installed_from(None, true, false));
}

#[test]
fn an_unpackaged_shell_with_its_own_config_counts() {
    assert!(installed_from(None, true, true));
}

#[test]
fn an_unpackaged_shell_without_a_known_config_can_never_install() {
    // Ambxst has no config directory recorded, so there is no second fact to
    // check and nothing to confirm a setup with.
    assert!(!installed_from(None, false, false));
}
