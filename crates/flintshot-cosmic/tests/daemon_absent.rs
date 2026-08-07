//! Headless tests for the daemon-absent UX (Task 005): the *Start daemon*
//! spawn seam and the `NoDaemon → Connected` recovery transition.
//!
//! No socket is bound, no display is needed, and no real process is
//! forked — the recording/failing spawners stand in for
//! [`RealDaemonSpawner`](flintshot_cosmic::daemon_probe::RealDaemonSpawner)
//! so the argv + honest-failure classification are exercised across the
//! module boundary without touching the OS process table.

use std::sync::Mutex;

use color_eyre::eyre;

use flintshot_cosmic::app::{App, ConnectionState, Message};
use flintshot_cosmic::daemon_probe::{self, DaemonSpawner};

/// Recording spawner: captures each spawn's argv, never forks. Mirrors
/// the `RecordingDaemonSpawner` pattern from `daemon_probe::tests`
/// (Task 002) but lives in the integration test so the seam is exercised
/// across the module boundary.
struct RecordingSpawner {
    calls: Mutex<Vec<Vec<String>>>,
}

impl RecordingSpawner {
    fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.calls.lock().expect("recorder lock").clone()
    }
}

impl DaemonSpawner for RecordingSpawner {
    fn spawn(&self, argv: &[&str]) -> eyre::Result<()> {
        self.calls
            .lock()
            .expect("recorder lock")
            .push(argv.iter().map(|s| (*s).to_string()).collect());
        Ok(())
    }
}

/// Failing spawner: always errors, never forks. Drives the honest-failure
/// status-text path (spawn errors must surface as status text, not panics).
struct FailingSpawner;

impl DaemonSpawner for FailingSpawner {
    fn spawn(&self, _argv: &[&str]) -> eyre::Result<()> {
        Err(eyre::eyre!("flintshot not on PATH (simulated)"))
    }
}

#[test]
fn start_daemon_spawns_flintshot_daemon_once() {
    let spawner = RecordingSpawner::new();
    let status = daemon_probe::start_daemon(&spawner);
    assert_eq!(
        spawner.calls(),
        vec![vec!["flintshot".to_string(), "daemon".to_string()]],
        "Start daemon must spawn exactly one `flintshot daemon` (binary + subcommand)"
    );
    assert_eq!(
        status, "starting daemon…",
        "a successful spawn reports the transitional status until the subscriber emits DaemonUp"
    );
}

#[test]
fn start_daemon_failure_surfaces_error_in_status_text() {
    let status = daemon_probe::start_daemon(&FailingSpawner);
    assert!(
        status.starts_with("could not start daemon:"),
        "spawn failure must surface as honest status text, not a panic (got {status})"
    );
}

#[test]
fn app_recovers_from_no_daemon_to_connected_on_daemon_up() {
    let mut app = App::new_for_integration_test();

    // Drive into NoDaemon — the subscriber reports the daemon gone.
    app.apply(&Message::DaemonGone);
    assert_eq!(
        app.connection(),
        ConnectionState::NoDaemon,
        "DaemonGone must drop the applet into the NoDaemon UI"
    );

    // DaemonUp flips it back — the recovery transition a *Start daemon*
    // click is aiming for.
    app.apply(&Message::DaemonUp);
    assert_eq!(
        app.connection(),
        ConnectionState::Connected,
        "DaemonUp must restore the full-action Connected UI"
    );
}
