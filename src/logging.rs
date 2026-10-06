//! Channel 1 of docs/LOGGING.md: the application log.
//!
//! This is where inventory, navigation and refusals go. The UI panel is for the
//! steps of an operation and nothing else, so anything that is not part of a
//! running operation belongs here.
//!
//! One session, one log: the file is truncated on start, not appended to.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::compositor::Detection;

const FILE_MODE: u32 = 0o600;
const DIR_MODE: u32 = 0o700;
const APP_DIR: &str = "rakuos-shell-picker";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    fn as_str(self) -> &'static str {
        match self {
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        }
    }
}

struct Sink {
    path: PathBuf,
    file: File,
}

fn sink() -> &'static Mutex<Option<Sink>> {
    static SINK: OnceLock<Mutex<Option<Sink>>> = OnceLock::new();
    SINK.get_or_init(|| Mutex::new(None))
}

/// RFC 3339 UTC.
///
/// Month is zero-padded rather than taken from a `month()` accessor, because the
/// meaning of that accessor is not obvious at a glance and a log that is off by
/// one month is worse than no log. From [`crate::clock`] so that logging stays
/// callable from any thread.
fn timestamp() -> String {
    crate::clock::rfc3339()
}

/// The directory the log lives in.
///
/// Prefers `XDG_RUNTIME_DIR`, which is already per-user and mode 0700. The
/// fallback creates its own UID-named directory inside the temp dir instead of
/// a bare file in `/tmp`, because `/tmp` is mode 1777 and these lines contain
/// config paths.
pub fn default_dir() -> Option<PathBuf> {
    if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") {
        let path = PathBuf::from(runtime);
        if path.is_absolute() {
            return Some(path.join(APP_DIR));
        }
    }

    let base = std::env::var_os("TMPDIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| PathBuf::from("/tmp"));

    // No libc dependency just for getuid: /proc/self is owned by whoever owns
    // this process.
    let uid = fs::metadata("/proc/self").map(|m| m.uid()).unwrap_or(0);
    Some(base.join(format!("{APP_DIR}-{uid}")))
}

fn open(dir: &Path) -> Option<Sink> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(DIR_MODE)
        .create(dir)
        .ok()?;
    let path = dir.join("picker.log");
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(FILE_MODE)
        .open(&path)
        .ok()?;
    Some(Sink { path, file })
}

/// Opens the log for this session. Call once at startup, before the UI.
pub fn init() -> Option<PathBuf> {
    init_at(&default_dir()?)
}

/// Same, but at an explicit directory. Lets tests stay out of the real runtime
/// directory.
pub fn init_at(dir: &Path) -> Option<PathBuf> {
    let opened = open(dir)?;
    let path = opened.path.clone();
    *sink().lock().ok()? = Some(opened);
    Some(path)
}

pub fn path() -> Option<PathBuf> {
    sink()
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.path.clone()))
}

pub fn log(level: Level, message: &str) {
    let mut guard = match sink().lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    let Some(sink) = guard.as_mut() else {
        return;
    };
    // A log that cannot be written must not take the app down with it.
    let file = &mut sink.file;
    let _ = writeln!(file, "{} {} {}", timestamp(), level.as_str(), message);
    let _ = file.flush();
}

pub fn info(message: &str) {
    log(Level::Info, message);
}

pub fn warn(message: &str) {
    log(Level::Warn, message);
}

pub fn error(message: &str) {
    log(Level::Error, message);
}

/// The opening summary as plain lines: version, detected compositor, then the
/// full inventory.
///
/// Pure, so it can be checked without opening a file. This is what the old UI
/// panel used to print; it belongs in the log file and nowhere else.
pub fn startup_lines(detection: &Detection) -> Vec<String> {
    let mut lines = vec![format!("start rakuos-shell-picker {APP_VERSION}")];

    lines.push(match detection.current {
        Some(id) => format!("compositor detected: {id}"),
        None => format!(
            "compositor not detected: {}",
            detection
                .refusal()
                .unwrap_or_else(|| "unknown reason".to_string())
        ),
    });

    // Grouped by compositor, because the interesting fact is which compositor
    // has which shells. A flat list would hide exactly that difference.
    let mut groups = Vec::new();
    for compositor in crate::compositor::all() {
        let mut parts = Vec::new();
        for shell in compositor.shells() {
            // Vocabulary matches status_label() on the card, so the log and the
            // screen never disagree about the same shell. The origin is worth
            // spelling out: a shell in the base image and one rum put in the
            // overlay fail in completely different ways.
            let state = if shell.running() {
                "running".to_string()
            } else if shell.installed() {
                match shell.origin() {
                    Some(origin) => format!("installed, {}", origin.label()),
                    None => "installed".to_string(),
                }
            } else {
                "not installed".to_string()
            };
            parts.push(format!("{}={state}", shell.id));
        }
        groups.push(format!("{}: {}", compositor.id(), parts.join(" ")));
    }
    lines.push(format!("inventory: {}", groups.join(" | ")));
    lines
}

pub fn startup(detection: &Detection) {
    for line in startup_lines(detection) {
        info(&line);
    }
}

#[cfg(test)]
#[path = "tests/logging.rs"]
mod tests;
