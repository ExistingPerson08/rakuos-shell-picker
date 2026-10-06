// rakuos-shell-picker: pick a desktop shell, per compositor.

use std::path::PathBuf;

use rakuos_shell_picker::{apply, compositor, session};

/// Explain what a switch would do, then stop.
///
/// `--dry-run <compositor> <shell>`, for instance `--dry-run hyprland dms`. A
/// switch replaces whole directories and ends the session, so being able to read
/// the plan without committing to it is the difference between trying it and
/// thinking about it.
fn dry_run(args: &[String]) -> ! {
    let want_comp = args.first().map(String::as_str).unwrap_or("hyprland");
    let want_shell = args.get(1).map(String::as_str).unwrap_or("dms");

    let Some(comp) = compositor::all().into_iter().find(|c| c.id() == want_comp) else {
        fail(&format!(
            "no compositor called {want_comp}; try one of: {}",
            compositor::all()
                .iter()
                .map(|c| c.id())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    };
    let Some(shell) = comp.shells().iter().find(|s| s.id == want_shell) else {
        fail(&format!(
            "{} has no shell called {want_shell}; try one of: {}",
            comp.label(),
            comp.shells()
                .iter()
                .map(|s| s.id)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    };

    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        fail("HOME is not set");
    };

    let ctx = apply::Context {
        home,
        backups_root: session::State::backups_dir(),
        skel: apply::skel_for(comp.id(), shell.id),
    };

    match apply::dry_run(shell, comp, &ctx, &mut |line| println!("{line}")) {
        Ok(()) => std::process::exit(0),
        Err(detail) => fail(&detail),
    }
}

fn fail(detail: &str) -> ! {
    eprintln!("dry run refused: {detail}");
    std::process::exit(1);
}

/// Switch for real, from the command line.
///
/// `--switch <compositor> <shell> [--no-logout]`, for instance
/// `--switch hyprland dms`. It runs the same pipeline the window does, so a
/// scripted switch and a clicked one cannot disagree. `--no-logout` stops short
/// of ending the session, which is the only way to watch the new shell come up and
/// still be here to see it.
fn switch(args: &[String]) -> ! {
    let flags: Vec<&String> = args.iter().filter(|a| a.starts_with("--")).collect();
    let positional: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let want_comp = positional.first().map(|s| s.as_str()).unwrap_or("hyprland");
    let want_shell = positional.get(1).map(|s| s.as_str()).unwrap_or("dms");
    let log_out = !flags.iter().any(|f| f.as_str() == "--no-logout");

    let Some(comp) = compositor::all().into_iter().find(|c| c.id() == want_comp) else {
        fail(&format!("no compositor called {want_comp}"));
    };
    let Some(shell) = comp.shells().iter().find(|s| s.id == want_shell) else {
        fail(&format!(
            "{} has no shell called {want_shell}",
            comp.label()
        ));
    };
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        fail("HOME is not set");
    };

    // Looked for across every compositor, the same way the window does it: a
    // running shell is still recognised when the tab being switched is not its own.
    let running = compositor::all()
        .into_iter()
        .flat_map(|c| c.shells())
        .find(|s| s.running());
    let roots = rakuos_shell_picker::switch::Roots {
        home,
        backups_root: session::State::backups_dir(),
    };

    println!(
        "switching {} to {}; log out at the end: {log_out}",
        comp.label(),
        shell.display_name
    );
    match rakuos_shell_picker::switch::everything(
        comp,
        Some(comp.id()),
        shell,
        running,
        &roots,
        log_out,
        &mut |line| {
            println!("  {line}");
            // The panel is not there to show this, so the log is the record. A
            // switch that replaced directories and restarted the shell should not
            // leave no trace just because it was run from a terminal.
            rakuos_shell_picker::logging::info(line);
        },
    ) {
        rakuos_shell_picker::switch::Outcome::Done(line) => {
            println!("done: {line}");
            std::process::exit(0);
        }
        rakuos_shell_picker::switch::Outcome::Failed(line) => {
            eprintln!("failed: {line}");
            std::process::exit(1);
        }
    }
}

fn main() -> gtk4::glib::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--dry-run") => dry_run(&args[1..]),
        Some("--switch") => switch(&args[1..]),
        _ => {}
    }

    rakuos_shell_picker::app::run()
}
