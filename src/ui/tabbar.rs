//! Compositor tabs, one per registered compositor.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Orientation, ToggleButton};

use crate::compositor;
use crate::ui::Shared;

pub fn build(stack: &gtk4::Stack, shared: &Shared) -> GtkBox {
    let bar = GtkBox::new(Orientation::Horizontal, 0);
    bar.set_halign(Align::Center);
    bar.add_css_class("tabbar");

    let tabs: Rc<RefCell<Vec<(&'static str, ToggleButton)>>> = Rc::new(RefCell::new(Vec::new()));

    let default = crate::compositor::default_id(&shared.borrow().detection().clone());

    for compositor in compositor::all() {
        let tab = ToggleButton::new();
        tab.set_label(compositor.label());
        tab.add_css_class("tab");
        // The running compositor's tab, not simply the first one.
        if compositor.id() == default {
            tab.set_active(true);
            tab.add_css_class("active");
        }

        // A tab that is not the running compositor is still readable, but it is
        // marked so it is obvious why its cards cannot be used.
        let is_current = shared.borrow().detection().is_current(compositor);
        if !is_current {
            tab.add_css_class("tab-blocked");
        }

        let tabs_cb = tabs.clone();
        let shared = shared.clone();
        let stack = stack.clone();
        let id = compositor.id();
        let label = compositor.label();
        let count = compositor.shells().len();

        tab.connect_toggled(move |btn| {
            if !btn.is_active() {
                return;
            }
            // Tabs are mutually exclusive.
            for (other, tb) in tabs_cb.borrow().iter() {
                if *other != id {
                    tb.set_active(false);
                    tb.remove_css_class("active");
                }
            }
            btn.add_css_class("active");
            stack.set_visible_child_name(id);

            let selection = shared.borrow();
            selection.note(&format!("compositor -> {label}"));
            match selection.guard_reason(compositor) {
                Some(reason) => selection.note(&format!("blocked: {reason}")),
                None if count == 0 => {
                    selection.note(&format!("{id}: no shells registered yet (coming soon)."))
                }
                None => selection.note(&format!("{id}: {count} shells")),
            }
        });

        tabs.borrow_mut().push((id, tab.clone()));
        bar.append(&tab);
    }

    bar
}
