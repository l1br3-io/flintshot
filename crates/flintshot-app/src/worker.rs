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

/// Dispatch a [`Request`]: spawn the right worker (if any), broadcast
/// the resulting [`Event`]s via `state.bus`, and report whether the
/// daemon should keep running or shut down.
///
/// `CaptureStarted` is emitted the moment a worker is launched. The
/// worker reports its final editor outcome back over IPC via
/// `Request::ReportSaved`, which the IPC server
/// intercepts to store the path + broadcast `Event::Saved`;
/// `EditorOpened` remains a process-started proxy (there is no
/// worker→daemon "opened" report). `Quit` requests shutdown (the caller
/// performs it). `PinLast` reads `state.last_saved` and spawns
/// `flintshot pin <path>`, tagging the result with a monotonic pin id
/// from `state.next_pin_id()`. Requests for unbuilt phases emit
/// `Error { code: "unsupported" }`.
///
/// # Errors
/// Propagates a [`Spawner::spawn`] failure (no event is emitted on
/// failure for capture requests; for `PinLast` the error propagates
/// before the `Pinned` event is broadcast, mirroring capture's
/// failure-ordering).
#[allow(
    clippy::wildcard_enum_match_arm,
    reason = "Request is #[non_exhaustive]; `_` is mandatory and groups the unsupported cases"
)]
pub fn dispatch(
    request: &Request,
    spawner: &dyn Spawner,
    state: &crate::daemon_state::DaemonState,
) -> eyre::Result<DispatchOutcome> {
    match request {
        Request::CaptureRegion { monitor, delay_ms } => {
            let argv = region_argv(*monitor, *delay_ms);
            state.bus.broadcast(Event::CaptureStarted);
            spawn_argv(spawner, &argv)?;
            // The worker process is confirmed launched (spawn returned
            // Ok). The worker reports `Saved` back over IPC
            // (`Request::ReportSaved`, handled in `handle_connection`);
            // `EditorOpened` has no worker→daemon report yet, so it
            // remains a process-started proxy.
            state.bus.broadcast(Event::EditorOpened);
            Ok(DispatchOutcome::Continue)
        }
        Request::CaptureFullscreen { monitor } => {
            let argv = fullscreen_argv(*monitor);
            state.bus.broadcast(Event::CaptureStarted);
            spawn_argv(spawner, &argv)?;
            state.bus.broadcast(Event::EditorOpened);
            Ok(DispatchOutcome::Continue)
        }
        Request::Quit => Ok(DispatchOutcome::Shutdown),
        // RATIONALE: PinLast reuses the worker-spawn model (ADR-0019)
        // and the pin-as-process model (ADR-0023) — the daemon spawns
        // `flintshot pin <path>` exactly like a capture, so the pin
        // outlives the daemon and crashes independently. `last_saved`
        // is populated by the editor's ReportSaved (Task 002) or
        // ReportPinned-on-save (Task 003). The path is cloned out
        // before spawn so the Mutex isn't held across fork/exec.
        Request::PinLast => {
            // RATIONALE: PinLast reuses the worker-spawn model (ADR-0019)
            // and the pin-as-process model (ADR-0023) — the daemon spawns
            // `flintshot pin <path>` exactly like a capture, so the pin
            // outlives the daemon and crashes independently. `last_saved`
            // is populated by the editor's ReportSaved (Task 002) or the
            // editor's Pin button (Task 003). The path is cloned out
            // before spawn so the Mutex isn't held across fork/exec.
            if let Some(path) = state.last_saved() {
                let argv = pin_argv(&path);
                spawn_argv(spawner, &argv)?;
                let id = state.next_pin_id();
                state.bus.broadcast(Event::Pinned { id });
            } else {
                state.bus.broadcast(Event::Error {
                    code: "no-last-capture".to_string(),
                    message: "no capture has been saved yet; save or \
                        pin from the editor first"
                        .to_string(),
                });
            }
            Ok(DispatchOutcome::Continue)
        }
        // Subscribe (handled by the connection layer), OpenLast /
        // UploadLast (their phases), and any future variant: report
        // unsupported rather than pretending to act.
        _ => {
            state.bus.broadcast(Event::Error {
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

/// Build the `flintshot pin <path>` argv for the editor's Pin button and
/// the daemon's `PinLast` (Task 004). The path is converted via
/// `to_string_lossy` — the pin-cache dir lives under `dirs::cache_dir`
/// (or the temp fallback), which is UTF-8 on every supported platform;
/// non-UTF8 cache dirs are exotic and would surface as a "no such file"
/// spawn error rather than silent corruption in practice.
#[must_use]
pub fn pin_argv(path: &std::path::Path) -> Vec<String> {
    vec!["pin".to_string(), path.to_string_lossy().into_owned()]
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
        Request::OpenLast => "open-last",
        Request::PinLast => "pin-last",
        Request::UploadLast { .. } => "upload-last",
        _ => "request",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon_state::DaemonState;
    use crate::ipc_server::SubscriberBus;
    use std::sync::Mutex;

    /// Drain every pending event from a broadcast receiver into a `Vec`.
    /// `dispatch` broadcasts synchronously, so by the time it returns
    /// every event is already in the channel — `try_recv` collects them
    /// without an async runtime.
    fn drain_events(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<Event> {
        let mut out = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            out.push(ev);
        }
        out
    }

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
        let state = DaemonState::new();
        let mut rx = state.bus.subscribe();
        let outcome = dispatch(
            &Request::CaptureRegion {
                monitor: Some(1),
                delay_ms: 200,
            },
            &spawner,
            &state,
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
            drain_events(&mut rx),
            vec![Event::CaptureStarted, Event::EditorOpened],
            "CaptureStarted on receipt, EditorOpened once the worker is confirmed running"
        );
    }

    #[test]
    fn capture_region_omits_flags_at_defaults() {
        let spawner = RecordingSpawner::new();
        let state = DaemonState::new();
        dispatch(
            &Request::CaptureRegion {
                monitor: None,
                delay_ms: 0,
            },
            &spawner,
            &state,
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
        let state = DaemonState::new();
        let mut rx = state.bus.subscribe();
        dispatch(
            &Request::CaptureFullscreen { monitor: None },
            &spawner,
            &state,
        )
        .unwrap();
        assert_eq!(spawner.snapshot(), vec![vec!["fullscreen".to_string()]]);
        assert_eq!(
            drain_events(&mut rx),
            vec![Event::CaptureStarted, Event::EditorOpened]
        );
    }

    #[test]
    fn capture_fullscreen_passes_monitor() {
        let spawner = RecordingSpawner::new();
        let state = DaemonState::new();
        dispatch(
            &Request::CaptureFullscreen { monitor: Some(2) },
            &spawner,
            &state,
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
    fn pin_argv_builds_pin_subcommand_with_path() {
        let argv = pin_argv(std::path::Path::new("/tmp/flintshot/pins/pin-42.png"));
        assert_eq!(
            argv,
            vec!["pin".to_string(), "/tmp/flintshot/pins/pin-42.png".into()]
        );
    }

    #[test]
    fn quit_signals_shutdown_without_spawning_or_emitting() {
        let spawner = RecordingSpawner::new();
        let state = DaemonState::new();
        let mut rx = state.bus.subscribe();
        let outcome = dispatch(&Request::Quit, &spawner, &state).unwrap();
        assert_eq!(outcome, DispatchOutcome::Shutdown);
        assert!(
            spawner.snapshot().is_empty(),
            "Quit must not spawn a worker"
        );
        assert!(
            drain_events(&mut rx).is_empty(),
            "Quit signals shutdown; it does not emit an event itself"
        );
    }

    #[test]
    fn unsupported_requests_emit_error_and_continue() {
        // PinLast is handled (see `pin_last_*` tests); only Subscribe,
        // OpenLast, and UploadLast remain unsupported.
        let cases = [
            Request::Subscribe,
            Request::OpenLast,
            Request::UploadLast {
                backend: "imgur".into(),
            },
        ];
        for request in cases {
            let spawner = RecordingSpawner::new();
            let state = DaemonState::new();
            let mut rx = state.bus.subscribe();
            let outcome = dispatch(&request, &spawner, &state).unwrap();
            assert_eq!(
                outcome,
                DispatchOutcome::Continue,
                "stayed running for {request:?}"
            );
            assert!(spawner.snapshot().is_empty(), "no worker for {request:?}");
            let events = drain_events(&mut rx);
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
        let state = DaemonState::new();
        let mut rx = state.bus.subscribe();
        let result = dispatch(
            &Request::CaptureRegion {
                monitor: None,
                delay_ms: 0,
            },
            &FailSpawner,
            &state,
        );
        assert!(result.is_err(), "spawn failure must propagate");
        // CaptureStarted was emitted "on receipt" (before spawn); the
        // EditorOpened event is suppressed since the worker never ran.
        assert_eq!(
            drain_events(&mut rx),
            vec![Event::CaptureStarted],
            "CaptureStarted on receipt; EditorOpened suppressed on spawn failure"
        );
    }

    /// `PinLast` with a populated `last_saved` spawns `flintshot pin
    /// <path>`, broadcasts exactly one `Event::Pinned { id }`, and
    /// returns `Continue`.
    #[test]
    fn pin_last_with_saved_path_spawns_pin_and_emits_pinned() {
        let spawner = RecordingSpawner::new();
        let state = DaemonState::new();
        state.record_save(std::path::PathBuf::from("/tmp/x.png"));
        let mut rx = state.bus.subscribe();

        let outcome = dispatch(&Request::PinLast, &spawner, &state).unwrap();

        assert_eq!(outcome, DispatchOutcome::Continue);
        assert_eq!(
            spawner.snapshot(),
            vec![vec!["pin".to_string(), "/tmp/x.png".into()]],
            "PinLast must spawn `flintshot pin <last_saved>`"
        );
        let events = drain_events(&mut rx);
        assert_eq!(events.len(), 1, "exactly one event (Pinned)");
        assert!(
            matches!(events[0], Event::Pinned { .. }),
            "expected Pinned event, got {:?}",
            events[0]
        );
    }

    /// `PinLast` with no prior save broadcasts a clear
    /// `Error { code: "no-last-capture" }` and spawns nothing.
    #[test]
    fn pin_last_without_saved_path_emits_no_last_capture_error() {
        let spawner = RecordingSpawner::new();
        let state = DaemonState::new();
        let mut rx = state.bus.subscribe();

        let outcome = dispatch(&Request::PinLast, &spawner, &state).unwrap();

        assert_eq!(outcome, DispatchOutcome::Continue);
        assert!(
            spawner.snapshot().is_empty(),
            "no path → no spawn (nothing to pin)"
        );
        let events = drain_events(&mut rx);
        assert_eq!(events.len(), 1);
        match &events[0] {
            Event::Error { code, message } => {
                assert_eq!(code, "no-last-capture");
                assert!(!message.is_empty(), "error must carry a human message");
            }
            other => panic!("expected no-last-capture Error, got {other:?}"),
        }
    }

    /// Consecutive `PinLast` calls produce strictly increasing pin ids
    /// (the `next_pin_id` counter is per-daemon monotonic).
    #[test]
    fn pin_last_ids_are_monotonic_across_calls() {
        let spawner = RecordingSpawner::new();
        let state = DaemonState::new();
        state.record_save(std::path::PathBuf::from("/tmp/a.png"));

        let mut rx1 = state.bus.subscribe();
        dispatch(&Request::PinLast, &spawner, &state).unwrap();
        let ev1 = drain_events(&mut rx1).pop().expect("first Pinned event");

        let mut rx2 = state.bus.subscribe();
        dispatch(&Request::PinLast, &spawner, &state).unwrap();
        let ev2 = drain_events(&mut rx2).pop().expect("second Pinned event");

        let (Event::Pinned { id: id1 }, Event::Pinned { id: id2 }) = (ev1, ev2) else {
            panic!("both events must be Pinned");
        };
        assert!(
            id2 > id1,
            "second pin id ({id2}) must exceed the first ({id1})"
        );
    }

    /// `PinLast` spawn failure propagates the error *before* the
    /// `Pinned` event is broadcast (mirrors capture's failure ordering).
    #[test]
    fn pin_last_spawn_failure_propagates_without_emitting_pinned() {
        let state = DaemonState::new();
        state.record_save(std::path::PathBuf::from("/tmp/x.png"));
        let mut rx = state.bus.subscribe();

        let result = dispatch(&Request::PinLast, &FailSpawner, &state);
        assert!(result.is_err(), "PinLast spawn failure must propagate");
        assert!(
            drain_events(&mut rx).is_empty(),
            "no Pinned event on spawn failure"
        );
    }

    /// Suppress unused-import warning for `SubscriberBus` — kept so the
    /// test module's imports document which bus type `DaemonState`
    /// carries without readers jumping files.
    #[allow(dead_code)]
    type _SubscriberBusImport = SubscriberBus;
}
