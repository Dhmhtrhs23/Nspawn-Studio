//! Thin wrappers around the external tools we drive: systemctl,
//! systemd-run, machinectl, journalctl and systemd-nspawn itself.

use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::Duration;

/// Result of running a command.
#[derive(Debug, Clone)]
pub struct CmdResult {
    pub program: String,
    pub args: Vec<String>,
    pub status: i32,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

impl CmdResult {
    pub fn combined(&self) -> String {
        let mut out = self.stdout.clone();
        if !self.stderr.is_empty() {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&self.stderr);
        }
        out
    }
}

/// Run a program and capture its output.
pub fn run(program: &str, args: &[&str]) -> std::io::Result<CmdResult> {
    let output: Output = Command::new(program).args(args).output()?;
    Ok(from_output(program, args, output))
}

fn from_output(program: &str, args: &[&str], output: Output) -> CmdResult {
    CmdResult {
        program: program.to_string(),
        args: args.iter().map(|a| a.to_string()).collect(),
        status: output.status.code().unwrap_or(-1),
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// True if the given executable exists on PATH.
pub fn which(program: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(program);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

/// Runtime state of a managed machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineState {
    Running,
    Stopped,
    Failed,
    Unknown,
}

impl MachineState {
    pub fn is_running(self) -> bool {
        matches!(self, MachineState::Running)
    }

    pub fn label(self) -> &'static str {
        match self {
            MachineState::Running => "running",
            MachineState::Stopped => "stopped",
            MachineState::Failed => "failed",
            MachineState::Unknown => "unknown",
        }
    }
}

/// Map systemctl is-active output to a MachineState.
pub fn parse_active_state(value: &str) -> MachineState {
    match value.trim() {
        "active" | "activating" | "reloading" => MachineState::Running,
        "inactive" | "deactivating" => MachineState::Stopped,
        "failed" => MachineState::Failed,
        _ => MachineState::Unknown,
    }
}

/// Query systemctl for the transient unit of a container.
pub fn machine_state(unit: &str) -> MachineState {
    match run("systemctl", &["is-active", unit]) {
        Ok(r) => parse_active_state(&r.stdout),
        Err(_) => MachineState::Unknown,
    }
}

/// Names of all currently registered machines, in one machinectl call.
pub fn running_machines() -> Vec<String> {
    match run("machinectl", &["list", "--no-legend", "--no-pager"]) {
        Ok(r) => machine_list_lines(&r.stdout),
        Err(_) => Vec::new(),
    }
}

/// True when machinectl reports a registered (running) machine.
pub fn machine_registered(name: &str) -> bool {
    running_machines().iter().any(|line| line == name)
}

/// State of a container, taking both the systemd unit and the registered
/// machine into account. Interactive runs started from a terminal are only
/// visible through machinectl.
pub fn container_state(machine: &str, unit: &str) -> MachineState {
    if machine_registered(machine) {
        MachineState::Running
    } else {
        machine_state(unit)
    }
}

/// Parse the first column (machine name) out of machinectl list output.
pub fn machine_list_lines(output: &str) -> Vec<String> {
    let mut lines = output.lines().peekable();
    if let Some(first) = lines.peek() {
        if first.trim_start().starts_with("MACHINE") {
            lines.next();
        }
    }
    lines
        .take_while(|line| !line.trim().is_empty())
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| !name.is_empty())
        .map(|name| name.to_string())
        .collect()
}

/// Clear anything left behind by a previous run of the same unit name.
pub fn clear_unit(unit: &str) {
    let _ = run("systemctl", &["stop", unit]);
    let _ = run("systemctl", &["reset-failed", unit]);
}

/// Start a container launcher as a transient systemd unit.
///
/// A previous failed run can leave a transient unit loaded under the same
/// name; systemd-run then refuses to create it ("Unit ... was already loaded
/// or has a fragment file."). We clear the unit first and retry once. If the
/// name is taken by a real installed unit we start that instead.
pub fn start_service(unit: &str, script: &str) -> Result<CmdResult, String> {
    let fragment = Path::new("/etc/systemd/system").join(unit);
    if fragment.exists() {
        return match run("systemctl", &["start", unit]) {
            Ok(r) if r.success => Ok(r),
            Ok(r) => Err(format!(
                "systemctl start {} failed ({}): {}",
                unit,
                r.status,
                r.combined().trim()
            )),
            Err(e) => Err(format!("could not execute systemctl: {}", e)),
        };
    }

    clear_unit(unit);
    let args = [
        "--unit",
        unit,
        "--collect",
        "--no-block",
        "--property=Type=simple",
        "--property=StandardOutput=journal",
        "--property=StandardError=journal",
        "--setenv=NSPAWN_STUDIO_SERVICE=1",
        script,
    ];

    let mut last_error = String::new();
    for attempt in 0..2 {
        match run("systemd-run", &args) {
            Ok(r) if r.success => return Ok(r),
            Ok(r) => {
                last_error = format!("systemd-run failed ({}): {}", r.status, r.combined().trim());
            }
            Err(e) => {
                last_error = format!("could not execute systemd-run: {}", e);
            }
        }
        clear_unit(unit);
        if attempt == 0 {
            std::thread::sleep(Duration::from_millis(300));
        }
    }
    Err(last_error)
}

/// Run a provisioning script inside the container's root filesystem.
pub fn provision(rootfs: &str, script: &str) -> Result<CmdResult, String> {
    match run(
        "systemd-nspawn",
        &["--quiet", "-D", rootfs, "/bin/sh", "-c", script],
    ) {
        Ok(r) => Ok(r),
        Err(e) => Err(format!("could not execute systemd-nspawn: {}", e)),
    }
}

/// Stop the transient unit of a container.
pub fn stop_service(unit: &str) -> Result<CmdResult, String> {
    match run("systemctl", &["stop", unit]) {
        Ok(r) if r.success => Ok(r),
        Ok(r) => Err(format!(
            "systemctl stop failed ({}): {}",
            r.status,
            r.combined().trim()
        )),
        Err(e) => Err(format!("could not execute systemctl: {}", e)),
    }
}

/// Forcibly terminate a registered machine (fallback stop).
pub fn terminate_machine(name: &str) -> Result<CmdResult, String> {
    match run("machinectl", &["terminate", name]) {
        Ok(r) => Ok(r),
        Err(e) => Err(format!("could not execute machinectl: {}", e)),
    }
}

/// Recent journal lines for a container unit.
pub fn journal(unit: &str, lines: u32) -> String {
    let n = format!("-n{}", lines);
    match run("journalctl", &["-u", unit, &n, "--no-pager", "--output=short-iso"]) {
        Ok(r) => r.combined(),
        Err(e) => format!("could not read journal: {}", e),
    }
}

/// Run a launcher script directly (used for interactive terminals).
pub fn spawn_detached(program: &str, args: &[&str]) -> std::io::Result<u32> {
    let child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(child.id())
}

/// How a terminal emulator expects the command to be passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalStyle {
    /// program -e "bash <script>"
    EString,
    /// program -e bash <script>
    EArgs,
    /// program -- bash <script>
    DashDash,
    /// program bash <script>
    Plain,
}

/// Terminal emulators we know how to launch, most preferred first.
pub const TERMINALS: &[(&str, TerminalStyle)] = &[
    ("x-terminal-emulator", TerminalStyle::EString),
    ("gnome-terminal", TerminalStyle::DashDash),
    ("kgx", TerminalStyle::DashDash),
    ("lxterminal", TerminalStyle::EString),
    ("xfce4-terminal", TerminalStyle::EString),
    ("mate-terminal", TerminalStyle::EString),
    ("konsole", TerminalStyle::EArgs),
    ("alacritty", TerminalStyle::EArgs),
    ("kitty", TerminalStyle::Plain),
    ("foot", TerminalStyle::Plain),
    ("urxvt", TerminalStyle::EArgs),
    ("xterm", TerminalStyle::EArgs),
];

/// Find the first available terminal emulator.
pub fn find_terminal() -> Option<(&'static str, TerminalStyle)> {
    TERMINALS
        .iter()
        .find(|(program, _)| which(program).is_some())
        .copied()
}

/// Build the argv needed to run a script in a terminal emulator.
pub fn terminal_command(script: &str) -> Option<Vec<String>> {
    let (program, style) = find_terminal()?;
    let mut argv = vec![program.to_string()];
    match style {
        TerminalStyle::EString => {
            argv.push("-e".to_string());
            argv.push(format!("bash {}", crate::generator::shell_quote(script)));
        }
        TerminalStyle::EArgs => {
            argv.push("-e".to_string());
            argv.push("bash".to_string());
            argv.push(script.to_string());
        }
        TerminalStyle::DashDash => {
            argv.push("--".to_string());
            argv.push("bash".to_string());
            argv.push(script.to_string());
        }
        TerminalStyle::Plain => {
            argv.push("bash".to_string());
            argv.push(script.to_string());
        }
    }
    Some(argv)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_true_and_false() {
        let ok = run("true", &[]).unwrap();
        assert!(ok.success);
        assert_eq!(ok.status, 0);
        let bad = run("false", &[]).unwrap();
        assert!(!bad.success);
        assert_eq!(bad.status, 1);
    }

    #[test]
    fn captures_stdout() {
        let r = run("sh", &["-c", "echo hello"]).unwrap();
        assert_eq!(r.stdout.trim(), "hello");
    }

    #[test]
    fn captures_stderr_and_combines() {
        let r = run("sh", &["-c", "echo out; echo err >&2"]).unwrap();
        assert!(r.stdout.contains("out"));
        assert!(r.stderr.contains("err"));
        let combined = r.combined();
        assert!(combined.contains("out"));
        assert!(combined.contains("err"));
    }

    #[test]
    fn which_finds_shell() {
        assert!(which("sh").is_some());
        assert!(which("definitely-not-a-real-binary-xyz").is_none());
    }

    #[test]
    fn active_state_parsing() {
        assert_eq!(parse_active_state("active\n"), MachineState::Running);
        assert_eq!(parse_active_state("inactive"), MachineState::Stopped);
        assert_eq!(parse_active_state("failed"), MachineState::Failed);
        assert_eq!(parse_active_state("unknown"), MachineState::Unknown);
        assert!(MachineState::Running.is_running());
    }

    #[test]
    fn machine_list_parsing() {
        let sample = "MACHINE CLASS     SERVICE        OS     VERSION ADDRESSES\ndemo    container systemd-nspawn debian 13      -\nother   container systemd-nspawn debian 12      -\n\n1 machines listed.\n";
        let names = machine_list_lines(sample);
        assert_eq!(names, vec!["demo", "other"]);
    }

    #[test]
    fn terminal_argv_shape() {
        // The exact terminal depends on the host; only check the tail shape.
        if let Some(argv) = terminal_command("/tmp/x.sh") {
            assert!(argv.len() >= 2);
            assert!(argv.iter().any(|a| a.contains("/tmp/x.sh")));
            assert!(argv.iter().any(|a| a == "bash" || a.contains("bash ")));
        }
    }

    #[test]
    fn machine_list_parsing_stops_at_summary() {
        let sample = "demo container systemd-nspawn debian 13 -\n\n1 machines listed.\n";
        assert_eq!(machine_list_lines(sample), vec!["demo"]);
    }

    #[test]
    fn clear_unit_is_safe_for_unknown_units() {
        clear_unit("nspawn-studio-definitely-not-a-unit.service");
    }
}
