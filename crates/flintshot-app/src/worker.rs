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

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use color_eyre::eyre;
use flintshot_core::ipc::{Event, Request};

use crate::ipc_server::LastSaved;

/// What the daemon should do after [`dispatch`] handles a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchOutcome {
    /// Keep running — the request was serviced (or reported unsupported).
    Continue,
    /// Shut the daemon down — [`Request::Quit`] (used by `--replace`,
    /// Task 006).
    Shutdown,
}

/// Monotonic pin-id counter, incremented once per successful `PinLast`.
///
/// Wraps an [`Arc<AtomicU64>`] so clones share the same counter — every
/// dispatch entry point (IPC, tray, hotkey) allocates from the same
/// sequence. [`next`](Self::next) returns 1-based ids (first call → 1).
#[derive(Clone, Default)]
pub struct PinIdSource {
    next: Arc<AtomicU64>,
}

impl PinIdSource {
    /// Create a fresh counter (next id will be 1).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate and return the next pin id.
    pub fn next(&self) -> u64 {
        self.next.fetch_add(1, Ordering::Relaxed) + 1
    }
}

/// Daemon-wide shared state that [`dispatch`] reads or mutates beyond the
/// spawner: the last-saved capture path (set by [`Request::ReportSaved`],
/// read by [`Request::PinLast`] / [`Request::OpenLast`]) and the
/// monotonic pin-id counter.
///
/// Cloning shares the underlying state (both fields are `Arc`-backed),
/// so every entry point observes the same last-saved path and pin-id
/// sequence.
#[derive(Clone)]
pub struct DispatchState {
    /// Last worker-reported save path.
    pub(crate) last_saved: LastSaved,
    /// Monotonic pin-id counter (incremented per successful `PinLast`).
    pub(crate) pin_ids: PinIdSource,
}

impl Default for DispatchState {
    fn default() -> Self {
        Self {
            last_saved: LastSaved::new(),
            pin_ids: PinIdSource::new(),
        }
    }
}

impl DispatchState {
    /// Fresh state: no last save recorded, pin ids starting at 1.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
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
/// it). `PinLast` / `OpenLast` read the last-saved path from `state`
/// (Task 4); with no prior save they emit
/// `Error { code: "no-last-capture" }`. `UploadLast` follows the same
/// last-saved pattern and emits `UploadStarted` optimistically before
/// spawn (Task 007). Requests for unbuilt phases emit
/// `Error { code: "unsupported" }`.
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
    state: &DispatchState,
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
        Request::PinLast => {
            if let Some(path) = state.last_saved.get() {
                let argv = pin_argv(&path);
                spawn_argv(spawner, &argv)?;
                let id = state.pin_ids.next();
                sink(Event::Pinned { id });
            } else {
                sink(Event::Error {
                    code: "no-last-capture".to_string(),
                    message: "no capture has been saved yet".to_string(),
                });
            }
            Ok(DispatchOutcome::Continue)
        }
        Request::OpenLast => {
            if let Some(path) = state.last_saved.get() {
                let argv = open_argv(&path);
                spawn_argv(spawner, &argv)?;
                sink(Event::EditorOpened);
            } else {
                sink(Event::Error {
                    code: "no-last-capture".to_string(),
                    message: "no capture has been saved yet".to_string(),
                });
            }
            Ok(DispatchOutcome::Continue)
        }
        Request::UploadLast { backend } => {
            if let Some(path) = state.last_saved.get() {
                let argv = upload_argv(&path, backend);
                sink(Event::UploadStarted {
                    backend: backend.clone(),
                });
                spawn_argv(spawner, &argv)?;
            } else {
                sink(Event::Error {
                    code: "no-last-capture".to_string(),
                    message: "no capture has been saved yet".to_string(),
                });
            }
            Ok(DispatchOutcome::Continue)
        }
        Request::Quit => Ok(DispatchOutcome::Shutdown),
        // ReportSaved / ReportUploaded are worker-originated state
        // updates. The IPC connection layer intercepts them (it owns the
        // subscriber bus + daemon state; dispatch does not). These
        // defensive arms ensure dispatch never reports them as
        // "unsupported" if a future refactor routes them here — they are
        // silent no-ops. See ADR-0023.
        Request::ReportSaved { .. } | Request::ReportUploaded { .. } => {
            Ok(DispatchOutcome::Continue)
        }
        // The tray-hide handshake (ADR-0036) is likewise
        // connection-layer territory: `HideTrayForCapture` drives the
        // daemon's tray gate for its ack, `ReportCaptured` releases
        // it. Same defensive no-op contract as the reports above.
        Request::HideTrayForCapture | Request::ReportCaptured => Ok(DispatchOutcome::Continue),
        // Subscribe (handled by the connection layer in Task 004) and
        // BindShortcuts (Phase 6: the connection layer intercepts it
        // to drive the daemon's portal session in Task 003), plus any
        // future variant: report unsupported rather than pretending
        // to act.
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

/// Build the `flintshot region [--monitor N] [--delay-ms M]
/// --standalone` argv.
///
/// The trailing `--standalone` breaks the delegation loop: without
/// it, the daemon-spawned worker would re-delegate back to the
/// daemon, which spawns the next worker (~60–100/s observed; see
/// `TEST_MATRIX` "L5 pass findings" 1). `delegate_to_daemon`
/// short-circuits on the flag.
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
    argv.push("--standalone".into());
    argv
}

/// Build the `flintshot fullscreen [--monitor N] --standalone` argv.
/// The `--standalone` guard mirrors [`region_argv`]'s — see its doc
/// comment for the fork-loop rationale.
fn fullscreen_argv(monitor: Option<u32>) -> Vec<String> {
    let mut argv = vec!["fullscreen".to_string()];
    if let Some(m) = monitor {
        argv.push("--monitor".into());
        argv.push(m.to_string());
    }
    argv.push("--standalone".into());
    argv
}

/// Built-in default backend id. When `Request::UploadLast`'s `backend`
/// equals this, `--backend` is omitted from the worker argv — the worker
/// resolves the same id from its own config. Tracks
/// [`flintshot_core::UploadConfig`]'s default (`"imgur"`); kept in sync
/// by the `default-backend = "imgur"` serialization round-trip test in
/// `config.rs`.
const DEFAULT_BACKEND: &str = "imgur";

/// Build the `flintshot upload <path> [--backend <backend>]` argv for
/// [`Request::UploadLast`]. Omits `--backend` when `backend` is the
/// built-in default (the worker picks it up from its own config).
fn upload_argv(path: &std::path::Path, backend: &str) -> Vec<String> {
    let mut argv = vec!["upload".to_string(), path.to_string_lossy().into_owned()];
    if backend != DEFAULT_BACKEND {
        argv.push("--backend".into());
        argv.push(backend.to_string());
    }
    argv
}

/// Build the `flintshot pin <path>` argv for [`Request::PinLast`].
fn pin_argv(path: &std::path::Path) -> Vec<String> {
    vec!["pin".to_string(), path.to_string_lossy().into_owned()]
}

/// Build the `flintshot open <path>` argv for [`Request::OpenLast`].
fn open_argv(path: &std::path::Path) -> Vec<String> {
    vec!["open".to_string(), path.to_string_lossy().into_owned()]
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
        Request::ReportUploaded { .. } => "report-uploaded",
        Request::OpenLast => "open-last",
        Request::PinLast => "pin-last",
        Request::UploadLast { .. } => "upload-last",
        Request::BindShortcuts => "bind-shortcuts",
        Request::HideTrayForCapture => "hide-tray-for-capture",
        Request::ReportCaptured => "report-captured",
        _ => "request",
    }
}

/// Does dispatching this request spawn a capture worker (region /
/// fullscreen)? The daemon's main-loop dispatch sites pre-hide the
/// tray icon for exactly these — nothing else freezes the screen, so
/// nothing else should make the icon vanish. See ADR-0036.
#[must_use]
pub fn spawns_capture_worker(request: &Request) -> bool {
    matches!(
        request,
        Request::CaptureRegion { .. } | Request::CaptureFullscreen { .. }
    )
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
        let state = DispatchState::new();
        let mut events = Vec::new();
        let outcome = dispatch(
            &Request::CaptureRegion {
                monitor: Some(1),
                delay_ms: 200,
            },
            &spawner,
            &state,
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
                "--standalone".into(),
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
            &DispatchState::new(),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(
            spawner.snapshot(),
            vec![vec!["region".to_string(), "--standalone".into()]],
            "no monitor / zero delay should yield `region --standalone`"
        );
    }

    #[test]
    fn capture_fullscreen_spawns_fullscreen_subcommand() {
        let spawner = RecordingSpawner::new();
        let state = DispatchState::new();
        let mut events = Vec::new();
        dispatch(
            &Request::CaptureFullscreen { monitor: None },
            &spawner,
            &state,
            &mut |ev| events.push(ev),
        )
        .unwrap();
        assert_eq!(
            spawner.snapshot(),
            vec![vec!["fullscreen".to_string(), "--standalone".into()]]
        );
        assert_eq!(events, vec![Event::CaptureStarted, Event::EditorOpened]);
    }

    #[test]
    fn capture_fullscreen_passes_monitor() {
        let spawner = RecordingSpawner::new();
        dispatch(
            &Request::CaptureFullscreen { monitor: Some(2) },
            &spawner,
            &DispatchState::new(),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(
            spawner.snapshot(),
            vec![vec![
                "fullscreen".to_string(),
                "--monitor".into(),
                "2".into(),
                "--standalone".into()
            ]]
        );
    }

    /// Regression test for the L5 fork loop (`TEST_MATRIX` finding 1):
    /// a daemon-spawned capture worker must not re-delegate back to
    /// the daemon — `run_region` / `run_fullscreen` delegate whenever
    /// the daemon is up unless `--standalone` is set, so its presence
    /// in the argv is what breaks the daemon↔worker spawn cycle.
    #[test]
    fn capture_workers_spawn_standalone_to_break_delegation_loop() {
        for request in [
            Request::CaptureRegion {
                monitor: None,
                delay_ms: 0,
            },
            Request::CaptureFullscreen { monitor: None },
        ] {
            let spawner = RecordingSpawner::new();
            dispatch(&request, &spawner, &DispatchState::new(), &mut |_| {}).unwrap();
            let calls = spawner.snapshot();
            assert_eq!(calls.len(), 1, "one worker for {request:?}");
            assert!(
                calls[0].contains(&"--standalone".to_string()),
                "worker argv must carry --standalone: {:?}",
                calls[0]
            );
        }
    }

    #[test]
    fn quit_signals_shutdown_without_spawning_or_emitting() {
        let spawner = RecordingSpawner::new();
        let mut events = Vec::new();
        let outcome = dispatch(&Request::Quit, &spawner, &DispatchState::new(), &mut |ev| {
            events.push(ev);
        })
        .unwrap();
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
        // PinLast, OpenLast (Task 4), and UploadLast (Task 007) are now
        // supported; BindShortcuts joins Subscribe here until the IPC
        // connection layer intercepts it (Phase 6 Task 003).
        let cases = [Request::Subscribe, Request::BindShortcuts];
        for request in cases {
            let spawner = RecordingSpawner::new();
            let mut events = Vec::new();
            let outcome = dispatch(&request, &spawner, &DispatchState::new(), &mut |ev| {
                events.push(ev);
            })
            .unwrap();
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
            &DispatchState::new(),
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
            &DispatchState::new(),
            &mut |ev| events.push(ev),
        )
        .unwrap();
        assert_eq!(outcome, DispatchOutcome::Continue);
        assert!(spawner.snapshot().is_empty(), "ReportSaved spawns nothing");
        assert!(events.is_empty(), "ReportSaved emits nothing from dispatch");
    }

    #[test]
    fn tray_hide_handshake_requests_are_no_ops_in_dispatch() {
        // The handshake lives in the IPC connection layer (it drives
        // the tray gate); dispatch must not report these as
        // "unsupported" if a refactor routes them here.
        for request in [Request::HideTrayForCapture, Request::ReportCaptured] {
            let spawner = RecordingSpawner::new();
            let mut events = Vec::new();
            let outcome = dispatch(&request, &spawner, &DispatchState::new(), &mut |ev| {
                events.push(ev);
            })
            .unwrap();
            assert_eq!(outcome, DispatchOutcome::Continue, "{request:?}");
            assert!(spawner.snapshot().is_empty(), "{request:?} spawns nothing");
            assert!(events.is_empty(), "{request:?} emits nothing");
        }
    }

    #[test]
    fn spawns_capture_worker_is_true_only_for_captures() {
        let captures = [
            Request::CaptureRegion {
                monitor: None,
                delay_ms: 0,
            },
            Request::CaptureRegion {
                monitor: Some(2),
                delay_ms: 500,
            },
            Request::CaptureFullscreen { monitor: None },
            Request::CaptureFullscreen { monitor: Some(1) },
        ];
        for request in &captures {
            assert!(
                spawns_capture_worker(request),
                "{request:?} freezes the screen: pre-hide applies"
            );
        }
        let others = [
            Request::Quit,
            Request::Subscribe,
            Request::OpenLast,
            Request::PinLast,
            Request::UploadLast {
                backend: "imgur".into(),
            },
            Request::HideTrayForCapture,
            Request::ReportCaptured,
        ];
        for request in &others {
            assert!(
                !spawns_capture_worker(request),
                "{request:?} never grabs the screen: the icon must not vanish"
            );
        }
    }

    #[test]
    fn report_uploaded_is_a_no_op_in_dispatch() {
        let spawner = RecordingSpawner::new();
        let mut events = Vec::new();
        let outcome = dispatch(
            &Request::ReportUploaded {
                url: "https://imgur.com/abc".into(),
                delete_handle: Some("delhash".into()),
            },
            &spawner,
            &DispatchState::new(),
            &mut |ev| events.push(ev),
        )
        .unwrap();
        assert_eq!(outcome, DispatchOutcome::Continue);
        assert!(
            spawner.snapshot().is_empty(),
            "ReportUploaded spawns nothing"
        );
        assert!(
            events.is_empty(),
            "ReportUploaded emits nothing from dispatch"
        );
    }

    #[test]
    fn pin_last_with_saved_path_spawns_pin_and_emits_pinned() {
        let spawner = RecordingSpawner::new();
        let state = DispatchState::new();
        state
            .last_saved
            .set(std::path::PathBuf::from("/tmp/shot.png"));
        let mut events = Vec::new();
        let outcome = dispatch(&Request::PinLast, &spawner, &state, &mut |ev| {
            events.push(ev);
        })
        .unwrap();

        assert_eq!(outcome, DispatchOutcome::Continue);
        assert_eq!(
            spawner.snapshot(),
            vec![vec!["pin".to_string(), "/tmp/shot.png".into()]],
            "PinLast spawns `flintshot pin <last-saved>`"
        );
        assert_eq!(events, vec![Event::Pinned { id: 1 }]);
    }

    #[test]
    fn pin_last_id_is_monotonic() {
        let spawner = RecordingSpawner::new();
        let state = DispatchState::new();
        state.last_saved.set(std::path::PathBuf::from("/tmp/a.png"));

        let mut events1 = Vec::new();
        dispatch(&Request::PinLast, &spawner, &state, &mut |ev| {
            events1.push(ev);
        })
        .unwrap();
        let mut events2 = Vec::new();
        dispatch(&Request::PinLast, &spawner, &state, &mut |ev| {
            events2.push(ev);
        })
        .unwrap();

        assert_eq!(events1, vec![Event::Pinned { id: 1 }]);
        assert_eq!(
            events2,
            vec![Event::Pinned { id: 2 }],
            "second pin gets the next monotonic id"
        );
        assert_eq!(spawner.snapshot().len(), 2, "two pins spawned");
    }

    #[test]
    fn pin_last_without_save_emits_error_and_does_not_spawn() {
        let spawner = RecordingSpawner::new();
        let state = DispatchState::new();
        let mut events = Vec::new();
        let outcome = dispatch(&Request::PinLast, &spawner, &state, &mut |ev| {
            events.push(ev);
        })
        .unwrap();

        assert_eq!(outcome, DispatchOutcome::Continue);
        assert!(
            spawner.snapshot().is_empty(),
            "PinLast with no prior save must not spawn"
        );
        assert_eq!(events.len(), 1, "exactly one event");
        match &events[0] {
            Event::Error { code, .. } => assert_eq!(code, "no-last-capture"),
            other => panic!("expected Error event, got {other:?}"),
        }
    }

    #[test]
    fn pin_last_spawn_failure_propagates_without_pinned() {
        let state = DispatchState::new();
        state.last_saved.set(std::path::PathBuf::from("/tmp/x.png"));
        let mut events = Vec::new();
        let result = dispatch(&Request::PinLast, &FailSpawner, &state, &mut |ev| {
            events.push(ev);
        });

        assert!(result.is_err(), "spawn failure must propagate");
        assert!(
            events.is_empty(),
            "no Pinned event when the pin worker failed to spawn"
        );
    }

    #[test]
    fn open_last_with_saved_path_spawns_open_and_emits_editor_opened() {
        let spawner = RecordingSpawner::new();
        let state = DispatchState::new();
        state
            .last_saved
            .set(std::path::PathBuf::from("/tmp/editor.png"));
        let mut events = Vec::new();
        let outcome = dispatch(&Request::OpenLast, &spawner, &state, &mut |ev| {
            events.push(ev);
        })
        .unwrap();

        assert_eq!(outcome, DispatchOutcome::Continue);
        assert_eq!(
            spawner.snapshot(),
            vec![vec!["open".to_string(), "/tmp/editor.png".into()]],
            "OpenLast spawns `flintshot open <last-saved>`"
        );
        assert_eq!(events, vec![Event::EditorOpened]);
    }

    #[test]
    fn open_last_without_save_emits_error_and_does_not_spawn() {
        let spawner = RecordingSpawner::new();
        let state = DispatchState::new();
        let mut events = Vec::new();
        let outcome = dispatch(&Request::OpenLast, &spawner, &state, &mut |ev| {
            events.push(ev);
        })
        .unwrap();

        assert_eq!(outcome, DispatchOutcome::Continue);
        assert!(
            spawner.snapshot().is_empty(),
            "OpenLast with no prior save must not spawn"
        );
        assert_eq!(events.len(), 1);
        match &events[0] {
            Event::Error { code, .. } => assert_eq!(code, "no-last-capture"),
            other => panic!("expected Error event, got {other:?}"),
        }
    }

    #[test]
    fn upload_last_with_saved_path_spawns_upload_and_emits_started() {
        let spawner = RecordingSpawner::new();
        let state = DispatchState::new();
        state
            .last_saved
            .set(std::path::PathBuf::from("/tmp/shot.png"));
        let mut events = Vec::new();
        let outcome = dispatch(
            &Request::UploadLast {
                backend: "imgur".into(),
            },
            &spawner,
            &state,
            &mut |ev| events.push(ev),
        )
        .unwrap();

        assert_eq!(outcome, DispatchOutcome::Continue);
        assert_eq!(
            spawner.snapshot(),
            vec![vec!["upload".to_string(), "/tmp/shot.png".into()]],
            "UploadLast spawns `flintshot upload <last-saved>` with no \
             --backend when backend is the config default"
        );
        assert_eq!(
            events,
            vec![Event::UploadStarted {
                backend: "imgur".into()
            }],
            "UploadStarted emitted optimistically (mirrors CaptureStarted)"
        );
    }

    #[test]
    fn upload_last_with_non_default_backend_passes_backend_flag() {
        let spawner = RecordingSpawner::new();
        let state = DispatchState::new();
        state
            .last_saved
            .set(std::path::PathBuf::from("/tmp/shot.png"));
        let mut events = Vec::new();
        dispatch(
            &Request::UploadLast {
                backend: "0x0".into(),
            },
            &spawner,
            &state,
            &mut |ev| events.push(ev),
        )
        .unwrap();

        assert_eq!(
            spawner.snapshot(),
            vec![vec![
                "upload".to_string(),
                "/tmp/shot.png".into(),
                "--backend".into(),
                "0x0".into()
            ]],
            "non-default backend must be passed as --backend"
        );
        assert_eq!(
            events,
            vec![Event::UploadStarted {
                backend: "0x0".into()
            }]
        );
    }

    #[test]
    fn upload_last_without_save_emits_error_and_does_not_spawn() {
        let spawner = RecordingSpawner::new();
        let state = DispatchState::new();
        let mut events = Vec::new();
        let outcome = dispatch(
            &Request::UploadLast {
                backend: "imgur".into(),
            },
            &spawner,
            &state,
            &mut |ev| events.push(ev),
        )
        .unwrap();

        assert_eq!(outcome, DispatchOutcome::Continue);
        assert!(
            spawner.snapshot().is_empty(),
            "UploadLast with no prior save must not spawn"
        );
        assert_eq!(events.len(), 1, "exactly one event");
        match &events[0] {
            Event::Error { code, .. } => assert_eq!(code, "no-last-capture"),
            other => panic!("expected Error event, got {other:?}"),
        }
    }

    #[test]
    fn upload_last_spawn_failure_propagates_after_upload_started() {
        let state = DispatchState::new();
        state.last_saved.set(std::path::PathBuf::from("/tmp/x.png"));
        let mut events = Vec::new();
        let result = dispatch(
            &Request::UploadLast {
                backend: "imgur".into(),
            },
            &FailSpawner,
            &state,
            &mut |ev| events.push(ev),
        );

        assert!(result.is_err(), "spawn failure must propagate");
        assert_eq!(
            events,
            vec![Event::UploadStarted {
                backend: "imgur".into()
            }],
            "UploadStarted emitted on receipt before spawn; no further event on failure"
        );
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
            (
                Request::ReportUploaded {
                    url: "https://x".into(),
                    delete_handle: None,
                },
                "report-uploaded",
            ),
            (Request::BindShortcuts, "bind-shortcuts"),
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
