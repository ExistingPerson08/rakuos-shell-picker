use super::*;
use crate::shell::UninstallPlan;

#[test]
fn a_removal_with_nothing_installed_never_reaches_the_package_manager() {
    // The mirror of the install's "already there" answer, and for the same
    // reason: a stage with nothing to do would still ask polkit for a password
    // and report a stage that removed nothing as progress.
    let plan = UninstallPlan {
        package: "rsp-no-such-package-anywhere",
        stages: &[&["rsp-no-such-package-anywhere"]],
        program: "pkexec",
        args: &["rum", "remove", "-y"],
    };

    let mut said = Vec::new();
    let outcome = uninstall(plan, &mut |line| said.push(line.to_string()));

    assert!(
        matches!(outcome, Removed::AlreadyGone { .. }),
        "an absent package is not a failure: {outcome:?}"
    );
    assert!(
        said.is_empty(),
        "nothing to remove is nothing to report, so no password prompt: {said:?}"
    );
}

#[test]
fn a_removal_plan_that_names_nothing_is_refused_rather_than_run() {
    // An empty plan has no packages, and "are all of these removed?" over an
    // empty list is answered yes — so without this the switch would announce
    // the shell was gone. It exists because the check before it would
    // otherwise be vacuous, exactly as for an install.
    let plan = UninstallPlan {
        package: "dms",
        stages: &[],
        program: "pkexec",
        args: &["rum", "remove", "-y"],
    };

    let outcome = uninstall(plan, &mut |_| {});
    assert!(matches!(outcome, Removed::Malformed { package: "dms" }));
}

#[test]
fn the_removal_plan_leaves_the_shared_runtime_on_the_machine() {
    // `quickshell-git` is a runtime rather than DMS, and Caelestia and anything
    // else running on quickshell would stop working the moment DMS was switched
    // away from. `docs/SAFETY.md` keeps `rum autoremove` as a separate explicit
    // choice precisely so that a switch never decides this on the user's behalf.
    let plan = crate::shell::find("dms")
        .expect("registered")
        .uninstall_plan
        .expect("dms can be installed, so it can be removed");

    assert!(
        !plan.packages().contains(&"quickshell-git"),
        "a switch must not take out a runtime another shell may be running on"
    );
    assert!(
        plan.packages().contains(&"dms"),
        "and it must take out the shell itself"
    );
}
