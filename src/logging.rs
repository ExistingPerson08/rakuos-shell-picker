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

/// RFC 3339 UTC. `%m` is used rather than `month()` because the meaning of that
/// accessor is not obvious at a glance, and a log that is off by one month is
/// worse than no log.
fn timestamp() -> String {
    gtk4::glib::DateTime::now_utc()
        .and_then(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ"))
        .map(|s| s.to_string())
        .unwrap_or_else(|_| "0000-00-00T00:00:00Z".to_string())
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
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("rsp-logtest-{tag}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn reset() {
        *sink().lock().unwrap() = None;
    }

    fn contents(dir: &Path) -> String {
        fs::read_to_string(dir.join("picker.log")).unwrap_or_default()
    }

    #[test]
    fn lines_carry_a_rfc3339_utc_timestamp_and_a_level() {
        let _guard = serial();
        let tmp = TempDir::new("stamp");
        let dir = tmp.0.join("nested");
        init_at(&dir);
        info("hello");

        let text = contents(&dir);
        let line = text.lines().next().expect("one line");
        let mut parts = line.split(' ');
        let ts = parts.next().unwrap();
        let level = parts.next().unwrap();
        assert_eq!(level, "INFO");
        assert_eq!(parts.next(), Some("hello"));
        assert!(
            ts.len() == 20 && ts.ends_with('Z') && ts.as_bytes()[10] == b'T',
            "timestamp not RFC 3339 UTC: {ts}"
        );
        assert!(
            ts[..4].chars().all(|c| c.is_ascii_digit()),
            "bad year: {ts}"
        );
    }

    #[test]
    fn the_log_is_truncated_on_the_next_run() {
        let _guard = serial();
        let tmp = TempDir::new("truncate");
        let dir = tmp.0.join("log");

        init_at(&dir);
        info("from the first session");
        assert!(contents(&dir).contains("first session"));

        init_at(&dir);
        assert!(
            !contents(&dir).contains("first session"),
            "a second session appended instead of starting a new log"
        );
    }

    #[test]
    fn the_log_file_is_not_readable_by_other_users() {
        let _guard = serial();
        use std::os::unix::fs::PermissionsExt;
        let tmp = TempDir::new("mode");
        let dir = tmp.0.join("log");
        init_at(&dir);

        let mode = fs::metadata(dir.join("picker.log"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "log file mode was {:o}", mode & 0o777);

        let dir_mode = fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(
            dir_mode & 0o777,
            0o700,
            "log directory mode was {:o}",
            dir_mode & 0o777
        );
    }

    #[test]
    fn writing_before_init_is_harmless() {
        let _guard = serial();
        reset();
        info("no sink yet, must not panic");
        assert!(path().is_none());
    }

    #[test]
    fn the_fallback_directory_is_private_and_per_user() {
        // Not a bare file in the shared temp dir.
        let dir = default_dir().expect("a directory");
        assert!(dir.is_absolute());
        assert_ne!(dir.file_name().unwrap(), "picker.log");
        if std::env::var_os("XDG_RUNTIME_DIR").is_none() {
            let name = dir.file_name().unwrap().to_string_lossy().to_string();
            assert!(
                name.starts_with(APP_DIR) && name.ends_with(&std::process::id().to_string())
                    || name.contains('-'),
                "fallback directory is not UID named: {name}"
            );
        }
    }

    /// The sink is process wide, so tests that open it must not overlap.
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|p| p.into_inner())
    }

    #[test]
    fn startup_summarises_version_compositor_and_inventory() {
        let detection = Detection {
            wayland: true,
            runtime_dir: Some(PathBuf::from("/run/user/1000")),
            current: Some("hyprland"),
            statuses: Vec::new(),
        };
        let text = startup_lines(&detection).join("\n");

        assert!(text.contains("start rakuos-shell-picker"), "{text}");
        assert!(text.contains("compositor detected: hyprland"), "{text}");
        assert!(text.contains("noctalia="), "no inventory: {text}");
        // The panel must never have to carry any of this.
        assert!(!text.contains("shells registered"), "{text}");
    }

    #[test]
    fn an_undetected_compositor_is_said_plainly() {
        let detection = Detection {
            wayland: false,
            runtime_dir: None,
            current: None,
            statuses: Vec::new(),
        };
        let text = startup_lines(&detection).join("\n");
        assert!(text.contains("compositor not detected:"), "{text}");
    }

    #[test]
    fn the_inventory_records_every_registered_shell() {
        let detection = Detection {
            wayland: true,
            runtime_dir: Some(PathBuf::from("/run/user/1000")),
            current: Some("hyprland"),
            statuses: Vec::new(),
        };
        let line = startup_lines(&detection)
            .into_iter()
            .find(|l| l.starts_with("inventory:"))
            .expect("inventory line");

        let registered: usize = crate::compositor::all()
            .iter()
            .map(|c| c.shells().len())
            .sum();
        // Every shell entry is exactly one `id=state` token.
        let counted = line
            .strip_prefix("inventory: ")
            .expect("prefix")
            .split_whitespace()
            .filter(|token| token.contains('='))
            .count();
        assert_eq!(counted, registered, "inventory line missed a shell: {line}");

        // Grouped per compositor, because the difference between the two pages
        // is the thing worth being able to read.
        for compositor in crate::compositor::all() {
            assert!(
                line.contains(&format!("{}: ", compositor.id())),
                "inventory does not name {}: {line}",
                compositor.id()
            );
        }
    }
}
