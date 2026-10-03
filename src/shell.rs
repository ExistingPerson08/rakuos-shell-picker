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
    /// Where this shell keeps state that is not configuration, relative to
    /// `$HOME`. Backed up alongside the config: Noctalia keeps most of itself
    /// here, so a backup that skipped it would lose the user's actual setup.
    /// `None` means the shell has none, or that it is not known, and less is
    /// then carried out of a switch.
    pub state_dir: Option<&'static str>,
    pub systemd_user_unit: Option<&'static str>,
    /// How to install this shell if it is missing. `None` means there is nothing
    /// to run: either the base image already has it, or no repository carries it.
    pub install_plan: Option<InstallPlan>,
    pub mechanism: StartMechanism,
    /// Health probe: a process name or an IPC socket path. Empty means the
    /// process name is used.
    pub health_socket: Option<&'static str>,
    /// File name inside the project `assets/` directory.
    pub logo: Option<&'static str>,
}

/// This project's own directory name, used to keep its files from colliding with
/// anything else under a shared XDG directory.
pub const PICKER_DIR: &str = "rakuos-shell-picker";

/// How this shell gets installed when it is not there yet.
///
/// Declared next to the shell rather than in a dispatch table, because whether a
/// shell can be installed is a fact about that shell: DMS ships in the RakuOS
/// repository, Noctalia is already in the base image, and Caelestia and Ambxst
/// are in no repository at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstallPlan {
    /// The package that identifies this shell, and what availability is checked
    /// against.
    pub package: &'static str,
    /// The install stages, in the order they must run. Each stage is one
    /// privileged transaction with its own prompt.
    ///
    /// Ordering is a safety property, not a convenience. A shell's runtime goes
    /// in its own stage first: it is small, it is the piece most likely to fail
    /// for its own reasons, and if it fails there is then nothing of the shell
    /// itself installed. One transaction would install everything or roll back
    /// together, so a failure in the runtime would take the shell's own packages
    /// with it for no benefit.
    ///
    /// Naming the packages inside a stage also takes a choice away from the
    /// dependency solver, which is stronger than inspecting the choice
    /// beforehand. `dms` requires `(quickshell or quickshell-git)` and the
    /// repository also carries `noctalia-qs`, which provides the same capability;
    /// a check reads metadata that can already be stale by the time the install
    /// runs, while the command line is read at the moment it matters.
    pub stages: &'static [&'static [&'static str]],
    /// The program to run, and its arguments.
    ///
    /// Package names do not belong here: they come from [`Self::stages`], so that
    /// the logged command and the command that actually runs cannot drift apart
    /// by someone forgetting to repeat a name in two places.
    pub program: &'static str,
    pub args: &'static [&'static str],
}

impl InstallPlan {
    /// The whole plan as one command line, for the log and the progress panel.
    ///
    /// This is what the plan would amount to, shown as a single line so the user
    /// can read the full set before authorizing anything. The runs are per stage.
    ///
    /// `pkexec` and not a password prompt: the password goes to the polkit
    /// agent, so it never passes through this process or its log.
    pub fn command_line(&self) -> String {
        self.line_for(&self.packages())
    }

    /// One stage as a command line.
    pub fn stage_command_line(&self, stage: &[&str]) -> String {
        self.line_for(stage)
    }

    fn line_for(&self, packages: &[&str]) -> String {
        let mut line = self.program.to_string();
        for arg in self.args {
            line.push(' ');
            line.push_str(arg);
        }
        for package in packages {
            line.push(' ');
            line.push_str(package);
        }
        line
    }

    /// Every package the plan installs, in stage order and without duplicates.
    pub fn packages(&self) -> Vec<&'static str> {
        let mut packages: Vec<&'static str> = Vec::new();
        for stage in self.stages {
            for package in *stage {
                if !packages.contains(package) {
                    packages.push(package);
                }
            }
        }
        packages
    }
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
        installed_from(self.origin(), self.binary_present(), self.config_present())
    }

    /// Has this shell actually been set up on this machine?
    pub fn config_present(&self) -> bool {
        self.config_path().is_some_and(|dir| dir.is_dir())
    }

    pub fn config_path(&self) -> Option<PathBuf> {
        let home = std::env::var_os("HOME")?;
        self.config_dir.map(|d| PathBuf::from(home).join(d))
    }

    pub fn state_path(&self) -> Option<PathBuf> {
        let home = std::env::var_os("HOME")?;
        self.state_dir.map(|d| PathBuf::from(home).join(d))
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

/// Whether a shell counts as installed, from the three facts we can observe.
///
/// Split out from the subprocess calls so the rules can be tested directly.
fn installed_from(origin: Option<Origin>, binary: bool, config: bool) -> bool {
    match origin {
        Some(origin) => origin.installed() || binary,
        // Nothing to ask about, so the binary has to carry the whole answer, and
        // it cannot be trusted alone: installing DMS pulls in noctalia-qs, which
        // puts quickshell in PATH, and Caelestia runs on that same runtime. The
        // binary would then claim Caelestia is installed while its config
        // directory does not exist. Requiring the config too keeps a shared
        // runtime from standing in for a shell that was never set up.
        None => binary && config,
    }
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
    fn an_install_plan_reads_as_one_command_line() {
        let plan = InstallPlan {
            package: "dms",
            stages: &[&["dms"]],
            program: "pkexec",
            args: &["rum", "install", "-y"],
        };
        assert_eq!(plan.command_line(), "pkexec rum install -y dms");
    }

    #[test]
    fn an_install_plan_with_no_arguments_still_names_its_stage() {
        let plan = InstallPlan {
            package: "x",
            stages: &[&["x"]],
            program: "pkexec",
            args: &[],
        };
        assert_eq!(plan.command_line(), "pkexec x");
    }

    #[test]
    fn the_whole_plan_reads_as_one_line_but_runs_stage_by_stage() {
        let plan = InstallPlan {
            package: "dms",
            stages: &[&["quickshell-git"], &["dms", "dms-cli"]],
            program: "pkexec",
            args: &["rum", "install", "-y"],
        };
        assert_eq!(
            plan.command_line(),
            "pkexec rum install -y quickshell-git dms dms-cli"
        );
        assert_eq!(
            plan.stage_command_line(&["quickshell-git"]),
            "pkexec rum install -y quickshell-git"
        );
        assert_eq!(
            plan.stage_command_line(&["dms", "dms-cli"]),
            "pkexec rum install -y dms dms-cli"
        );
    }

    #[test]
    fn a_package_is_never_named_twice_across_stages() {
        let plan = InstallPlan {
            package: "dms",
            stages: &[&["quickshell-git"], &["dms", "quickshell-git"]],
            program: "pkexec",
            args: &["rum", "install", "-y"],
        };
        assert_eq!(plan.packages(), vec!["quickshell-git", "dms"]);
    }

    #[test]
    fn the_dms_plan_installs_the_runtime_before_the_shell() {
        let plan = crate::compositor::hyprland::shells::dms::SHELL
            .install_plan
            .expect("dms is installable from the rakuos repository");
        assert_eq!(plan.stages.len(), 2);
        assert_eq!(plan.stages[0], &["quickshell-git"]);
        assert!(plan.stages[1].contains(&"dms"));
        assert!(plan.stages[1].contains(&"dankcalendar-git"));
    }

    #[test]
    fn the_dms_plan_asks_for_quickshell_git_and_not_noctalia_qs() {
        let plan = crate::compositor::hyprland::shells::dms::SHELL
            .install_plan
            .expect("dms is installable from the rakuos repository");
        let line = plan.command_line();
        assert!(line.contains("quickshell-git"), "{line}");
        assert!(!line.contains("noctalia-qs"), "{line}");
    }

    #[test]
    fn every_origin_says_where_it_came_from() {
        assert_eq!(Origin::Base.label(), "base image");
        assert_eq!(Origin::Overlay.label(), "overlay");
        assert_eq!(Origin::NotInstalled.label(), "not installed");
    }

    #[test]
    fn a_packaged_shell_is_installed_when_its_package_says_so() {
        assert!(installed_from(Some(Origin::Base), false, false));
        assert!(installed_from(Some(Origin::Overlay), false, false));
    }

    #[test]
    fn a_packaged_shell_with_a_stale_binary_still_counts() {
        // The package was removed but the binary is still on disk. Treated as
        // installed so a click attempts a switch instead of claiming there is
        // nothing there.
        assert!(installed_from(Some(Origin::NotInstalled), true, false));
    }

    #[test]
    fn an_unpackaged_shell_needs_more_than_its_runtime() {
        // quickshell exists because DMS dragged in noctalia-qs. Caelestia has no
        // package and no config, so it is not installed.
        assert!(!installed_from(None, true, false));
    }

    #[test]
    fn an_unpackaged_shell_with_its_own_config_counts() {
        assert!(installed_from(None, true, true));
    }

    #[test]
    fn an_unpackaged_shell_without_a_known_config_can_never_install() {
        // Ambxst has no config directory recorded, so there is no second fact to
        // check and nothing to confirm a setup with.
        assert!(!installed_from(None, false, false));
    }
}
