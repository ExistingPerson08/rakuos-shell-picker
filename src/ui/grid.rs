//! The shell grid for one compositor, and the placeholder for an empty one.
//!
//! Fixed grid geometry. Rows grow without bound, so the grid scrolls instead of
//! the window growing: adding shells later stays safe well past 20 entries.

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Label, Orientation};

use crate::compositor::Compositor;
use crate::ui::{card, logo, Shared};

const COLUMNS: i32 = 4;
const COLUMN_SPACING: u32 = 34;
const ROW_SPACING: u32 = 46;
/// Width used by the "coming soon" placeholder, which spans the grid.
const CARD_WIDE: i32 = 320;

pub fn build(compositor: &'static dyn Compositor, shared: &Shared) -> gtk4::ScrolledWindow {
    let shells = compositor.shells();
    if shells.is_empty() {
        return scroller(placeholder(compositor).upcast());
    }

    let grid = gtk4::Grid::new();
    grid.set_column_spacing(COLUMN_SPACING);
    grid.set_row_spacing(ROW_SPACING);

    for (i, shell) in shells.iter().enumerate() {
        grid.attach(
            &card::build(shell, shared),
            (i as i32) % COLUMNS,
            (i as i32) / COLUMNS,
            1,
            1,
        );
    }

    let center = GtkBox::new(Orientation::Horizontal, 0);
    center.set_halign(Align::Center);
    center.set_valign(Align::Start);
    center.set_margin_top(14);
    center.set_margin_bottom(14);
    center.append(&grid);

    scroller(center.upcast())
}

/// Shown when a compositor has no shells registered yet, so an empty registry
/// reads as "not built" instead of a broken or blank page.
fn placeholder(compositor: &'static dyn Compositor) -> GtkBox {
    let card = GtkBox::new(Orientation::Vertical, 0);
    card.set_halign(Align::Center);
    card.set_valign(Align::Center);
    card.set_size_request(CARD_WIDE, -1);
    card.add_css_class("card");
    card.add_css_class("card-wide");

    let mark = Label::new(Some("?"));
    mark.add_css_class("logo-fallback");
    mark.add_css_class("dim-fallback");
    mark.set_halign(Align::Center);
    mark.set_valign(Align::Center);
    card.append(&logo::frame(&mark.upcast()));

    let title = Label::new(Some("Coming soon"));
    title.add_css_class("card-label");
    title.set_halign(Align::Center);
    card.append(&title);

    let sub = Label::new(Some(&format!(
        "no {} shells registered yet",
        compositor.id()
    )));
    sub.add_css_class("card-sub");
    sub.set_halign(Align::Center);
    card.append(&sub);

    card
}

fn scroller(child: gtk4::Widget) -> gtk4::ScrolledWindow {
    let scroller = gtk4::ScrolledWindow::builder()
        .child(&child)
        .vexpand(true)
        .build();
    // Horizontal scroll stays off, so the grid width is pinned no matter how
    // many shells get registered.
    scroller.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    scroller.set_propagate_natural_width(false);
    scroller.set_propagate_natural_height(false);
    scroller.set_hexpand(true);
    scroller
}
