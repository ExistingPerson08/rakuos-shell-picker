//! The page, assembled from parts.
//!
//! This is the "index" of the UI: it owns the shared selection state and wires
//! the tab bar, the compositor stack and the log panel together. It knows
//! nothing about individual shells, and nothing about how a compositor is
//! implemented, so backend work never has to touch this file.

pub mod card;
pub mod grid;
pub mod log;
pub mod logo;
pub mod tabbar;

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Label, Orientation, TextView, ToggleButton};

use crate::compositor;

/// One selectable shell card, remembered so selecting one can deselect the rest.
struct Item {
    shell_id: &'static str,
    button: ToggleButton,
    label: Label,
}

/// State shared by the tab bar and every card.
pub struct Selection {
    log: TextView,
    items: Vec<Item>,
}

impl Selection {
    pub fn new(log: TextView) -> Self {
        Self {
            log,
            items: Vec::new(),
        }
    }

    pub fn register(&mut self, shell_id: &'static str, button: &ToggleButton, label: &Label) {
        self.items.push(Item {
            shell_id,
            button: button.clone(),
            label: label.clone(),
        });
    }

    /// Single-selection: clear every other card, then mark this one.
    pub fn activate(&mut self, shell_id: &str) {
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

    pub fn write(&self, line: &str) {
        log::write(&self.log, line);
    }
}

pub type Shared = Rc<RefCell<Selection>>;

pub struct Picker {
    root: GtkBox,
}

impl Picker {
    pub fn new() -> Self {
        let log = log::build();
        let shared: Shared = Rc::new(RefCell::new(Selection::new(log.clone())));

        // One tab per compositor, one page per compositor, straight from the
        // registry. Registering a shell touches no UI file.
        let stack = gtk4::Stack::new();
        stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
        stack.set_vexpand(true);
        for compositor in compositor::all() {
            stack.add_named(&grid::build(compositor, &shared), Some(compositor.id()));
        }
        stack.set_visible_child_name(compositor::default_id());

        let root = GtkBox::new(Orientation::Vertical, 0);
        root.add_css_class("picker-root");
        root.append(&tabbar::build(&stack, &shared));
        root.append(&stack);
        root.append(&log::panel(&log));

        log::startup(&log);

        Self { root }
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.root.upcast_ref()
    }
}

impl Default for Picker {
    fn default() -> Self {
        Self::new()
    }
}
