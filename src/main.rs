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
    // The same reason as `switch` below: refusals in here are written with
    // `fail`, and a sink nobody opened drops them. A dry run that ends in a
    // refusal is exactly the one worth finding in the log afterwards.
    let _ = rakuos_shell_picker::logging::init_command();

    let (Some(want_comp), Some(want_shell)) = (args.first(), args.get(1)) else {
        fail("dry run", "usage: --dry-run <compositor> <shell>");
    };
    let want_comp = want_comp.as_str();
    let want_shell = want_shell.as_str();

    let Some(comp) = compositor::all().into_iter().find(|c| c.id() == want_comp) else {
        fail(
            "dry run",
            &format!(
                "no compositor called {want_comp}; try one of: {}",
                compositor::all()
                    .iter()
                    .map(|c| c.id())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        );
    };
    let Some(shell) = comp.shells().iter().find(|s| s.id == want_shell) else {
        fail(
            "dry run",
            &format!(
                "{} has no shell called {want_shell}; try one of: {}",
                comp.label(),
                comp.shells()
                    .iter()
                    .map(|s| s.id)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        );
    };

    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        fail("dry run", "HOME is not set");
    };

    let ctx = apply::Context {
        home,
        backups_root: session::State::backups_dir(),
        skel: apply::skel_for(comp.id(), shell.id),
    };

    match apply::dry_run(shell, comp, &ctx, &mut |line| println!("{line}")) {
        Ok(()) => std::process::exit(0),
        Err(detail) => fail("dry run", &detail),
    }
}

/// Refuse, on the terminal and in the log.
///
/// `scope` says which command is refusing. It used to be hardcoded to
/// "dry run" even when the caller was `--switch`, so a bad `--switch`
/// argument answered `dry run refused: no compositor called ...` — a message
/// about a command the user had not run.
fn fail(scope: &str, detail: &str) -> ! {
    let line = format!("{scope} refused: {detail}");
    eprintln!("{line}");
    rakuos_shell_picker::logging::error(&line);
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
    // Before anything else, because the steps below call `logging::info` and a
    // sink that was never opened makes every one of those calls a no-op. The
    // window opens the log in `app::run`; a scripted switch never gets there, so
    // without this the record the comment further down promises would not exist.
    // `init_command` rather than `init`: a terminal switch appends to the log
    // instead of emptying it, so running one does not erase the window's own
    // session, which may well be mid-switch.
    let _ = rakuos_shell_picker::logging::init_command();

    let flags: Vec<&String> = args.iter().filter(|a| a.starts_with("--")).collect();
    let positional: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let (Some(want_comp), Some(want_shell)) = (positional.first(), positional.get(1)) else {
        fail(
            "switch",
            "usage: --switch <compositor> <shell> [--no-logout]",
        );
    };
    let want_comp = want_comp.as_str();
    let want_shell = want_shell.as_str();
    let log_out = !flags.iter().any(|f| f.as_str() == "--no-logout");

    let Some(comp) = compositor::all().into_iter().find(|c| c.id() == want_comp) else {
        fail("switch", &format!("no compositor called {want_comp}"));
    };
    let Some(shell) = comp.shells().iter().find(|s| s.id == want_shell) else {
        fail(
            "switch",
            &format!("{} has no shell called {want_shell}", comp.label()),
        );
    };
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        fail("switch", "HOME is not set");
    };

    // The compositor actually running, read the way the window reads it. The
    // guard in `prepare` compares this against `comp`, so it has to be the real
    // answer: passing `comp.id()` here would compare Hyprland with itself and
    // make the guard a tautology that can never refuse.
    let current = compositor::detect().current;

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

    let opening = format!(
        "switching {} to {}; log out at the end: {log_out}",
        comp.label(),
        shell.display_name
    );
    println!("{opening}");
    rakuos_shell_picker::logging::info(&opening);
    match rakuos_shell_picker::switch::everything(
        comp,
        current,
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
            rakuos_shell_picker::logging::info(&line);
            std::process::exit(0);
        }
        rakuos_shell_picker::switch::Outcome::Failed(line) => {
            eprintln!("failed: {line}");
            // A refusal happens before a single step is reported, so without this
            // a switch that never started would leave only the session marker.
            rakuos_shell_picker::logging::error(&line);
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
