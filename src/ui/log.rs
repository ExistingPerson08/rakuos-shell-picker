//! The log panel pinned to the bottom of the window, plus writing to it.

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Orientation, TextView};

/// Fixed height for the log panel at the bottom of the window.
pub const LOG_HEIGHT: i32 = 100;

pub fn build() -> TextView {
    let view = TextView::new();
    view.set_editable(false);
    view.set_cursor_visible(false);
    view.set_monospace(true);
    view.add_css_class("log-view");
    view
}

/// The bordered container. Returned as a Box on purpose: an earlier version
/// returned the inner scroller, which silently dropped all of this styling.
pub fn panel(log: &TextView) -> GtkBox {
    let frame = GtkBox::new(Orientation::Vertical, 0);
    frame.add_css_class("log-frame");
    frame.set_size_request(-1, LOG_HEIGHT);

    let scroller = gtk4::ScrolledWindow::builder()
        .child(log)
        .hexpand(true)
        .vexpand(true)
        .build();
    scroller.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    scroller.set_propagate_natural_width(false);
    scroller.set_propagate_natural_height(false);

    frame.append(&scroller);
    frame
}

pub fn write(log: &TextView, line: &str) {
    let buffer = log.buffer();
    let mut end = buffer.end_iter();
    buffer.insert(&mut end, line);
    buffer.insert(&mut end, "\n");
}

/// Opening summary, built from the registry rather than hardcoded per shell.
pub fn startup(log: &TextView) {
    write(log, "rakuos-shell-picker 0.1.0");
    for compositor in crate::compositor::all() {
        let shells = compositor.shells();
        write(
            log,
            &format!("{}: {} shells", compositor.label(), shells.len()),
        );
        for shell in shells {
            write(log, &format!("  {:<10} {}", shell.id, shell.status_label()));
        }
    }
    write(log, "select a shell card to continue");
}
