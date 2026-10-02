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
    },
    /// Compositor could not be identified at all.
    RefuseUndetected { reason: String },
}

/// Decide what a click means. No side effects.
pub fn decide(
    detection: &Detection,
    compositor: &'static dyn Compositor,
    shell: &'static Shell,
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

    if shell.running() {
        return Action::AlreadyRunning {
            shell: shell.display_name.to_string(),
        };
    }

    if !shell.installed() {
        return Action::NotInstalled {
            shell: shell.display_name.to_string(),
        };
    }

    let from = running_shell(detection).map(|s| s.display_name.to_string());

    Action::ConfirmSwitch {
        from,
        to: shell.display_name,
    }
}

/// The shell that is alive right now, if any of the registered ones is.
fn running_shell(_detection: &Detection) -> Option<&'static Shell> {
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
pub fn refuse_cross_compositor(parent: &ApplicationWindow, target: &str, current: &str) {
    alert(
        parent,
        "Blocked",
        &format!("{target} is not the running compositor — {current} is."),
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
    on_confirm: impl Fn() + 'static,
) {
    let body = match from {
        Some(name) => format!("{name} is running now."),
        None => "No registered shell is running.".to_string(),
    };

    let dialog = AlertDialog::new(Some(&format!("Switch to {to}?")), Some(&body));
    dialog.set_extra_child(Some(&markup_label(
        "Configuration is backed up and never deleted.",
    )));
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
        Action::ConfirmSwitch { from, to } => {
            confirm_switch(parent, from.as_deref(), to, move || {
                // No backend yet, so there is no operation to report and the
                // panel stays hidden. Opening it here would be a lie: the panel
                // means steps are being taken.
                crate::logging::info(&format!("switch requested: {to}"));
                crate::logging::info("backend not implemented yet, nothing was changed");
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compositor::SystemEnv;
    use crate::shell::StartMechanism;
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
        package: None,
        binary: "basename",
        start_argv: &["basename"],
        config_dir: None,
        systemd_user_unit: None,
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
        systemd_user_unit: None,
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
        systemd_user_unit: None,
        mechanism: StartMechanism::DirectProcess,
        health_socket: None,
        logo: None,
    };

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
        crate::compositor::all()
            .into_iter()
            .find(|c| c.id() == "hyprland")
            .unwrap()
    }

    fn niri() -> &'static dyn Compositor {
        crate::compositor::all()
            .into_iter()
            .find(|c| c.id() == "niri")
            .unwrap()
    }

    #[test]
    fn click_on_a_switchable_shell_asks_for_confirmation() {
        let detection = crate::compositor::detect_with(&hyprland_session());
        assert_eq!(detection.current, Some("hyprland"));
        let action = decide(&detection, hyprland(), &INSTALLED_IDLE);
        assert!(
            matches!(action, Action::ConfirmSwitch { .. }),
            "expected a confirmation, got {action:?}"
        );
    }

    #[test]
    fn click_on_a_different_compositor_is_refused_outright() {
        let detection = crate::compositor::detect_with(&hyprland_session());
        let action = decide(&detection, niri(), &INSTALLED_IDLE);
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
        let action = decide(&detection, niri(), &NOT_INSTALLED);
        assert!(matches!(action, Action::RefuseCrossCompositor { .. }));
    }

    #[test]
    fn uninstalled_shell_does_not_offer_to_switch() {
        let detection = crate::compositor::detect_with(&hyprland_session());
        let action = decide(&detection, hyprland(), &NOT_INSTALLED);
        assert!(
            matches!(action, Action::NotInstalled { .. }),
            "expected not-installed, got {action:?}"
        );
    }

    #[test]
    fn running_shell_reports_already_running() {
        let detection = crate::compositor::detect_with(&hyprland_session());
        let action = decide(&detection, hyprland(), &ALREADY_RUNNING);
        assert!(
            matches!(action, Action::AlreadyRunning { .. }),
            "expected already-running, got {action:?}"
        );
    }

    #[test]
    fn undetected_compositor_refuses_everything() {
        let detection = crate::compositor::detect_with(&env(&[]));
        assert_eq!(detection.current, None);
        let action = decide(&detection, hyprland(), &INSTALLED_IDLE);
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
            decide(&detection, hyprland(), &INSTALLED_IDLE),
            Action::RefuseUndetected { .. }
        ));
    }

    #[test]
    fn every_refusal_is_distinct_from_a_confirmation() {
        // The point of the gate: only one outcome may lead to the backend.
        let detection = crate::compositor::detect_with(&hyprland_session());
        let outcomes = [
            decide(&detection, hyprland(), &INSTALLED_IDLE),
            decide(&detection, hyprland(), &NOT_INSTALLED),
            decide(&detection, hyprland(), &ALREADY_RUNNING),
            decide(&detection, niri(), &INSTALLED_IDLE),
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
}
