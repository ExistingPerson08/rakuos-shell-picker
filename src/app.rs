use gtk4::prelude::*;
use libadwaita::{Application, ApplicationWindow};

use crate::ui::Picker;

const APP_ID: &str = "org.rakuos.ShellPicker";
const DEFAULT_SIZE: i32 = 800;

pub fn run() -> gtk4::glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();

    app.connect_startup(|_| load_css());

    app.connect_activate(|app| {
        if let Some(window) = app.active_window() {
            window.present();
            return;
        }

        let picker = Picker::new();
        let window = ApplicationWindow::builder()
            .application(app)
            .title("Shell Picker")
            .default_width(DEFAULT_SIZE)
            .default_height(DEFAULT_SIZE)
            .content(picker.widget())
            .build();
        window.present();
    });

    app.run()
}

fn load_css() {
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(include_str!("style.css"));
    let display = gtk4::gdk::Display::default().expect("no display");
    gtk4::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
