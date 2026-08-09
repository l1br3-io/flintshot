//! Worker spawn module: the daemon launches `flintshot` child processes
//! for capture+editor work (ADR-0019), never capturing in-process.
//!
//! Why spawn children instead of capturing in the daemon: isolation (an
//! editor crash can't kill the tray) and reuse (the Phase 1
//! `region`/`fullscreen` CLI path runs verbatim). The binary is resolved
//! via [`std::env::current_exe`], not `PATH`, so the worker is always
//! the same `flintshot` as the daemon.
//!
//! The dispatch table maps a [`Request`] to a [`Spawner`] call plus the
//! [`Event`]s to emit, and is unit-testable via [`RecordingSpawner`]
//! without forking a single process.

use color_eyre::eyre;
use flintshot_core::ipc::{Event, Request};

/// What the daemon should do after [`dispatch`] handles a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchOutcome {
    /// Keep running — the request was serviced (or reported unsupported).
    Continue,
    /// Shut the daemon down — [`Request::Quit`] (used by `--replace`,
    /// Task 006).
    Shutdown,
}

/// Launches a `flintshot` worker subprocess. The real impl
/// ([`CommandSpawner`]) resolves the binary via `current_exe`; tests use
/// [`RecordingSpawner`].
pub trait Spawner {
    /// Spawn the worker with `argv` (the subcommand + flags, **excluding**
    /// the binary path). Errors if the child cannot be launched.
    fn spawn(&self, argv: &[&str]) -> eyre::Result<()>;
}

/// Real spawner: runs `std::env::current_exe()` with `argv`, detached.
///
/// Each worker gets a lightweight "reaper" thread that blocks on
/// [`std::process::Child::wait`] so the worker is reaped when its editor
/// closes rather than lingering as a zombie while the daemon runs
/// (captures are infrequent, so a thread-per-worker is cheap).
#[derive(Debug, Default, Clone, Copy)]
pub struct CommandSpawner;

impl Spawner for CommandSpawner {
    fn spawn(&self, argv: &[&str]) -> eyre::Result<()> {
        let exe = std::env::current_exe()
            .map_err(|e| eyre::eyre!("resolving current_exe for worker: {e}"))?;
        let mut cmd = std::process::Command::new(&exe);
        cmd.args(argv)
            // The editor reads nothing from stdin and writes nothing to
            // stdout; null both so worker output can't interleave with
            // the daemon's. stderr is inherited so tracing logs survive
            // for debugging.
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null());
        let mut child = cmd
            .spawn()
            .map_err(|e| eyre::eyre!("spawning worker {exe:?} {argv:?}: {e}"))?;

        // Reap when the worker exits. Dropping the JoinHandle detaches
        // the reaper thread itself (it ends as soon as wait returns).
        let _reaper = std::thread::Builder::new()
            .name("flintshot-worker-reaper".into())
            .spawn(move || {
                let _ = child.wait();
            })
            .map_err(|e| eyre::eyre!("spawning worker reaper thread: {e}"))?;
        Ok(())
    }
}

/// Dispatch a [`Request`]: spawn the right worker (if any), emit the
/// resulting [`Event`]s to `sink`, and report whether the daemon should
/// keep running or shut down.
///
/// `CaptureStarted` is emitted the moment a worker is launched; later
/// events (`EditorOpened`/`Saved`) need the worker to report back, which
/// is wired in Task 006. `Quit` requests shutdown (the caller performs
/// it). Requests for unbuilt phases emit `Error { code: "unsupported" }`.
///
/// # Errors
/// Propagates a [`Spawner::spawn`] failure (no event is emitted on
/// failure).
#[allow(
    clippy::wildcard_enum_match_arm,
    reason = "Request is #[non_exhaustive]; `_` is mandatory and groups the unsupported cases"
)]
pub fn dispatch(
    request: &Request,
    spawner: &dyn Spawner,
    sink: &mut impl FnMut(Event),
) -> eyre::Result<DispatchOutcome> {
    match request {
        Request::CaptureRegion { monitor, delay_ms } => {
            let argv = region_argv(*monitor, *delay_ms);
            sink(Event::CaptureStarted);
            spawn_argv(spawner, &argv)?;
            // The worker process is confirmed launched (spawn returned
            // Ok). True "editor window opened" detection needs the
            // worker to report back over IPC; until then we emit
            // `EditorOpened` as a process-started proxy.
            sink(Event::EditorOpened);
            Ok(DispatchOutcome::Continue)
        }
        Request::CaptureFullscreen { monitor } => {
            let argv = fullscreen_argv(*monitor);
            sink(Event::CaptureStarted);
            spawn_argv(spawner, &argv)?;
            sink(Event::EditorOpened);
            Ok(DispatchOutcome::Continue)
        }
        Request::Quit => Ok(DispatchOutcome::Shutdown),
        // ReportSaved is a worker-originated state update. The IPC
        // connection layer intercepts it to record the path in the
        // daemon's last-saved state (it owns that state; dispatch does
        // not). This defensive arm ensures dispatch never reports
        // ReportSaved as "unsupported" if a future refactor routes it
        // here — it is a silent no-op. See ADR-0023.
        Request::ReportSaved { .. } => Ok(DispatchOutcome::Continue),
        // Subscribe (handled by the connection layer in Task 004),
        // OpenLast/PinLast/UploadLast (their phases), and any future
        // variant: report unsupported rather than pretending to act.
        _ => {
            sink(Event::Error {
                code: "unsupported".to_string(),
                message: format!(
                    "`{}` is not implemented in this phase of flintshot",
                    variant_name(request)
                ),
            });
            Ok(DispatchOutcome::Continue)
        }
    }
}

/// Build the `flintshot region [--monitor N] [--delay-ms M]` argv.
fn region_argv(monitor: Option<u32>, delay_ms: u32) -> Vec<String> {
    let mut argv = vec!["region".to_string()];
    if let Some(m) = monitor {
        argv.push("--monitor".into());
        argv.push(m.to_string());
    }
    if delay_ms > 0 {
        argv.push("--delay-ms".into());
        argv.push(delay_ms.to_string());
    }
    argv
}

/// Build the `flintshot fullscreen [--monitor N]` argv.
fn fullscreen_argv(monitor: Option<u32>) -> Vec<String> {
    let mut argv = vec!["fullscreen".to_string()];
    if let Some(m) = monitor {
        argv.push("--monitor".into());
        argv.push(m.to_string());
    }
    argv
}

/// Hand an owned-string argv to [`Spawner::spawn`] (which takes `&[&str]`).
fn spawn_argv(spawner: &dyn Spawner, argv: &[String]) -> eyre::Result<()> {
    let refs: Vec<&str> = argv.iter().map(String::as_str).collect();
    spawner.spawn(&refs)
}

/// Short human-readable variant name for error messages (avoids the
/// full-struct `Debug` of the field-carrying variants).
#[allow(
    clippy::wildcard_enum_match_arm,
    reason = "Request is #[non_exhaustive]; `_` is mandatory"
)]
fn variant_name(request: &Request) -> &'static str {
    match request {
        Request::CaptureRegion { .. } => "capture-region",
        Request::CaptureFullscreen { .. } => "capture-fullscreen",
        Request::Quit => "quit",
        Request::Subscribe => "subscribe",
        Request::ReportSaved { .. } => "report-saved",
        Request::OpenLast => "open-last",
        Request::PinLast => "pin-last",
        Request::UploadLast { .. } => "upload-last",
        _ => "request",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Test spawner: records every spawn's argv, never forks.
    struct RecordingSpawner {
        calls: Mutex<Vec<Vec<String>>>,
    }

    impl RecordingSpawner {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
            }
        }
        fn snapshot(&self) -> Vec<Vec<String>> {
            self.calls.lock().expect("recorder lock").clone()
        }
    }

    impl Spawner for RecordingSpawner {
        fn spawn(&self, argv: &[&str]) -> eyre::Result<()> {
            self.calls
                .lock()
                .expect("recorder lock")
                .push(argv.iter().map(|s| (*s).to_string()).collect());
            Ok(())
        }
    }

    /// A spawner that always fails — proves spawn errors propagate
    /// without emitting an event.
    struct FailSpawner;

    impl Spawner for FailSpawner {
        fn spawn(&self, _argv: &[&str]) -> eyre::Result<()> {
            Err(eyre::eyre!("simulated spawn failure"))
        }
    }

    #[test]
    fn capture_region_spawns_region_subcommand_and_emits_started() {
        let spawner = RecordingSpawner::new();
        let mut events = Vec::new();
        let outcome = dispatch(
            &Request::CaptureRegion {
                monitor: Some(1),
                delay_ms: 200,
            },
            &spawner,
            &mut |ev| events.push(ev),
        )
        .unwrap();

        assert_eq!(outcome, DispatchOutcome::Continue);
        assert_eq!(
            spawner.snapshot(),
            vec![vec![
                "region".to_string(),
                "--monitor".into(),
                "1".into(),
                "--delay-ms".into(),
                "200".into(),
            ]]
        );
        assert_eq!(
            events,
            vec![Event::CaptureStarted, Event::EditorOpened],
            "CaptureStarted on receipt, EditorOpened once the worker is confirmed running"
        );
    }

    #[test]
    fn capture_region_omits_flags_at_defaults() {
        let spawner = RecordingSpawner::new();
        dispatch(
            &Request::CaptureRegion {
                monitor: None,
                delay_ms: 0,
            },
            &spawner,
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(
            spawner.snapshot(),
            vec![vec!["region".to_string()]],
            "no monitor / zero delay should yield a bare `region`"
        );
    }

    #[test]
    fn capture_fullscreen_spawns_fullscreen_subcommand() {
        let spawner = RecordingSpawner::new();
        let mut events = Vec::new();
        dispatch(
            &Request::CaptureFullscreen { monitor: None },
            &spawner,
            &mut |ev| events.push(ev),
        )
        .unwrap();
        assert_eq!(spawner.snapshot(), vec![vec!["fullscreen".to_string()]]);
        assert_eq!(events, vec![Event::CaptureStarted, Event::EditorOpened]);
    }

    #[test]
    fn capture_fullscreen_passes_monitor() {
        let spawner = RecordingSpawner::new();
        dispatch(
            &Request::CaptureFullscreen { monitor: Some(2) },
            &spawner,
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(
            spawner.snapshot(),
            vec![vec![
                "fullscreen".to_string(),
                "--monitor".into(),
                "2".into()
            ]]
        );
    }

    #[test]
    fn quit_signals_shutdown_without_spawning_or_emitting() {
        let spawner = RecordingSpawner::new();
        let mut events = Vec::new();
        let outcome = dispatch(&Request::Quit, &spawner, &mut |ev| events.push(ev)).unwrap();
        assert_eq!(outcome, DispatchOutcome::Shutdown);
        assert!(
            spawner.snapshot().is_empty(),
            "Quit must not spawn a worker"
        );
        assert!(
            events.is_empty(),
            "Quit signals shutdown; it does not emit an event itself"
        );
    }

    #[test]
    fn unsupported_requests_emit_error_and_continue() {
        let cases = [
            Request::Subscribe,
            Request::OpenLast,
            Request::PinLast,
            Request::UploadLast {
                backend: "imgur".into(),
            },
        ];
        for request in cases {
            let spawner = RecordingSpawner::new();
            let mut events = Vec::new();
            let outcome = dispatch(&request, &spawner, &mut |ev| events.push(ev)).unwrap();
            assert_eq!(
                outcome,
                DispatchOutcome::Continue,
                "stayed running for {request:?}"
            );
            assert!(spawner.snapshot().is_empty(), "no worker for {request:?}");
            assert_eq!(events.len(), 1, "exactly one event for {request:?}");
            match &events[0] {
                Event::Error { code, message } => {
                    assert_eq!(code, "unsupported");
                    assert!(!message.is_empty());
                }
                other => panic!("expected Error event, got {other:?}"),
            }
        }
    }

    #[test]
    fn spawn_failure_propagates_after_capture_started() {
        let mut events = Vec::new();
        let result = dispatch(
            &Request::CaptureRegion {
                monitor: None,
                delay_ms: 0,
            },
            &FailSpawner,
            &mut |ev| events.push(ev),
        );
        assert!(result.is_err(), "spawn failure must propagate");
        // CaptureStarted was emitted "on receipt" (before spawn); the
        // EditorOpened event is suppressed since the worker never ran.
        assert_eq!(
            events,
            vec![Event::CaptureStarted],
            "CaptureStarted on receipt; EditorOpened suppressed on spawn failure"
        );
    }

    #[test]
    fn report_saved_is_a_no_op_in_dispatch() {
        // ReportSaved is a state-update handled in the IPC connection
        // layer, not the spawn table. dispatch must treat it as a silent
        // Continue — no worker spawn, no event — so it never emits an
        // "unsupported" error if routed here.
        let spawner = RecordingSpawner::new();
        let mut events = Vec::new();
        let outcome = dispatch(
            &Request::ReportSaved {
                path: std::path::PathBuf::from("/tmp/x.png"),
            },
            &spawner,
            &mut |ev| events.push(ev),
        )
        .unwrap();
        assert_eq!(outcome, DispatchOutcome::Continue);
        assert!(spawner.snapshot().is_empty(), "ReportSaved spawns nothing");
        assert!(events.is_empty(), "ReportSaved emits nothing from dispatch");
    }

    #[test]
    fn variant_name_covers_every_current_variant() {
        // variant_name feeds "unsupported" error messages; it must name
        // every current variant so messages stay readable (and never
        // fall through to the generic "request").
        let cases = [
            (
                Request::CaptureRegion {
                    monitor: None,
                    delay_ms: 0,
                },
                "capture-region",
            ),
            (
                Request::CaptureFullscreen { monitor: None },
                "capture-fullscreen",
            ),
            (Request::Quit, "quit"),
            (Request::Subscribe, "subscribe"),
            (
                Request::ReportSaved {
                    path: std::path::PathBuf::from("/tmp/x.png"),
                },
                "report-saved",
            ),
            (Request::OpenLast, "open-last"),
            (Request::PinLast, "pin-last"),
            (
                Request::UploadLast {
                    backend: "imgur".into(),
                },
                "upload-last",
            ),
        ];
        for (request, expected) in cases {
            assert_eq!(
                variant_name(&request),
                expected,
                "variant_name mismatch for {request:?}"
            );
        }
    }
}
