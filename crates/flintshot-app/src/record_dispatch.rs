//! Daemon-side record dispatch — the single-recording gate and the
//! record worker's lifecycle (U3).
//!
//! `RecordStart` (IPC) spawns `flintshot record --json-events` as a
//! child with **piped stdout** — its own spawn path, deliberately
//! not [`crate::worker::CommandSpawner`], which nulls stdout (plan
//! decision 3; the json-events lines are how the daemon learns
//! started/stopped/failed). A reader thread parses those lines and
//! broadcasts the matching IPC events; a reaper thread catches a
//! worker that dies without a final line (crash isolation: the
//! daemon broadcasts `RecordingFailed` and stays alive itself).
//!
//! One recording at a time — the gate refuses a second
//! `RecordStart` while a worker lives, mirroring
//! [`crate::worker::CaptureDispatch`]'s serialized dispatch.
//!
//! Two report paths converge on [`finalize_recording_event`]:
//! the daemon-spawned worker's stdout `stopped` line (reader
//! thread), and a standalone `flintshot record` run reporting
//! [`flintshot_core::ipc::Request::ReportRecorded`] over the socket
//! (intercepted in `ipc_server`). The worker never does both —
//! `--json-events` mode skips the socket report.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use flintshot_core::ipc::Event as IpcEvent;

use crate::ipc_server::SubscriberBus;

/// How long a `RecordStop`'d worker gets to finalize (print its
/// `stopped` line) before the SIGKILL fallback.
pub const STOP_GRACE: Duration = Duration::from_secs(10);

/// Post-exit grace the reaper grants the reader to deliver the
/// worker's final line before declaring a crash: the child's **exit
/// status becoming visible** and the reader **consuming the last
/// pipe line** are separate events, and a scheduling-late reader
/// must not turn a healthy finalize into `RecordingFailed`. The flag
/// is set the moment the line is parsed (before the broadcast), so
/// this grace only ever waits on thread scheduling — 2 s is orders
/// beyond any healthy drain.
const READER_GRACE: Duration = Duration::from_secs(2);

/// A running record worker.
#[derive(Clone)]
struct Running {
    pid: u32,
    child: Arc<Mutex<Option<Child>>>,
    /// Set by the reader thread the moment a final (`stopped` /
    /// `failed`) line arrives. The reaper's crash decision waits on
    /// this — not on the gate — so the gate can be cleared promptly
    /// on exit while the crash verdict still honors a slow reader.
    finalized: Arc<AtomicBool>,
}

/// The daemon-wide record dispatch context. Cloneable handle; the
/// gate lives behind the shared `Arc`.
#[derive(Clone)]
pub struct RecordDispatch {
    inner: Arc<Inner>,
}

struct Inner {
    /// `Some` while a record worker lives; the single-recording gate.
    gate: Mutex<Option<Running>>,
    /// Bus for `Recording*` broadcasts; `None` in tests that assert
    /// through other seams.
    bus: Option<Arc<SubscriberBus>>,
    /// Spawn seam: the real binary in production, shell scripts in
    /// tests.
    spawner: Box<dyn RecordSpawner + Send + Sync>,
    /// Stop-grace override for tests.
    stop_grace: Duration,
}

/// Spawns the record worker with **piped stdout**. The production
/// impl resolves the binary via `current_exe` (the daemon/worker
/// ADR-0019 rule); tests substitute shell scripts that speak the
/// same json-events lines.
pub trait RecordSpawner: Send + Sync {
    /// Spawn `argv` (excluding the binary); stdout must be piped,
    /// stderr inherited, stdin null.
    fn spawn_record(&self, argv: &[String]) -> std::io::Result<Child>;
}

/// The production spawner: `current_exe` + argv.
#[derive(Default)]
pub struct ExeRecordSpawner;

impl RecordSpawner for ExeRecordSpawner {
    fn spawn_record(&self, argv: &[String]) -> std::io::Result<Child> {
        let exe = std::env::current_exe()?;
        Command::new(exe)
            .args(argv)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
    }
}

impl Default for RecordDispatch {
    /// Test/default seam: no bus (broadcasts are no-ops), real
    /// spawner. Production wiring goes through [`Self::new`].
    fn default() -> Self {
        Self {
            inner: Arc::new(Inner {
                gate: Mutex::new(None),
                bus: None,
                spawner: Box::new(ExeRecordSpawner),
                stop_grace: STOP_GRACE,
            }),
        }
    }
}

impl RecordDispatch {
    /// Production dispatch: real spawner, live bus, 10s stop grace.
    #[must_use]
    pub fn new(bus: SubscriberBus) -> Self {
        Self {
            inner: Arc::new(Inner {
                gate: Mutex::new(None),
                bus: Some(Arc::new(bus)),
                spawner: Box::new(ExeRecordSpawner),
                stop_grace: STOP_GRACE,
            }),
        }
    }

    /// The argv the daemon uses: the record verb in daemon mode.
    #[must_use]
    pub fn worker_argv() -> Vec<String> {
        vec!["record".into(), "--json-events".into()]
    }

    /// Start a recording. Refused (as a plain `Err` string — the IPC
    /// layer turns it into a `RecordingFailed` broadcast) while one
    /// is already running.
    ///
    /// # Errors
    /// `Err` when a recording is already live or the worker failed
    /// to spawn; the message is broadcast-ready.
    pub fn start(&self, argv: &[String]) -> Result<(), String> {
        let mut gate = self.inner.gate.lock().expect("record gate poisoned");
        if let Some(running) = gate.as_ref() {
            return Err(format!(
                "a recording is already running (pid {}); stop it first",
                running.pid
            ));
        }
        let mut child = self
            .inner
            .spawner
            .spawn_record(argv)
            .map_err(|e| format!("spawning the record worker: {e}"))?;
        let pid = child.id();
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "record worker spawned without stdout".to_string())?;
        let child = Arc::new(Mutex::new(Some(child)));
        let finalized = Arc::new(AtomicBool::new(false));
        *gate = Some(Running {
            pid,
            child: Arc::clone(&child),
            finalized: Arc::clone(&finalized),
        });
        drop(gate);

        if let Some(bus) = &self.inner.bus {
            let bus = Arc::clone(bus);
            std::thread::spawn(move || read_worker_lines(stdout, &bus, &finalized));
        } else {
            std::thread::spawn(move || {
                // Drain anyway so the worker's pipe never fills.
                let _ = BufReader::new(stdout).lines().count();
            });
        }

        let inner = Arc::clone(&self.inner);
        std::thread::spawn(move || reap_worker(&inner, pid, &child));
        Ok(())
    }

    /// Stop the running recording: SIGTERM (the worker's handler
    /// finalizes and prints `stopped`), then SIGKILL after the stop
    /// grace. The final event arrives via the reader (graceful:
    /// `stopped` line) or the reaper (killed without one:
    /// `RecordingFailed`) — stop itself never clears the gate, so
    /// the crash-isolation report can never be suppressed.
    ///
    /// # Errors
    /// `Err` when nothing is recording.
    pub fn stop(&self) -> Result<(), String> {
        let running = {
            let gate = self.inner.gate.lock().expect("record gate poisoned");
            let Some(running) = gate.as_ref() else {
                return Err("nothing is recording".into());
            };
            Running {
                pid: running.pid,
                child: Arc::clone(&running.child),
                finalized: Arc::clone(&running.finalized),
            }
        };
        sigterm(running.pid);
        let deadline = Instant::now() + self.inner.stop_grace;
        loop {
            let exited = running
                .child
                .lock()
                .expect("record child mutex poisoned")
                .as_mut()
                .is_some_and(|child| child.try_wait().map_or(true, |s| s.is_some()));
            if exited || Instant::now() > deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        if let Some(child) = running
            .child
            .lock()
            .expect("record child mutex poisoned")
            .as_mut()
        {
            let _ = child.kill(); // already-exited is harmless
        }
        Ok(())
    }
}

/// Parse the worker's json-events lines, broadcasting the matching
/// IPC events. `finalized` is set (before the broadcast, so the
/// reaper can never observe a final event without its flag) the
/// moment a final `stopped`/`failed` line arrives — the reaper's
/// crash verdict keys off it.
fn read_worker_lines(
    stdout: std::process::ChildStdout,
    bus: &SubscriberBus,
    finalized: &AtomicBool,
) {
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        match value.get("type").and_then(|t| t.as_str()) {
            Some("started") => bus.broadcast(IpcEvent::RecordingStarted),
            Some("stopped") => {
                let path = value
                    .get("path")
                    .and_then(|p| p.as_str())
                    .map(PathBuf::from)
                    .unwrap_or_default();
                finalized.store(true, Ordering::Release);
                bus.broadcast(finalize_event(&path));
            }
            Some("failed") => {
                let code = value
                    .get("code")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(1);
                let message = value
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("recording failed")
                    .to_string();
                finalized.store(true, Ordering::Release);
                bus.broadcast(IpcEvent::RecordingFailed {
                    code: i32::try_from(code).unwrap_or(1),
                    message,
                });
            }
            _ => {}
        }
    }
}

/// The reaper: a worker that exited without a final line is a
/// crash — broadcast `RecordingFailed`, clear the gate, and keep
/// the daemon alive (ADR-0063 crash isolation).
fn reap_worker(inner: &Inner, pid: u32, child: &Arc<Mutex<Option<Child>>>) {
    loop {
        let status = child
            .lock()
            .expect("record child mutex poisoned")
            .as_mut()
            .and_then(|c| c.try_wait().ok().flatten());
        if let Some(_status) = status {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    // Clear the gate promptly on exit (a new recording may start),
    // but defer the crash verdict: the worker's exit status becoming
    // visible and the reader consuming its final pipe line are
    // separate events, so grant the reader READER_GRACE to deliver
    // the final line before declaring "exited without finalizing".
    // Without this, a scheduling-late reader turned healthy
    // finalizes into RecordingFailed (observed as a full-suite
    // flake: the reaper's 50 ms exit poll regularly beat the reader
    // under load).
    let mut gate = inner.gate.lock().expect("record gate poisoned");
    let crashed = gate.as_ref().is_some_and(|r| r.pid == pid);
    if crashed {
        let finalized = Arc::clone(&gate.as_ref().expect("checked pid").finalized);
        *gate = None;
        drop(gate);
        let deadline = Instant::now() + READER_GRACE;
        while !finalized.load(Ordering::Acquire) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(25));
        }
        if !finalized.load(Ordering::Acquire) {
            if let Some(bus) = &inner.bus {
                bus.broadcast(IpcEvent::RecordingFailed {
                    code: 1,
                    message: "the record worker exited without finalizing".into(),
                });
            }
        }
    }
}

/// One finalized recording: recents row (video kind) + the broadcast
/// event — the shared tail of both report paths.
fn finalize_event(path: &std::path::Path) -> IpcEvent {
    flintshot_core::recents::record_video(path, crate::capture_flow::recents_limit());
    IpcEvent::RecordingStopped {
        path: path.to_path_buf(),
    }
}

/// Best-effort SIGTERM (std's `kill()` is SIGKILL-only): shell out
/// to `/bin/kill -TERM`, ignoring failures (an already-dead pid is
/// harmless; the reaper still runs).
fn sigterm(pid: u32) {
    let _ = Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc_server::SubscriberBus;
    use flintshot_core::ipc::Event;

    /// A spawner whose "worker" is a shell script emitting the given
    /// json-events lines, sleeping `sleep_secs` between.
    struct ScriptSpawner {
        lines_before: Vec<String>,
        sleep_secs: f64,
        lines_after: Vec<String>,
    }

    impl RecordSpawner for ScriptSpawner {
        fn spawn_record(&self, _argv: &[String]) -> std::io::Result<Child> {
            use std::fmt::Write as _;
            let mut script = String::new();
            for line in &self.lines_before {
                let _ = write!(script, "echo '{line}'; ");
            }
            let _ = write!(script, "sleep {}; ", self.sleep_secs);
            for line in &self.lines_after {
                let _ = write!(script, "echo '{line}'; ");
            }
            Command::new("/bin/sh")
                .arg("-c")
                .arg(script)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
        }
    }

    fn dispatch_with(spawner: ScriptSpawner, grace: Duration) -> RecordDispatch {
        RecordDispatch {
            inner: Arc::new(Inner {
                gate: Mutex::new(None),
                bus: Some(Arc::new(SubscriberBus::new())),
                spawner: Box::new(spawner),
                stop_grace: grace,
            }),
        }
    }

    fn bus_of(dispatch: &RecordDispatch) -> Arc<SubscriberBus> {
        Arc::clone(
            dispatch
                .inner
                .bus
                .as_ref()
                .expect("test dispatch always carries a bus"),
        )
    }

    async fn wait_for_event(rx: &mut tokio::sync::broadcast::Receiver<Event>, secs: u64) -> Event {
        tokio::time::timeout(Duration::from_secs(secs), rx.recv())
            .await
            .expect("event arrives within budget")
            .expect("bus still live")
    }

    fn started_line() -> String {
        r#"{"type":"started","path":"/tmp/t.mp4"}"#.into()
    }
    fn stopped_line(path: &str) -> String {
        format!(r#"{{"type":"stopped","path":"{path}","bytes":123}}"#)
    }

    #[tokio::test]
    async fn record_dispatch_events_flow() {
        let dispatch = dispatch_with(
            ScriptSpawner {
                lines_before: vec![started_line()],
                sleep_secs: 0.2,
                lines_after: vec![stopped_line("/tmp/flow.mp4")],
            },
            STOP_GRACE,
        );
        let mut rx = bus_of(&dispatch).subscribe();
        dispatch
            .start(&RecordDispatch::worker_argv())
            .expect("first start accepted");

        assert!(matches!(
            wait_for_event(&mut rx, 5).await,
            Event::RecordingStarted
        ));
        match wait_for_event(&mut rx, 5).await {
            Event::RecordingStopped { path } => {
                assert_eq!(path, PathBuf::from("/tmp/flow.mp4"));
            }
            other => panic!("expected RecordingStopped, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn record_gate_refuses_second() {
        let dispatch = dispatch_with(
            ScriptSpawner {
                lines_before: vec![],
                sleep_secs: 5.0, // long enough to stay "running"
                lines_after: vec![],
            },
            STOP_GRACE,
        );
        dispatch
            .start(&["record".into(), "--json-events".into()])
            .expect("first start accepted");
        let refused = dispatch
            .start(&["record".into(), "--json-events".into()])
            .expect_err("second start must be refused");
        assert!(refused.contains("already running"), "{refused}");

        // After a stop, the gate opens again.
        dispatch.stop().expect("stop signals the worker");
        // The reaper clears asynchronously; give it the grace-free
        // path a moment.
        std::thread::sleep(Duration::from_millis(200));
        assert!(dispatch.start(&[]).is_ok(), "gate reopens after stop");
    }

    #[tokio::test]
    async fn worker_kill_isolated() {
        // A worker that ignores SIGTERM and never prints a final
        // line: zero grace SIGKILLs it, the reaper broadcasts the
        // failure, and the daemon-side dispatch stays usable.
        let dispatch = dispatch_with(
            ScriptSpawner {
                lines_before: vec![started_line()],
                sleep_secs: 30.0,
                lines_after: vec![],
            },
            Duration::ZERO,
        );
        let mut rx = bus_of(&dispatch).subscribe();
        dispatch.start(&[]).expect("start accepted");
        assert!(matches!(
            wait_for_event(&mut rx, 5).await,
            Event::RecordingStarted
        ));

        dispatch.stop().expect("stop kills");
        match wait_for_event(&mut rx, 5).await {
            Event::RecordingFailed { code, message } => {
                assert_eq!(code, 1);
                assert!(message.contains("without finalizing"), "{message}");
            }
            other => panic!("expected RecordingFailed, got {other:?}"),
        }
        // The dispatch survived: it can start again.
        assert!(dispatch.start(&[]).is_ok());
    }
}
