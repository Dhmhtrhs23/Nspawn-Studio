//! End-to-end tests for the systemd start path.
//!
//! These need a running systemd and root, and are skipped otherwise.

use nspawn_studio_core::command;
use std::time::Duration;

fn can_run() -> bool {
    nspawn_studio_core::store::running_as_root() && command::which("systemd-run").is_some()
}

/// A previously started (dead but still loaded) transient unit must not stop
/// us from starting the container again.
#[test]
fn start_service_recovers_from_a_stale_unit() {
    if !can_run() {
        eprintln!("skipping: requires root and systemd-run");
        return;
    }
    let unit = "nspawn-studio-selftest.service";
    let _ = command::run("systemctl", &["stop", unit]);
    let _ = command::run("systemctl", &["reset-failed", unit]);

    // systemd-run without --collect leaves the finished unit loaded, which is
    // exactly the state that made systemd-run print
    // "Unit ... was already loaded or has a fragment file".
    let _ = command::run("systemd-run", &["--unit", unit, "/bin/true"]);
    std::thread::sleep(Duration::from_millis(600));

    let result = command::start_service(unit, "/bin/true");
    assert!(result.is_ok(), "start_service failed: {}", result.unwrap_err());

    let _ = command::run("systemctl", &["stop", unit]);
    let _ = command::run("systemctl", &["reset-failed", unit]);
}

#[test]
fn clear_unit_is_idempotent() {
    if !can_run() {
        return;
    }
    command::clear_unit("nspawn-studio-no-such-unit.service");
    command::clear_unit("nspawn-studio-no-such-unit.service");
}
