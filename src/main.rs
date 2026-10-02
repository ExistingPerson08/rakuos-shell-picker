// rakuos-shell-picker: pick a desktop shell, per compositor.

fn main() -> gtk4::glib::ExitCode {
    env_logger::init();
    rakuos_shell_picker::app::run()
}
