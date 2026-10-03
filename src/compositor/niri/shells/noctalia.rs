use crate::shell::{Shell, StartMechanism};

/// Niri runs this with `spawn-at-startup "noctalia"` in `config.kdl`.
/// `--daemon` exists because Niri waits for a startup command to return before
/// continuing, so a foreground shell would stall the session.
pub static SHELL: Shell = Shell {
    id: "noctalia",
    display_name: "Noctalia",
    upstream: "noctalia-shell",
    package: Some("noctalia"),
    binary: "noctalia",
    start_argv: &["noctalia"],
    config_dir: Some(".config/noctalia"),
    state_dir: Some(".local/state/noctalia"),
    systemd_user_unit: None,
    mechanism: StartMechanism::SpawnAtStartup,
    health_socket: None,
    logo: Some("noctalia.svg"),
};
