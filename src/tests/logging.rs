use super::*;

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("rsp-logtest-{tag}-{}", std::process::id()));
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
fn a_command_line_session_does_not_discard_the_windows() {
    // The window owns the log and starts it empty. A switch run from a terminal
    // is a second session in the same file, and emptying it there would delete
    // whatever the window had recorded — up to and including the switch that is
    // still running when the terminal one is started.
    let _guard = serial();
    let tmp = TempDir::new("cli");
    let dir = tmp.0.join("log");

    init_at(&dir);
    info("what the window recorded before the terminal was used");
    assert!(contents(&dir).contains("before the terminal was used"));

    init_command_at(&dir);
    info("what the terminal did next");

    let text = contents(&dir);
    assert!(
        text.contains("before the terminal was used"),
        "the window's session was discarded: {text}"
    );
    assert!(text.contains("what the terminal did next"), "{text}");
    assert!(
        text.contains("--- command line session ---"),
        "the two sessions have no boundary: {text}"
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
