//! Daemon discovery: connect-or-classify + the *Start daemon* spawn seam.
//!
//! When the applet probes for the daemon and nothing answers, it enters
//! [`NoDaemon`](crate::app::ConnectionState::NoDaemon) and offers a
//! *Start daemon* button. That button spawns `flintshot daemon` detached
//! via a [`DaemonSpawner`] — a trait seam (mirrors Phase 2's
//! `worker::Spawner` at `crates/flintshot-app/src/worker.rs:30`) so the
//! argv + detached-spawn flags are unit-tested without forking a real
//! process.

use color_eyre::eyre;
use std::process::{Command, Stdio};

/// Spawn a detached `flintshot daemon` subprocess. The real impl
/// ([`RealDaemonSpawner`]) resolves `flintshot` on `PATH`; tests use
/// [`RecordingDaemonSpawner`].
///
/// `argv` includes the binary name as its first element (matching
/// [`daemon_argv`]'s shape), e.g. `["flintshot", "daemon"]`.
pub trait DaemonSpawner {
    /// Spawn the subprocess with the given `argv` (binary + args). Errors
    /// if the child cannot be launched.
    ///
    /// # Errors
    /// Propagates an error if the OS `spawn` call fails (e.g. `flintshot`
    /// not on `PATH`) or the reaper thread cannot be created.
    fn spawn(&self, argv: &[&str]) -> eyre::Result<()>;
}

/// Real spawner: runs `flintshot daemon` resolved via `PATH`, detached.
///
/// Mirrors the `worker::CommandSpawner` pattern: stdin/stdout are nulled
/// so daemon output doesn't interleave; stderr is inherited so tracing
/// logs survive for debugging. A lightweight "reaper" thread waits on
/// the child so it is reaped when the daemon exits rather than lingering
/// as a zombie.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealDaemonSpawner;

impl DaemonSpawner for RealDaemonSpawner {
    fn spawn(&self, argv: &[&str]) -> eyre::Result<()> {
        let Some(binary) = argv.first() else {
            return Err(eyre::eyre!("empty daemon argv"));
        };
        let mut cmd = Command::new(binary);
        cmd.args(&argv[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::null());
        let mut child = cmd
            .spawn()
            .map_err(|e| eyre::eyre!("spawning daemon {argv:?}: {e}"))?;

        let _reaper = std::thread::Builder::new()
            .name("flintshot-daemon-reaper".into())
            .spawn(move || {
                let _ = child.wait();
            })
            .map_err(|e| eyre::eyre!("spawning daemon reaper thread: {e}"))?;
        Ok(())
    }
}

/// Pure: build the `["flintshot", "daemon"]` argv.
///
/// Factored out of the spawn path so tests assert the argv without
/// forking. The binary is resolved via `PATH` by the spawner — matching
/// the tray's `$EDITOR` assumption for *Open config* (spec Q6).
#[must_use]
pub fn daemon_argv() -> Vec<String> {
    vec!["flintshot".to_string(), "daemon".to_string()]
}

/// Run the *Start daemon* spawn via `spawner` using [`daemon_argv`], and
/// return the status-line text describing the outcome.
///
/// RATIONALE: this is factored out of `App::update`'s `ClickStartDaemon`
/// arm so the argv + honest-failure classification are unit-tested with a
/// recording/failing spawner (no real fork). The real path passes
/// [`RealDaemonSpawner`], which resolves `flintshot` on `PATH` — a
/// production install puts it there; dev runs launch the daemon directly.
/// A spawn failure surfaces as status text (the `Err` arm), never a panic
/// — the "honesty about platform limits" principle from `VISION.md`
/// extended to the daemon lifecycle.
///
/// # Returns
/// - `"starting daemon…"` on `Ok` — the transitional status until the
///   subscriber stream emits [`DaemonUp`](crate::app::Message::DaemonUp).
/// - `"could not start daemon: <e>"` on `Err`.
#[must_use]
pub fn start_daemon(spawner: &dyn DaemonSpawner) -> String {
    let argv = daemon_argv();
    let refs: Vec<&str> = argv.iter().map(String::as_str).collect();
    match spawner.spawn(&refs) {
        Ok(()) => "starting daemon…".to_string(),
        Err(e) => format!("could not start daemon: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Test spawner: records every spawn's argv, never forks.
    /// Mirrors `worker::RecordingSpawner`.
    struct RecordingDaemonSpawner {
        calls: Mutex<Vec<Vec<String>>>,
    }

    impl RecordingDaemonSpawner {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
            }
        }
        fn snapshot(&self) -> Vec<Vec<String>> {
            self.calls.lock().expect("recorder lock").clone()
        }
    }

    impl DaemonSpawner for RecordingDaemonSpawner {
        fn spawn(&self, argv: &[&str]) -> eyre::Result<()> {
            self.calls
                .lock()
                .expect("recorder lock")
                .push(argv.iter().map(|s| (*s).to_string()).collect());
            Ok(())
        }
    }

    #[test]
    fn daemon_argv_is_flintshot_daemon() {
        assert_eq!(
            daemon_argv(),
            vec!["flintshot".to_string(), "daemon".to_string()]
        );
    }

    #[test]
    fn recording_spawner_captures_daemon_argv() {
        let spawner = RecordingDaemonSpawner::new();
        let argv = daemon_argv();
        let refs: Vec<&str> = argv.iter().map(String::as_str).collect();
        spawner
            .spawn(&refs)
            .expect("recording spawn should succeed");
        assert_eq!(
            spawner.snapshot(),
            vec![vec!["flintshot".to_string(), "daemon".to_string()]],
            "spawner must record the full argv including the binary name"
        );
    }

    #[test]
    fn recording_spawner_records_multiple_calls() {
        let spawner = RecordingDaemonSpawner::new();
        let argv = daemon_argv();
        let refs: Vec<&str> = argv.iter().map(String::as_str).collect();
        spawner.spawn(&refs).unwrap();
        spawner.spawn(&refs).unwrap();
        assert_eq!(spawner.snapshot().len(), 2);
    }
}
