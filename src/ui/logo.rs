//! Shell logos, loaded straight from `assets/` as SVG.
//!
//! GTK 4.22 renders SVG through GdkTexture, so there is no raster step and no
//! PNG fallback to maintain. The file name comes from `Shell::logo`, which is
//! declared next to the shell itself.

use std::path::PathBuf;

use gtk4::gdk;
use gtk4::gio;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Image, Label, Orientation};

use crate::shell::Shell;

/// Painted edge length of the logo artwork.
pub const LOGO_SIZE: i32 = 72;
/// Size of the bordered box that holds it.
pub const LOGO_BOX: i32 = 100;

/// The bordered, centred container. Shared by shell cards and the placeholder.
pub fn frame(child: &gtk4::Widget) -> GtkBox {
    let frame = GtkBox::new(Orientation::Horizontal, 0);
    frame.set_halign(Align::Center);
    frame.set_valign(Align::Center);
    frame.set_size_request(LOGO_BOX, LOGO_BOX);
    frame.add_css_class("logo-frame");
    frame.append(child);
    frame
}

pub fn build(shell: &Shell) -> gtk4::Widget {
    let widget: gtk4::Widget = match load(shell) {
        Some(texture) => {
            let image = Image::new();
            image.set_pixel_size(LOGO_SIZE);
            // pixel_size only sets a minimum, so pin the request too; otherwise
            // the image can allocate at the SVG's natural size and overflow.
            image.set_size_request(LOGO_SIZE, LOGO_SIZE);
            image.set_paintable(Some(&texture));
            image.upcast()
        }
        None => letter_fallback(shell),
    };

    widget.set_halign(Align::Center);
    widget.set_valign(Align::Center);
    widget
}

fn load(shell: &Shell) -> Option<gdk::Texture> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join(shell.logo?);
    match gdk::Texture::from_file(&gio::File::for_path(path)) {
        Ok(texture) => Some(texture),
        Err(err) => {
            crate::logging::warn(&format!("{}: cannot load logo: {err}", shell.id));
            None
        }
    }
}

fn letter_fallback(shell: &Shell) -> gtk4::Widget {
    let initial = shell
        .display_name
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".into());
    let label = Label::new(Some(&initial));
    label.add_css_class("logo-fallback");
    label.upcast()
}
