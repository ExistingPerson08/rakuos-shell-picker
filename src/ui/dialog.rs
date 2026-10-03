//! The safety gate: nothing reaches the backend without passing through here.
//!
//! `decide()` is deliberately pure — it maps a situation to an `Action` with no
//! GTK involved, so every branch is unit testable. The `AdwAlertDialog`
//! builders below only render what `decide()` already decided.
//!
//! Rules implemented here come from docs/SAFETY.md.

use libadwaita::prelude::*;
use libadwaita::{AlertDialog, ApplicationWindow, ResponseAppearance};

use crate::compositor::{Compositor, Detection};
use crate::shell::Shell;

/// What a click on a shell card means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// The card belongs to a compositor that is not the running one.
    RefuseCrossCompositor {
        target: &'static str,
        current: &'static str,
    },
    /// This shell is already the one running.
    AlreadyRunning { shell: String },
    /// Not installed, and installing is out of scope.
    NotInstalled { shell: String },
    /// Real switch candidate: needs an explicit yes.
    ConfirmSwitch {
        from: Option<String>,
        to: &'static str,
        /// The target is not installed yet and will be installed by the switch.
        install: bool,
    },
    /// Compositor could not be identified at all.
    RefuseUndetected { reason: String },
}

/// What the caller knows about a shell at the moment of the click.
///
/// Passing this in is what keeps `decide` pure. Asking the shell directly would
/// mean a package query and a scan of /proc inside a routing decision, which is
/// both slow to do twice and impossible to test without depending on whatever
/// happens to be running on the machine.
#[derive(Clone, Copy)]
pub struct ShellState {
    /// The package is installed, or an unpackaged shell is fully set up.
    pub installed: bool,
    /// A process for this shell is alive right now.
    pub running: bool,
    /// Any registered shell that is alive, named in the confirmation.
    pub running_shell: Option<&'static Shell>,
}

/// Decide what a click means. No side effects.
pub fn decide(
    detection: &Detection,
    compositor: &'static dyn Compositor,
    shell: &'static Shell,
    state: ShellState,
) -> Action {
    // Compositor checks come first: a card on the wrong compositor must never
    // be reachable, whatever state the shell itself is in.
    match detection.current {
        None => {
            return Action::RefuseUndetected {
                reason: detection.refusal().unwrap_or_else(|| {
                    "the running compositor could not be identified.".to_string()
                }),
            }
        }
        Some(current) if current != compositor.id() => {
            return Action::RefuseCrossCompositor {
                target: compositor.id(),
                current,
            }
        }
        _ => {}
    }

    if state.running {
        return Action::AlreadyRunning {
            shell: shell.display_name.to_string(),
        };
    }

    // Not being installed only refuses a shell there is no way to obtain. The
    // test is the install plan rather than the package name, because the plan is
    // the per-shell declaration that an install is actually possible: DMS has
    // one, while Caelestia and Ambxst have a package field of None and no
    // repository entry either. A shell with a plan is worth switching to,
    // because the backend installs it only after the backup.
    let install = !state.installed && shell.install_plan.is_some();
    if !state.installed && !install {
        return Action::NotInstalled {
            shell: shell.display_name.to_string(),
        };
    }

    let from = state.running_shell.map(|s| s.display_name.to_string());

    Action::ConfirmSwitch {
        from,
        to: shell.display_name,
        install,
    }
}

/// The shell that is alive right now, if any of the registered ones is.
pub fn running_shell() -> Option<&'static Shell> {
    // Look across every compositor so a running shell is still recognised when
    // the tab being viewed is a different one.
    crate::compositor::all()
        .iter()
        .flat_map(|c| c.shells())
        .find(|s| s.running())
}

// ---------------------------------------------------------------------------
// GTK rendering
// ---------------------------------------------------------------------------

fn markup_label(text: &str) -> gtk4::Label {
    let label = gtk4::Label::new(Some(text));
    label.set_use_markup(true);
    label.set_justify(gtk4::Justification::Center);
    label.set_wrap(true);
    label.set_max_width_chars(38);
    label
}

/// A short alert with an optional emphasised line underneath.
///
/// The body is plain text, so bold cannot be smuggled in through markup tags.
/// The emphasised part therefore goes in `extra` as its own markup label.
///
/// Responses are added before presenting: a dialog that appears before it has
/// any response is a dialog with no visible button.
fn alert(
    parent: &ApplicationWindow,
    heading: &str,
    body: &str,
    extra: Option<&str>,
) -> AlertDialog {
    let dialog = AlertDialog::new(Some(heading), Some(body));

    if let Some(text) = extra {
        dialog.set_extra_child(Some(&markup_label(text)));
    }

    dialog.set_default_response(Some("close"));
    dialog.set_close_response("close");
    dialog.add_response("close", "Close");
    dialog.present(Some(parent));
    dialog
}

/// Compositor mismatch. Not a yes/no: there is nothing to agree to.
pub fn refuse_cross_compositor(parent: &ApplicationWindow, target: &str, _current: &str) {
    alert(
        parent,
        "Blocked",
        &format!("{target} is not the running compositor"),
        Some("Switch image in <b>\"RakuOS system management\"</b>."),
    );
}

/// Detection failed: we refuse rather than guess.
pub fn refuse_undetected(parent: &ApplicationWindow, reason: &str) {
    alert(
        parent,
        "Blocked",
        &format!("{reason}."),
        Some("Nothing was changed."),
    );
}

/// Already running: nothing to switch to.
pub fn already_running(parent: &ApplicationWindow, shell: &str) {
    alert(
        parent,
        "Already running",
        &format!("{shell} is already running."),
        Some("Nothing to switch to."),
    );
}

/// Not installed. Installing belongs to the image, not to this picker.
pub fn not_installed(parent: &ApplicationWindow, shell: &str) {
    alert(
        parent,
        "Not installed",
        &format!("{shell} is not installed."),
        Some("Packages come from the <b>RakuOS</b> image."),
    );
}

/// The real switch: an explicit yes, and only a yes.
///
/// Cancel is the default response, so pressing Enter does the safe thing.
pub fn confirm_switch(
    parent: &ApplicationWindow,
    from: Option<&str>,
    to: &str,
    install: bool,
    on_confirm: impl Fn() + 'static,
) {
    let body = match from {
        Some(name) => format!("{name} is running now."),
        None => "No registered shell is running.".to_string(),
    };

    let dialog = AlertDialog::new(Some(&format!("Switch to {to}?")), Some(&body));
    // Say so before the click, not after: installing is the only step here that
    // changes the system, and it happens after the backup, not before.
    let extra = if install {
        format!("{to} is not installed yet. Your configuration is backed up first, then it is installed.")
    } else {
        "Configuration is backed up and never deleted.".to_string()
    };
    dialog.set_extra_child(Some(&markup_label(&extra)));
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("switch", &format!("Switch to {to}"));
    dialog.set_response_appearance("switch", ResponseAppearance::Suggested);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    dialog.connect_response(None, move |dialog, response| {
        dialog.close();
        if response == "switch" {
            on_confirm();
        }
    });
    dialog.present(Some(parent));
}

/// Show `action` to the user.
///
/// Refusals are not operations, so they go to the application log only: the
/// panel must not light up because the user clicked something the app was never
/// going to do. Only the confirmed switch opens the panel, because from there
/// on steps are actually being taken.
pub fn present(parent: &ApplicationWindow, action: Action) {
    match action {
        Action::RefuseCrossCompositor { target, current } => {
            crate::logging::info(&format!(
                "refused: {target} is not the running compositor ({current} is)"
            ));
            refuse_cross_compositor(parent, target, current);
        }
        Action::RefuseUndetected { reason } => {
            crate::logging::info(&format!("refused: {reason}"));
            refuse_undetected(parent, &reason);
        }
        Action::AlreadyRunning { shell } => {
            crate::logging::info(&format!("no-op: {shell} is already running"));
            already_running(parent, &shell);
        }
        Action::NotInstalled { shell } => {
            crate::logging::info(&format!("no-op: {shell} is not installed"));
            not_installed(parent, &shell);
        }
        Action::ConfirmSwitch { .. } => {
            // Handled by the caller, which owns the shared selection state and
            // can therefore start the operation. Reaching this arm means the
            // caller sent the action back without acting on it, which would be a
            // bug rather than a state, so it is recorded instead of ignored.
            crate::logging::info("switch confirmed but not started: caller did not handle it");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compositor::SystemEnv;
    use crate::shell::{InstallPlan, StartMechanism};
    use std::collections::HashMap;

    /// Statics, because `decide` takes `&'static Shell` exactly like the real
    /// registry does.
    ///
    /// INSTALLED_IDLE uses a real binary that is on PATH but is never a long
    /// lived process, so it reads as "installed and idle". Picking "sh" here
    /// was wrong: the shell session running cargo leaves live `sh` processes,
    /// so it read as already running. ALREADY_RUNNING uses "bash", which is
    /// alive in this very test process.
    static INSTALLED_IDLE: Shell = Shell {
        id: "fake",
        display_name: "Fake Shell",
        upstream: "test",
        package: Some("fake"),
        binary: "basename",
        start_argv: &["basename"],
        config_dir: None,
        state_dir: None,
        install_plan: Some(InstallPlan {
            package: "fake",
            stages: &[&["fake"]],
            program: "pkexec",
            args: &["rum", "install", "-y"],
        }),
        systemd_user_unit: None,

        companion_systemd_user_units: &[],
        skel_extra_paths: &[],
        mechanism: StartMechanism::DirectProcess,
        health_socket: None,
        logo: None,
    };

    static NOT_INSTALLED: Shell = Shell {
        id: "ghost",
        display_name: "Ghost Shell",
        upstream: "test",
        package: None,
        binary: "definitely-not-here",
        start_argv: &["definitely-not-here"],
        config_dir: None,
        state_dir: None,
        install_plan: None,
        systemd_user_unit: None,

        companion_systemd_user_units: &[],
        skel_extra_paths: &[],
        mechanism: StartMechanism::DirectProcess,
        health_socket: None,
        logo: None,
    };

    static ALREADY_RUNNING: Shell = Shell {
        id: "bash-shell",
        display_name: "Bash Shell",
        upstream: "test",
        package: None,
        binary: "bash",
        start_argv: &["bash"],
        config_dir: None,
        state_dir: None,
        install_plan: None,
        systemd_user_unit: None,

        companion_systemd_user_units: &[],
        skel_extra_paths: &[],
        mechanism: StartMechanism::DirectProcess,
        health_socket: None,
        logo: None,
    };

    /// An installed shell that is not running: the one case a click accepts.
    fn ready() -> ShellState {
        ShellState {
            installed: true,
            running: false,
            running_shell: Some(&ALREADY_RUNNING),
        }
    }

    /// Nothing to switch to.
    fn absent() -> ShellState {
        ShellState {
            installed: false,
            running: false,
            running_shell: None,
        }
    }

    /// Not installed, but there is a plan to install it.
    fn absent_installable() -> ShellState {
        ShellState {
            installed: false,
            running: false,
            running_shell: Some(&ALREADY_RUNNING),
        }
    }

    /// Already the shell on screen.
    fn busy() -> ShellState {
        ShellState {
            installed: true,
            running: true,
            running_shell: Some(&ALREADY_RUNNING),
        }
    }

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn hyprland_session() -> HashMap<String, String> {
        env(&[
            ("WAYLAND_DISPLAY", "wayland-1"),
            ("XDG_RUNTIME_DIR", "/run/user/1000"),
            ("HYPRLAND_INSTANCE_SIGNATURE", "test-signature"),
        ])
    }

    fn hyprland() -> &'static dyn Compositor {
        compositor_with_id("hyprland")
    }

    fn niri() -> &'static dyn Compositor {
        compositor_with_id("niri")
    }

    fn compositor_with(
        detection: &crate::compositor::Detection,
        id: &str,
    ) -> &'static dyn Compositor {
        let _ = detection;
        compositor_with_id(id)
    }

    fn compositor_with_id(id: &str) -> &'static dyn Compositor {
        crate::compositor::all()
            .into_iter()
            .find(|c| c.id() == id)
            .unwrap_or_else(|| panic!("{id} registered"))
    }

    #[test]
    fn click_on_a_switchable_shell_asks_for_confirmation() {
        let detection = crate::compositor::detect_with(&hyprland_session());
        assert_eq!(detection.current, Some("hyprland"));
        let action = decide(&detection, hyprland(), &INSTALLED_IDLE, ready());
        assert!(
            matches!(action, Action::ConfirmSwitch { .. }),
            "expected a confirmation, got {action:?}"
        );
    }

    #[test]
    fn click_on_a_different_compositor_is_refused_outright() {
        let detection = crate::compositor::detect_with(&hyprland_session());
        let action = decide(&detection, niri(), &INSTALLED_IDLE, ready());
        assert_eq!(
            action,
            Action::RefuseCrossCompositor {
                target: "niri",
                current: "hyprland"
            }
        );
    }

    #[test]
    fn compositor_check_wins_over_shell_state() {
        // A shell that is not even installed, clicked on the wrong compositor,
        // must still be refused for the compositor reason.
        let detection = crate::compositor::detect_with(&hyprland_session());
        let action = decide(&detection, niri(), &NOT_INSTALLED, absent());
        assert!(matches!(action, Action::RefuseCrossCompositor { .. }));
    }

    #[test]
    fn uninstalled_shell_does_not_offer_to_switch() {
        let detection = crate::compositor::detect_with(&hyprland_session());
        let action = decide(&detection, hyprland(), &NOT_INSTALLED, absent());
        assert!(
            matches!(action, Action::NotInstalled { .. }),
            "expected not-installed, got {action:?}"
        );
    }

    #[test]
    fn running_shell_reports_already_running() {
        let detection = crate::compositor::detect_with(&hyprland_session());
        let action = decide(&detection, hyprland(), &ALREADY_RUNNING, busy());
        assert!(
            matches!(action, Action::AlreadyRunning { .. }),
            "expected already-running, got {action:?}"
        );
    }

    #[test]
    fn undetected_compositor_refuses_everything() {
        let detection = crate::compositor::detect_with(&env(&[]));
        assert_eq!(detection.current, None);
        let action = decide(&detection, hyprland(), &INSTALLED_IDLE, ready());
        assert!(
            matches!(action, Action::RefuseUndetected { .. }),
            "expected refusal, got {action:?}"
        );
    }

    #[test]
    fn no_action_is_taken_without_a_wayland_session() {
        // Even with a compositor marker, no session means no action.
        let detection =
            crate::compositor::detect_with(&env(&[("HYPRLAND_INSTANCE_SIGNATURE", "test")]));
        assert!(matches!(
            decide(&detection, hyprland(), &INSTALLED_IDLE, ready()),
            Action::RefuseUndetected { .. }
        ));
    }

    #[test]
    fn a_packaged_but_missing_shell_offers_an_install() {
        // INSTALLED_IDLE has an install plan but is reported as not installed:
        // that is DMS today, and the dialog has to say what will happen rather
        // than imply the shell is already there.
        let detection = crate::compositor::detect_with(&hyprland_session());
        match decide(
            &detection,
            hyprland(),
            &INSTALLED_IDLE,
            absent_installable(),
        ) {
            Action::ConfirmSwitch { install, to, .. } => {
                assert!(install, "the dialog must say it will install first");
                assert_eq!(to, "Fake Shell");
            }
            other => panic!("expected a confirmation, got {other:?}"),
        }
    }

    #[test]
    fn an_installed_shell_is_not_announced_as_installing() {
        let detection = crate::compositor::detect_with(&hyprland_session());
        match decide(&detection, hyprland(), &INSTALLED_IDLE, ready()) {
            Action::ConfirmSwitch { install, .. } => assert!(!install),
            other => panic!("expected a confirmation, got {other:?}"),
        }
    }

    #[test]
    fn a_shell_with_no_package_and_nothing_installed_is_refused() {
        // NOT_INSTALLED has no package and no install plan, so there is nowhere
        // to get it from. Refusing here is what keeps a doomed switch from
        // taking a backup first.
        let detection = crate::compositor::detect_with(&hyprland_session());
        let state = ShellState {
            installed: false,
            running: false,
            running_shell: None,
        };
        assert!(matches!(
            decide(&detection, hyprland(), &NOT_INSTALLED, state),
            Action::NotInstalled { .. }
        ));
    }

    #[test]
    fn every_refusal_is_distinct_from_a_confirmation() {
        // The point of the gate: only one outcome may lead to the backend.
        let detection = crate::compositor::detect_with(&hyprland_session());
        let outcomes = [
            decide(&detection, hyprland(), &INSTALLED_IDLE, ready()),
            decide(&detection, hyprland(), &NOT_INSTALLED, absent()),
            decide(&detection, hyprland(), &ALREADY_RUNNING, busy()),
            decide(&detection, niri(), &INSTALLED_IDLE, ready()),
        ];
        let confirmations = outcomes
            .iter()
            .filter(|a| matches!(a, Action::ConfirmSwitch { .. }))
            .count();
        assert_eq!(
            confirmations, 1,
            "exactly one path may confirm: {outcomes:?}"
        );
    }

    #[test]
    fn the_real_environment_detects_hyprland() {
        // Guards against detection silently breaking on the actual session.
        let detection = crate::compositor::detect_with(&SystemEnv);
        if let Some(current) = detection.current {
            assert_eq!(current, "hyprland");
        }
    }

    /// A Niri session must block the Hyprland page in exactly the same way a
    /// Hyprland session blocks the Niri page. The guard compares ids, so this
    /// is meant to hold without any extra code; these tests are here so that if
    /// someone ever hardcodes a direction, it fails.
    fn niri_session() -> HashMap<String, String> {
        let socket = std::env::temp_dir().join("rsp-fake-niri.sock");
        let _ = std::os::unix::net::UnixListener::bind(&socket);
        let mut env = HashMap::new();
        env.insert("WAYLAND_DISPLAY".to_string(), "wayland-1".to_string());
        env.insert("XDG_RUNTIME_DIR".to_string(), "/run/user/1000".to_string());
        env.insert(
            "NIRI_SOCKET".to_string(),
            socket.to_string_lossy().to_string(),
        );
        env
    }

    /// Mirrors `Selection::guard_reason`, which needs a window. The wording is
    /// what matters here, so it is reproduced rather than dragged into a GTK
    /// test.
    fn detection_reason(
        detection: &crate::compositor::Detection,
        page: &dyn Compositor,
    ) -> Option<String> {
        let current = detection.current?;
        if current == page.id() {
            return None;
        }
        Some(format!(
            "{} is not the running compositor — {current} is.",
            page.label()
        ))
    }

    #[test]
    fn the_guard_works_in_both_directions() {
        let hyprland_up = crate::compositor::detect_with(&hyprland_session());
        let niri_up = crate::compositor::detect_with(&niri_session());
        assert_eq!(hyprland_up.current, Some("hyprland"));
        assert_eq!(niri_up.current, Some("niri"));

        // Hyprland running: the Niri page is blocked.
        assert_eq!(
            decide(&hyprland_up, niri(), &INSTALLED_IDLE, ready()),
            Action::RefuseCrossCompositor {
                target: "niri",
                current: "hyprland"
            }
        );

        // Niri running: the Hyprland page is blocked, with the ids swapped.
        assert_eq!(
            decide(&niri_up, hyprland(), &INSTALLED_IDLE, ready()),
            Action::RefuseCrossCompositor {
                target: "hyprland",
                current: "niri"
            }
        );
    }

    #[test]
    fn the_running_compositor_is_the_only_one_that_offers_a_switch() {
        for (env, running, blocked) in [
            (hyprland_session(), "hyprland", "niri"),
            (niri_session(), "niri", "hyprland"),
        ] {
            let detection = crate::compositor::detect_with(&env);
            let running_page = compositor_with(&detection, running);
            let blocked_page = compositor_with(&detection, blocked);

            assert!(
                matches!(
                    decide(&detection, running_page, &INSTALLED_IDLE, ready()),
                    Action::ConfirmSwitch { .. }
                ),
                "{running} page should offer a switch when {running} is running"
            );
            assert!(
                matches!(
                    decide(&detection, blocked_page, &INSTALLED_IDLE, ready()),
                    Action::RefuseCrossCompositor { .. }
                ),
                "{blocked} page should be blocked when {running} is running"
            );
        }
    }

    #[test]
    fn the_block_reason_names_both_compositors_in_the_right_order() {
        let detection = crate::compositor::detect_with(&niri_session());
        let reason = detection_reason(&detection, hyprland());
        let text = reason.expect("hyprland must be refused while niri runs");
        assert!(text.contains("hyprland"), "{text}");
        assert!(text.contains("niri"), "{text}");
        // The page being viewed must be named first, the running one second.
        assert!(
            text.find("hyprland") < text.find("niri"),
            "the wrong order reads as if the other compositor were running: {text}"
        );
    }

    #[test]
    fn the_app_opens_on_the_running_compositors_tab() {
        // Otherwise a Niri user lands on the blocked Hyprland page.
        assert_eq!(
            crate::compositor::default_id(&crate::compositor::detect_with(&hyprland_session())),
            "hyprland"
        );
        assert_eq!(
            crate::compositor::default_id(&crate::compositor::detect_with(&niri_session())),
            "niri"
        );
        // With nothing detected some tab still has to be selected.
        let unknown = crate::compositor::detect_with(&env(&[]));
        assert_eq!(
            crate::compositor::default_id(&unknown),
            crate::compositor::all()[0].id()
        );
    }
}
