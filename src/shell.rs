//! The shell type and everything needed to inspect one.
//!
//! A shell here is the panel/widgets/status layer, not the compositor and not
//! the desktop environment. Hyprland stays running across every entry here, so
//! switching shells never restarts the compositor.
//!
//! Which shells exist is *not* defined here: the registries live under
//! `compositor::hyprland::shells` and `compositor::niri::shells`, because the
//! same shell name may not exist for every compositor.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StartMechanism {
    /// Launched from the compositor's autostart file.
    Autostart,
    /// Launched from a compositor start-event hook.
    StartEvent,
    /// Niri's `spawn-at-startup` in `config.kdl`. Kept apart from `Autostart`
    /// because the backend has to write a different syntax into a different
    /// file, and Niri waits for the command to return before continuing.
    SpawnAtStartup,
    /// Launched by a systemd user unit.
    SystemdUserUnit,
    /// Launched directly, with no supervisor.
    DirectProcess,
}

/// Where a package lives on a RakuOS system.
///
/// RakuOS keeps the read-only base image and the writable overlay in two
/// separate rpm databases, and `rpm -q` reads only the base one. A package that
/// `rum install` placed in the overlay is therefore reported as not installed by
/// `rpm -q`, which is why this is an enum instead of the bool the old code had.
///
/// Verified equivalent to `rum origin` on all three cases, without rum touching
/// the network.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Origin {
    /// Part of the read-only base image.
    Base,
    /// Installed into the overlay after the image was built.
    Overlay,
    NotInstalled,
}

impl Origin {
    pub fn installed(self) -> bool {
        !matches!(self, Origin::NotInstalled)
    }

    pub fn label(self) -> &'static str {
        match self {
            Origin::Base => "base image",
            Origin::Overlay => "overlay",
            Origin::NotInstalled => "not installed",
        }
    }

    /// The verdict from two independent database lookups.
    ///
    /// Base wins over overlay, matching what `rum origin` reports, so a package
    /// that somehow exists in both is not described twice.
    fn from_flags(base: bool, overlay: bool) -> Self {
        match (base, overlay) {
            (true, _) => Origin::Base,
            (false, true) => Origin::Overlay,
            (false, false) => Origin::NotInstalled,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Shell {
    pub id: &'static str,
    pub display_name: &'static str,
    pub upstream: &'static str,
    pub package: Option<&'static str>,
    pub binary: &'static str,
    pub start_argv: &'static [&'static str],
    pub config_dir: Option<&'static str>,
    pub systemd_user_unit: Option<&'static str>,
    pub mechanism: StartMechanism,
    /// Health probe: a process name or an IPC socket path. Empty means the
    /// process name is used.
    pub health_socket: Option<&'static str>,
    /// File name inside the project `assets/` directory.
    pub logo: Option<&'static str>,
}

/// Look a shell up across every registered compositor. Registries stay
/// per-compositor; this is only for resolving an id that came from elsewhere,
/// such as the saved session state.
pub fn find(id: &str) -> Option<&'static Shell> {
    crate::compositor::all()
        .iter()
        .flat_map(|c| c.shells())
        .find(|s| s.id == id)
}

impl Shell {
    pub fn binary_present(&self) -> bool {
        which(self.binary).is_some()
    }

    /// Where this shell's package lives, if it has one.
    ///
    /// `None` for shells that are not packaged at all, where a binary in `PATH`
    /// is the only evidence there is.
    pub fn origin(&self) -> Option<Origin> {
        self.package.map(package_origin)
    }

    pub fn installed(&self) -> bool {
        self.origin().is_some_and(Origin::installed) || self.binary_present()
    }

    pub fn config_path(&self) -> Option<PathBuf> {
        let home = std::env::var_os("HOME")?;
        self.config_dir.map(|d| PathBuf::from(home).join(d))
    }

    pub fn mechanism_label(&self) -> &'static str {
        match self.mechanism {
            StartMechanism::Autostart => "compositor autostart",
            StartMechanism::StartEvent => "compositor start event",
            StartMechanism::SpawnAtStartup => "niri spawn-at-startup",
            StartMechanism::SystemdUserUnit => "systemd user unit",
            StartMechanism::DirectProcess => "direct process",
        }
    }

    pub fn start_command(&self) -> String {
        self.start_argv.join(" ")
    }

    pub fn running(&self) -> bool {
        process_running(self.binary)
    }

    /// The line under the shell name on its card.
    ///
    /// Says what clicking will do, per docs/UI.md: an installed, idle shell is
    /// a candidate to switch to, while an uninstalled one has nothing to switch
    /// to at all. `running` is the one case where clicking changes nothing,
    /// which the dialog then explains.
    pub fn status_label(&self) -> &'static str {
        if self.running() {
            "running"
        } else if self.installed() {
            "switch"
        } else {
            "not installed"
        }
    }
}

fn which(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(bin))
        .find(|p| p.is_file())
}

/// The overlay rpm database path, as reported by `rum paths`.
///
/// Read from the system rather than hardcoded, because that path is an
/// implementation detail of the running image. Resolved once per process.
fn overlay_dbpath() -> Option<&'static Path> {
    static DB: OnceLock<Option<PathBuf>> = OnceLock::new();
    DB.get_or_init(|| {
        let out = Command::new("rum").args(["paths"]).output().ok()?;
        parse_overlay_dbpath(&String::from_utf8_lossy(&out.stdout))
    })
    .as_deref()
}

/// Pull the overlay database path out of `rum paths` output.
fn parse_overlay_dbpath(text: &str) -> Option<PathBuf> {
    text.lines()
        .find_map(|line| line.trim().strip_prefix("overlay rpmdb:"))
        .map(|path| PathBuf::from(path.trim()))
        .filter(|path| path.is_absolute())
}

/// Is `pkg` present in the rpm database at `dbpath`?
///
/// `None` means the default database, which on RakuOS is the base image.
fn in_rpmdb(dbpath: Option<&Path>, pkg: &str) -> bool {
    let mut cmd = Command::new("rpm");
    if let Some(db) = dbpath {
        cmd.arg("--dbpath").arg(db);
    }
    cmd.args(["-q", pkg])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// Whether the package is installed, and where it came from if it is.
///
/// Gives the same answer as `rum origin` without rum touching the network: that
/// command spends several seconds refreshing repository metadata, and this runs
/// once per shell every time the card list is built.
pub fn package_origin(pkg: &str) -> Origin {
    Origin::from_flags(in_rpmdb(None, pkg), in_rpmdb(overlay_dbpath(), pkg))
}

fn process_running(name: &str) -> bool {
    std::fs::read_dir("/proc")
        .map(|entries| {
            entries.flatten().any(|e| {
                e.file_name()
                    .to_str()
                    .and_then(|p| p.parse::<u32>().ok())
                    .is_some_and(|pid| {
                        std::fs::read_to_string(format!("/proc/{pid}/comm"))
                            .map(|c| c.trim() == name)
                            .unwrap_or(false)
                    })
            })
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_package_in_the_base_image_wins() {
        assert_eq!(Origin::from_flags(true, false), Origin::Base);
        assert_eq!(Origin::from_flags(true, true), Origin::Base);
        assert!(Origin::Base.installed());
    }

    #[test]
    fn an_overlay_only_package_counts_as_installed() {
        assert_eq!(Origin::from_flags(false, true), Origin::Overlay);
        assert!(Origin::Overlay.installed());
    }

    #[test]
    fn a_package_in_neither_database_is_not_installed() {
        assert_eq!(Origin::from_flags(false, false), Origin::NotInstalled);
        assert!(!Origin::NotInstalled.installed());
    }

    #[test]
    fn the_overlay_database_path_comes_from_rum_paths() {
        // Real output of `rum paths` on a split RakuOS system.
        let text = "mode: split (RakuOS overlay)\n\
                    base snapshot rpmdb: /usr/share/rpm\n\
                    overlay rpmdb: /var/lib/rakuos/rum-rpmdb\n\
                    overlay upperdir: /var/lib/rakuos/overlay/upper\n";
        assert_eq!(
            parse_overlay_dbpath(text),
            Some(PathBuf::from("/var/lib/rakuos/rum-rpmdb"))
        );
    }

    #[test]
    fn a_non_rakuos_system_yields_no_overlay_path() {
        // Without an overlay line we must not guess a path: querying the wrong
        // database would report every overlay package as missing.
        assert_eq!(parse_overlay_dbpath("mode: normal\n"), None);
        assert_eq!(parse_overlay_dbpath(""), None);
    }

    #[test]
    fn a_relative_overlay_path_is_not_trusted() {
        assert_eq!(parse_overlay_dbpath("overlay rpmdb: rum-rpmdb\n"), None);
        assert_eq!(parse_overlay_dbpath("overlay rpmdb:\n"), None);
    }

    #[test]
    fn every_origin_says_where_it_came_from() {
        assert_eq!(Origin::Base.label(), "base image");
        assert_eq!(Origin::Overlay.label(), "overlay");
        assert_eq!(Origin::NotInstalled.label(), "not installed");
    }
}
