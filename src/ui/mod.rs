//! The page, assembled from parts.
//!
//! This is the "index" of the UI: it owns the shared selection state and wires
//! the tab bar, the compositor stack and the log panel together. It knows
//! nothing about individual shells, and nothing about how a compositor is
//! implemented, so backend work never has to touch this file.
//!
//! Every click on a shell card goes through `Selection::activate`, which is the
//! only place the safety gate is invoked from.

pub mod card;
pub mod dialog;
pub mod grid;
pub mod log;
pub mod logo;
pub mod tabbar;

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Label, Orientation, Revealer, TextView, ToggleButton};
use libadwaita::ApplicationWindow;

use crate::compositor::{self, Compositor, Detection};

/// One selectable shell card, remembered so selecting one can deselect the rest.
struct Item {
    shell_id: &'static str,
    button: ToggleButton,
    label: Label,
}

/// State shared by the tab bar and every card.
pub struct Selection {
    log: TextView,
    panel: Revealer,
    items: Vec<Item>,
    detection: Detection,
    /// Weak, because the window owns this widget: a strong reference would make
    /// the two keep each other alive.
    window: gtk4::glib::WeakRef<ApplicationWindow>,
}

impl Selection {
    pub fn new(
        log: TextView,
        panel: Revealer,
        detection: Detection,
        window: &Rc<ApplicationWindow>,
    ) -> Self {
        Self {
            log,
            panel,
            items: Vec::new(),
            detection,
            window: window.downgrade(),
        }
    }

    pub fn register(&mut self, shell_id: &'static str, button: &ToggleButton, label: &Label) {
        self.items.push(Item {
            shell_id,
            button: button.clone(),
            label: label.clone(),
        });
    }

    /// Undo any pending selection. Used when a click ends in a refusal or a
    /// cancel: a card left selected suggests a switch is waiting to happen.
    fn clear_highlight(&mut self) {
        for item in self.items.iter() {
            item.button.set_active(false);
            item.label.remove_css_class("selected");
        }
    }

    /// Single-selection: clear every other card, then mark this one.
    fn highlight(&mut self, shell_id: &str) {
        for item in self.items.iter() {
            if item.shell_id != shell_id {
                item.button.set_active(false);
                item.label.remove_css_class("selected");
            }
        }
        if let Some(item) = self.items.iter().find(|i| i.shell_id == shell_id) {
            item.label.add_css_class("selected");
        }
    }

    /// Browsing, inventory, refusals. Application log only: a panel that shows
    /// these is a second console instead of a progress indicator.
    pub fn note(&self, line: &str) {
        crate::logging::info(line);
    }

    /// One step of a running operation. Both channels, and the panel is shown.
    pub fn progress(&self, line: &str) {
        log::step(&self.log, &self.panel, line);
    }

    /// Something failed: the panel stays until the user closes it.
    pub fn failed(&self, line: &str) {
        log::failed(&self.log, &self.panel, line);
    }

    pub fn detection(&self) -> &Detection {
        &self.detection
    }

    /// A card was clicked. Runs the safety gate; nothing else does.
    pub fn activate(
        &mut self,
        compositor: &'static dyn Compositor,
        shell: &'static crate::shell::Shell,
    ) {
        // State is gathered here, once per click, so the routing below stays a
        // pure function of it.
        let state = dialog::ShellState {
            installed: shell.installed(),
            running: shell.running(),
            running_shell: dialog::running_shell(),
        };
        let action = dialog::decide(&self.detection, compositor, shell, state);
        match action {
            dialog::Action::ConfirmSwitch { .. } => self.highlight(shell.id),
            _ => self.clear_highlight(),
        }

        let Some(window) = self.window.upgrade() else {
            return;
        };
        dialog::present(&window, action);
    }

    /// The niri page and any other non-running compositor: cards are inert, but
    /// they still explain themselves instead of looking broken.
    pub fn guard_reason(&self, compositor: &'static dyn Compositor) -> Option<String> {
        if let Some(current) = self.detection.current {
            if current != compositor.id() {
                return Some(format!(
                    "{} is not the running compositor — {} is. Switching image \
                     is done in \"RakuOS system management\".",
                    compositor.label(),
                    current
                ));
            }
            return None;
        }
        self.detection.refusal()
    }

    /// Clicked the placeholder on a page that the guard is blocking. Shows the
    /// refusal instead of silently doing nothing.
    pub fn guard_click(&mut self, compositor: &'static dyn Compositor) {
        let reason = self
            .guard_reason(compositor)
            .unwrap_or_else(|| "refused: nothing to switch to.".to_string());
        self.note(&format!("refused: {reason}"));

        let Some(window) = self.window.upgrade() else {
            return;
        };
        match self.detection.current {
            Some(current) => dialog::refuse_cross_compositor(&window, compositor.id(), current),
            None => dialog::refuse_undetected(&window, &reason),
        }
    }
}

pub type Shared = Rc<RefCell<Selection>>;

pub struct Picker {
    root: GtkBox,
}

impl Picker {
    pub fn new(window: &Rc<ApplicationWindow>) -> Self {
        let log = log::build();
        let panel = log::panel(&log);
        let detection = compositor::detect();

        // Version, compositor and inventory belong to the application log. The
        // panel stays hidden until an operation actually starts.
        crate::logging::startup(&detection);

        let shared: Shared = Rc::new(RefCell::new(Selection::new(
            log.clone(),
            panel.clone(),
            detection.clone(),
            window,
        )));

        // One tab per compositor, one page per compositor, straight from the
        // registry. Registering a shell touches no UI file.
        let stack = gtk4::Stack::new();
        stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
        stack.set_vexpand(true);
        for compositor in compositor::all() {
            stack.add_named(&grid::build(compositor, &shared), Some(compositor.id()));
        }
        stack.set_visible_child_name(compositor::default_id(&detection));

        let root = GtkBox::new(Orientation::Vertical, 0);
        root.add_css_class("picker-root");
        root.append(&tabbar::build(&stack, &shared));
        root.append(&stack);
        root.append(&panel);

        Self { root }
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.root.upcast_ref()
    }
}
