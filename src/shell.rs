//! The shell type and everything needed to inspect one.
//!
//! A shell here is the panel/widgets/status layer, not the compositor and not
//! the desktop environment. Hyprland stays running across every entry here, so
//! switching shells never restarts the compositor.
//!
//! Which shells exist is *not* defined here: the registries live under
//! `compositor::hyprland::shells` and `compositor::niri::shells`, because the
//! same shell name may not exist for every compositor.

use std::path::PathBuf;
use std::process::Command;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StartMechanism {
    /// Launched from the compositor's autostart file.
    Autostart,
    /// Launched from a compositor start-event hook.
    StartEvent,
    /// Launched by a systemd user unit.
    SystemdUserUnit,
    /// Launched directly, with no supervisor.
    DirectProcess,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Availability {
    Available,
    NotInstalled,
    NoPackage,
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

    pub fn package_installed(&self) -> Option<bool> {
        self.package.map(rpm_installed)
    }

    pub fn availability(&self) -> Availability {
        if self.binary_present() {
            return Availability::Available;
        }
        match self.package_installed() {
            Some(_) => Availability::NotInstalled,
            None => Availability::NoPackage,
        }
    }

    pub fn installed(&self) -> bool {
        self.package_installed().unwrap_or(false) || self.binary_present()
    }

    pub fn config_path(&self) -> Option<PathBuf> {
        let home = std::env::var_os("HOME")?;
        self.config_dir.map(|d| PathBuf::from(home).join(d))
    }

    pub fn mechanism_label(&self) -> &'static str {
        match self.mechanism {
            StartMechanism::Autostart => "compositor autostart",
            StartMechanism::StartEvent => "compositor start event",
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

    /// One line for the UI card and the log.
    pub fn status_label(&self) -> &'static str {
        if self.running() {
            "running"
        } else if self.installed() {
            "installed"
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

fn rpm_installed(pkg: &str) -> bool {
    Command::new("rpm")
        .args(["-q", pkg])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
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
