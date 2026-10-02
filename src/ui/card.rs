//! One selectable shell card: logo, name, status.

use gtk4::pango;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Label, Orientation, ToggleButton};

use crate::shell::Shell;
use crate::ui::{logo, Shared};

/// Card width is pinned here (not CSS) so 4 columns always fit the 806px window
/// no matter how many shells get registered.
pub const CARD_WIDTH: i32 = 150;

pub fn build(shell: &'static Shell, shared: &Shared) -> ToggleButton {
    let btn = ToggleButton::new();
    btn.set_size_request(CARD_WIDTH, -1);
    btn.add_css_class("card");
    if !shell.installed() {
        btn.add_css_class("unavailable");
    }

    let col = GtkBox::new(Orientation::Vertical, 0);
    col.set_halign(Align::Center);

    let name = Label::new(Some(shell.display_name));
    name.add_css_class("card-label");
    name.set_halign(Align::Center);
    // Bounds the label so a long name can never widen its column.
    name.set_max_width_chars(12);
    name.set_ellipsize(pango::EllipsizeMode::End);

    let sub = Label::new(Some(shell.status_label()));
    sub.add_css_class("card-sub");
    sub.set_halign(Align::Center);

    col.append(&logo::frame(&logo::build(shell)));
    col.append(&name);
    col.append(&sub);
    btn.set_child(Some(&col));

    let selected = shared.clone();
    let id = shell.id;
    shared.borrow_mut().register(id, &btn, &name);

    btn.connect_toggled(move |btn| {
        if !btn.is_active() {
            return;
        }

        let mut selection = selected.borrow_mut();
        selection.activate(id);

        if shell.installed() {
            selection.write(&format!(
                "selected: {} ({})",
                shell.display_name,
                shell.start_command()
            ));
        } else {
            selection.write(&format!(
                "selected: {} — not installed, nothing to switch to",
                shell.display_name
            ));
        }
    });

    btn
}
