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

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use color_eyre::eyre;
use flintshot_core::ipc::{Event, Request};

use crate::ipc_server::LastSaved;
use crate::sessions::{CaptureSessions, SupersedeAction};

/// What the daemon should do after [`dispatch`] handles a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchOutcome {
    /// Keep running — the request was serviced (or reported unsupported).
    Continue,
    /// Shut the daemon down — [`Request::Quit`] (used by `--replace`,
    /// Task 006).
    Shutdown,
}

/// Daemon-wide shared state that [`dispatch`] reads or mutates beyond the
/// spawner: the last-saved capture path (set by [`Request::ReportSaved`],
/// read by [`Request::OpenLast`]).
///
/// Cloning shares the underlying state (the field is `Arc`-backed),
/// so every entry point observes the same last-saved path.
#[derive(Clone)]
pub struct DispatchState {
    /// Last worker-reported save path.
    pub(crate) last_saved: LastSaved,
    /// Daemon-side `[capture] backend` view, threaded into
    /// daemon-spawned capture argvs (elided when `"auto"` — the
    /// worker then re-resolves from its own config load). Set once at
    /// daemon start; `Arc` so every clone shares it.
    pub(crate) capture_backend: Arc<str>,
}

impl Default for DispatchState {
    fn default() -> Self {
        Self {
            last_saved: LastSaved::new(),
            capture_backend: Arc::from(DEFAULT_CAPTURE_BACKEND),
        }
    }
}

impl DispatchState {
    /// Fresh state: no last save recorded, default backend.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach the daemon's resolved `[capture] backend` (from
    /// [`flintshot_core::Config`]); capture argvs pass it as
    /// `--backend` unless it is the built-in default.
    #[must_use]
    pub fn with_capture_backend(mut self, backend: &str) -> Self {
        self.capture_backend = Arc::from(backend);
        self
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
/// Each worker gets a lightweight "reaper" thread that polls
/// [`std::process::Child::try_wait`] (short lock holds — the kill
/// handle stays reachable, see `sessions.rs`) so the worker is reaped
/// when its editor closes rather than lingering as a zombie while the
/// daemon runs (captures are infrequent, so a thread-per-worker is
/// cheap).
///
/// With a [`crate::sessions::CaptureSessions`] attached
/// ([`Self::with_bus_and_sessions`]), the spawn also **registers**
/// capture workers in the registry (kill handle), and the reaper
/// deregisters on exit — process death stays the crash-safe session
/// truth (ADR-0046). With a [`SubscriberBus`] attached, the reaper
/// broadcasts [`Event::SessionEnded`] for capture workers.
#[derive(Default, Clone)]
pub struct CommandSpawner {
    /// Bus the reaper broadcasts `SessionEnded` on. `None` in tests
    /// and no-bus call sites (plain `Default`).
    bus: Option<crate::ipc_server::SubscriberBus>,
    /// Registry capture workers register in. `None` keeps the
    /// old bus-only behavior (tests, pre-wiring call sites).
    sessions: Option<Arc<CaptureSessions>>,
}

impl CommandSpawner {
    /// Attach the subscriber bus and the session registry the
    /// daemon's supersede path drives.
    #[must_use]
    pub fn with_bus_and_sessions(
        bus: crate::ipc_server::SubscriberBus,
        sessions: Arc<CaptureSessions>,
    ) -> Self {
        Self {
            bus: Some(bus),
            sessions: Some(sessions),
        }
    }
}

/// Reaper poll cadence: exits are observed within one tick — fast
/// enough to land inside the supersede graces, idle enough to be
/// noise while an editor window stays open for minutes.
const REAP_POLL: Duration = Duration::from_millis(50);

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
        let child = cmd
            .spawn()
            .map_err(|e| eyre::eyre!("spawning worker {exe:?} {argv:?}: {e}"))?;

        // Decided before the argv slice's owner moves on.
        let session_ended = is_capture_argv(argv);
        let pid = child.id();
        let handle = Arc::new(Mutex::new(Some(child)));
        if let Some(sessions) = &self.sessions {
            if is_capture_argv(argv) {
                sessions.register_capture(pid, Arc::clone(&handle));
            } else if is_app_argv(argv) {
                sessions.register_app(pid, Arc::clone(&handle));
            }
        }
        let bus = self.bus.clone();
        let sessions = self.sessions.clone();

        // Reap when the worker exits. Dropping the JoinHandle detaches
        // the reaper thread itself (it ends as soon as the poll
        // observes the exit).
        let _reaper = std::thread::Builder::new()
            .name("flintshot-worker-reaper".into())
            .spawn(move || {
                loop {
                    let observed = {
                        // A poisoned handle means the session state is
                        // unrecoverable; stop reaping (the registry
                        // stays until its own bounds clear it).
                        let Ok(mut guard) = handle.lock() else {
                            break;
                        };
                        match guard.as_mut() {
                            Some(child) => child.try_wait().map_or(true, |s| s.is_some()),
                            None => true,
                        }
                    };
                    if observed {
                        break;
                    }
                    std::thread::sleep(REAP_POLL);
                }
                if session_ended {
                    if let Some(bus) = bus {
                        bus.broadcast(Event::SessionEnded);
                    }
                }
                if let Some(sessions) = sessions {
                    sessions.exited(pid);
                }
            })
            .map_err(|e| eyre::eyre!("spawning worker reaper thread: {e}"))?;
        Ok(())
    }
}

/// Does this worker argv run the app window (tracked so the pre-grab
/// handshake knows an app owes a hide-ack)?
fn is_app_argv(argv: &[&str]) -> bool {
    matches!(argv.first(), Some(&"app"))
}

/// Does this worker argv run a capture (and so owe a
/// [`Event::SessionEnded`] when it exits)? The argv-side twin of
/// [`spawns_capture_worker`]'s request-side check — keep both in
/// sync when a capture mode is added.
fn is_capture_argv(argv: &[&str]) -> bool {
    matches!(argv.first(), Some(&"region" | &"fullscreen"))
}

/// Dispatch a [`Request`]: spawn the right worker (if any), emit the
/// resulting [`Event`]s to `sink`, and report whether the daemon should
/// keep running or shut down.
///
/// `CaptureStarted` is emitted the moment a worker is launched; later
/// events (`EditorOpened`/`Saved`) need the worker to report back, which
/// is wired in Task 006. `Quit` requests shutdown (the caller performs
/// it). `OpenLast` reads the last-saved path from `state`
/// (Task 4); with no prior save it emits
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
            let argv = region_argv(*monitor, *delay_ms, &state.capture_backend);
            sink(Event::CaptureStarted);
            spawn_argv(spawner, &argv)?;
            // No EditorOpened here: spawn does not imply a window —
            // the worker reports one per actual window via
            // `ReportEditorWindowOpened` (smart-window-hiding).
            Ok(DispatchOutcome::Continue)
        }
        Request::CaptureFullscreen { monitor } => {
            let argv = fullscreen_argv(*monitor, &state.capture_backend);
            sink(Event::CaptureStarted);
            spawn_argv(spawner, &argv)?;
            Ok(DispatchOutcome::Continue)
        }
        Request::OpenLast => {
            if let Some(path) = state.last_saved.get() {
                let argv = open_argv(&path);
                spawn_argv(spawner, &argv)?;
                // The open worker reports its own EditorOpened.
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
        // The app-window opener (app-window effort). Broadcast-first
        // so an already-running window (which holds the app lock)
        // shows and focuses itself; the spawned worker then exits
        // promptly on that held lock. With no window running, the
        // spawn is what creates one (and the broadcast lands on
        // nobody — harmless).
        Request::OpenApp => {
            sink(Event::AppFocus);
            let argv = app_argv();
            spawn_argv(spawner, &argv)?;
            Ok(DispatchOutcome::Continue)
        }
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

/// Built-in default capture backend id. When the daemon-side config
/// view equals this, `--backend` is omitted from the capture worker
/// argv — the worker resolves the same value from its own config.
/// Tracks [`flintshot_core::CaptureConfig`]'s default (`"auto"`); kept
/// in sync by the `backend = "auto"` round-trip tests in core's
/// `config.rs`.
const DEFAULT_CAPTURE_BACKEND: &str = "auto";

/// Build the `flintshot region [--monitor N] [--delay-ms M]
/// [--backend BACKEND] --standalone` argv.
///
/// The trailing `--standalone` breaks the delegation loop: without
/// it, the daemon-spawned worker would re-delegate back to the
/// daemon, which spawns the next worker (~60–100/s observed; see
/// `TEST_MATRIX` "L5 pass findings" 1). `delegate_to_daemon`
/// short-circuits on the flag.
///
/// `--backend` mirrors [`upload_argv`]'s elision rule: passed only
/// when it differs from the built-in default.
fn region_argv(monitor: Option<u32>, delay_ms: u32, backend: &str) -> Vec<String> {
    let mut argv = vec!["region".to_string()];
    if let Some(m) = monitor {
        argv.push("--monitor".into());
        argv.push(m.to_string());
    }
    if delay_ms > 0 {
        argv.push("--delay-ms".into());
        argv.push(delay_ms.to_string());
    }
    push_backend_flag(&mut argv, backend);
    argv.push("--standalone".into());
    argv
}

/// Build the `flintshot fullscreen [--monitor N] [--backend BACKEND]
/// --standalone` argv. The `--standalone` guard mirrors
/// [`region_argv`]'s — see its doc comment for the fork-loop
/// rationale.
fn fullscreen_argv(monitor: Option<u32>, backend: &str) -> Vec<String> {
    let mut argv = vec!["fullscreen".to_string()];
    if let Some(m) = monitor {
        argv.push("--monitor".into());
        argv.push(m.to_string());
    }
    push_backend_flag(&mut argv, backend);
    argv.push("--standalone".into());
    argv
}

/// Append `--backend <id>` unless it is the built-in default (the
/// worker resolves the default from its own config load).
fn push_backend_flag(argv: &mut Vec<String>, backend: &str) {
    if backend != DEFAULT_CAPTURE_BACKEND {
        argv.push("--backend".into());
        argv.push(backend.to_string());
    }
}

/// Build the `flintshot app` argv for [`Request::OpenApp`]. No
/// `--standalone` flavor exists here: the worker takes the app lock
/// and exits fast when a window is already running (the broadcast
/// focused it); only a lock-free spawn becomes the window.
fn app_argv() -> Vec<String> {
    vec!["app".to_string()]
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
        Request::UploadLast { .. } => "upload-last",
        Request::BindShortcuts => "bind-shortcuts",
        Request::HideTrayForCapture => "hide-tray-for-capture",
        Request::ReportCaptured => "report-captured",
        Request::OpenApp => "open-app",
        Request::PrepareCapture => "prepare-capture",
        Request::ReportAppHidden => "report-app-hidden",
        Request::ReportEditorWindowOpened => "report-editor-window-opened",
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

/// Supersede-wait poll cadence: the machine owns the deadlines; this
/// is only how often a `Wait` re-checks. An exit wakes the wait
/// early (registry condvar), so this is the straggler-path cadence.
const SUPERSEDE_POLL: Duration = Duration::from_millis(25);

/// The daemon-wide capture dispatch context: the session registry
/// every capture worker registers in, plus the mutex that makes
/// **supersede-then-spawn** one atomic unit across every entry point
/// (IPC server, X11 hotkeys, portal hotkeys). Cloneable and
/// thread-safe; designed alongside `sessions.rs`.
#[derive(Clone, Default)]
pub struct CaptureDispatch {
    sessions: Arc<CaptureSessions>,
    serialize: Arc<Mutex<()>>,
}

impl CaptureDispatch {
    /// Production graces (see the `sessions.rs` consts).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Testable seam: explicit supersede graces (grace, kill wait).
    #[cfg(test)]
    #[must_use]
    pub fn with_graces(grace: Duration, kill_wait: Duration) -> Self {
        Self {
            sessions: Arc::new(CaptureSessions::with_graces(grace, kill_wait)),
            serialize: Arc::new(Mutex::new(())),
        }
    }

    /// The shared session registry (spawner wiring + assertions).
    #[must_use]
    pub fn sessions(&self) -> Arc<CaptureSessions> {
        Arc::clone(&self.sessions)
    }

    /// **Supersede-then-spawn**, serialized: broadcast
    /// [`Event::CaptureSuperseded`], wait (bounded) for the old
    /// session's graceful exit, hard-kill stragglers, wait (bounded)
    /// again — then run the normal dispatch arm that spawns the new
    /// worker. The mutex makes two simultaneous triggers line up;
    /// latest wins once serialized, no queue (Replace policy).
    ///
    /// Blocking (worst case grace + kill wait, ~2 s). Async entry
    /// points must use [`dispatch_capture`], which parks this on the
    /// blocking pool; sync loop sites may call it directly.
    ///
    /// # Errors
    /// Propagates a [`Spawner::spawn`] failure from the dispatch arm.
    pub fn dispatch_capture_blocking(
        &self,
        request: &Request,
        spawner: &dyn Spawner,
        state: &DispatchState,
        sink: &mut impl FnMut(Event),
    ) -> eyre::Result<DispatchOutcome> {
        let _serial = self
            .serialize
            .lock()
            .expect("capture dispatch mutex poisoned");
        loop {
            match self.sessions.supersede_step(Instant::now()) {
                SupersedeAction::Proceed => break,
                SupersedeAction::Broadcast => {
                    tracing::info!("superseding the live capture session");
                    sink(Event::CaptureSuperseded);
                }
                SupersedeAction::Kill => {
                    tracing::warn!("supersede grace expired with workers live; hard-killing them");
                }
                // Block on the registry condvar for a tick: a graceful
                // exit wakes this immediately, the machine's own
                // deadlines bound the straggler path.
                SupersedeAction::Wait => {
                    let _ = self
                        .sessions
                        .wait_no_captures(Instant::now() + SUPERSEDE_POLL);
                }
            }
        }
        dispatch(request, spawner, state, sink)
    }
}

/// Async wrapper around [`CaptureDispatch::dispatch_capture_blocking`]
/// for tokio entry points (IPC server, portal hotkeys): the bounded
/// supersede waits run on the blocking pool so a tokio worker thread
/// never blocks. Broadcasts go through the daemon's shared bus.
///
/// # Errors
/// Propagates a dispatch/spawn failure from the blocking path.
pub async fn dispatch_capture(
    ctx: CaptureDispatch,
    request: Request,
    state: DispatchState,
    bus: crate::ipc_server::SubscriberBus,
) -> eyre::Result<DispatchOutcome> {
    let sessions = ctx.sessions();
    tokio::task::spawn_blocking(move || {
        let spawner = CommandSpawner::with_bus_and_sessions(bus.clone(), sessions);
        ctx.dispatch_capture_blocking(&request, &spawner, &state, &mut |ev| {
            bus.broadcast(ev);
        })
    })
    .await
    .map_err(|e| eyre::eyre!("capture dispatch task: {e}"))?
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
            vec![Event::CaptureStarted],
            "CaptureStarted on receipt; EditorOpened is now the worker's own truthful report"
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
        assert_eq!(events, vec![Event::CaptureStarted]);
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

    #[test]
    fn capture_argv_elides_backend_at_default() {
        // The default DispatchState carries the built-in default
        // backend ("auto"): the flag is omitted and the worker
        // re-resolves from its own config load.
        let spawner = RecordingSpawner::new();
        dispatch(
            &Request::CaptureRegion {
                monitor: None,
                delay_ms: 0,
            },
            &spawner,
            &DispatchState::new().with_capture_backend("auto"),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(
            spawner.snapshot(),
            vec![vec!["region".to_string(), "--standalone".into()]],
            "default backend must not reach the argv"
        );
    }

    #[test]
    fn capture_argv_passes_explicit_backend() {
        for request in [
            Request::CaptureRegion {
                monitor: None,
                delay_ms: 0,
            },
            Request::CaptureFullscreen { monitor: None },
        ] {
            let spawner = RecordingSpawner::new();
            dispatch(
                &request,
                &spawner,
                &DispatchState::new().with_capture_backend("portal"),
                &mut |_| {},
            )
            .unwrap();
            let calls = spawner.snapshot();
            assert_eq!(calls.len(), 1, "one worker for {request:?}");
            let argv = &calls[0];
            let pos = argv
                .iter()
                .position(|a| a == "--backend")
                .unwrap_or_else(|| panic!("--backend missing from {argv:?}"));
            assert_eq!(
                argv.get(pos + 1).map(String::as_str),
                Some("portal"),
                "backend id must follow the flag: {argv:?}"
            );
            assert!(
                argv.contains(&"--standalone".to_string()),
                "--standalone still present: {argv:?}"
            );
        }
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
    fn open_app_broadcasts_focus_and_spawns_app_worker() {
        let spawner = RecordingSpawner::new();
        let mut events = Vec::new();
        let outcome = dispatch(
            &Request::OpenApp,
            &spawner,
            &DispatchState::new(),
            &mut |ev| events.push(ev),
        )
        .unwrap();
        assert_eq!(outcome, DispatchOutcome::Continue);
        assert_eq!(
            events,
            vec![Event::AppFocus],
            "the broadcast is what focuses an existing window"
        );
        assert_eq!(spawner.snapshot(), vec![vec!["app".to_string()]]);
    }

    #[test]
    fn capture_argv_classification_matches_capture_requests() {
        // The argv-side twin of `spawns_capture_worker`: every argv
        // the request-side flags as a capture must classify true,
        // and every non-capture worker argv must not.
        let captures = [
            vec!["region"],
            vec!["region", "--standalone"],
            vec!["fullscreen", "--monitor", "1", "--standalone"],
        ];
        for argv in captures {
            assert!(is_capture_argv(&argv), "{argv:?} is a capture worker");
        }
        let others = [
            vec!["app"],
            vec!["open", "/tmp/x.png"],
            vec!["upload", "/tmp/x.png"],
            vec![],
        ];
        for argv in others {
            assert!(!is_capture_argv(&argv), "{argv:?} is not a capture worker");
        }
    }

    /// The reaper's `SessionEnded` broadcast, exercised with a real
    /// (short-lived) child. The spawned child is this test binary
    /// run with `region --standalone` as libtest args — libtest
    /// exits promptly whatever it makes of them; the reaper only
    /// cares that the process ends.
    #[test]
    fn reaper_broadcasts_session_ended_for_capture_workers() {
        let bus = crate::ipc_server::SubscriberBus::new();
        let mut rx = bus.subscribe();
        let spawner = CommandSpawner::with_bus_and_sessions(bus, Arc::new(CaptureSessions::new()));

        spawner
            .spawn(&["region", "--standalone"])
            .expect("spawning the stand-in worker");

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime");
        let received = runtime.block_on(async {
            tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv()).await
        });
        assert!(
            matches!(received, Ok(Ok(Event::SessionEnded))),
            "expected SessionEnded, got {received:?}"
        );
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
        // OpenLast (Task 4) and UploadLast (Task 007) are now
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
        // CaptureStarted was emitted "on receipt" (before spawn);
        // EditorOpened is the worker's own report, so it never comes
        // from dispatch at all.
        assert_eq!(events, vec![Event::CaptureStarted]);
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
    fn open_last_with_saved_path_spawns_open_worker() {
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
        assert!(
            events.is_empty(),
            "the open worker reports its own EditorOpened once a window opens"
        );
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
            (Request::HideTrayForCapture, "hide-tray-for-capture"),
            (Request::ReportCaptured, "report-captured"),
            (Request::OpenApp, "open-app"),
            (Request::PrepareCapture, "prepare-capture"),
            (Request::ReportAppHidden, "report-app-hidden"),
            (
                Request::ReportEditorWindowOpened,
                "report-editor-window-opened",
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

    // ── supersede-before-spawn (smart-window-hiding, Task 003) ──────

    use std::time::{Duration, Instant};

    fn fake_session_handle() -> Arc<Mutex<Option<std::process::Child>>> {
        Arc::new(Mutex::new(None))
    }

    #[test]
    fn dispatch_capture_supersedes_a_straggler_before_spawning() {
        // Tiny graces: the fake session never exits, so the supersede
        // rides the grace → kill → kill-wait path (the kill is a no-op
        // on the fake handle) and still spawns.
        let ctx =
            CaptureDispatch::with_graces(Duration::from_millis(30), Duration::from_millis(10));
        ctx.sessions().register_capture(9001, fake_session_handle());
        let spawner = RecordingSpawner::new();
        let mut events = Vec::new();

        let outcome = ctx
            .dispatch_capture_blocking(
                &Request::CaptureRegion {
                    monitor: None,
                    delay_ms: 0,
                },
                &spawner,
                &DispatchState::new(),
                &mut |ev| events.push(ev),
            )
            .unwrap();

        assert_eq!(outcome, DispatchOutcome::Continue);
        assert_eq!(spawner.snapshot().len(), 1, "the new worker spawned");
        let superseded_at = events
            .iter()
            .position(|e| matches!(e, Event::CaptureSuperseded))
            .expect("CaptureSuperseded broadcast");
        let started_at = events
            .iter()
            .position(|e| matches!(e, Event::CaptureStarted))
            .expect("CaptureStarted emitted");
        assert!(
            superseded_at < started_at,
            "supersede must precede the new spawn: {events:?}"
        );
        // The straggler never reported an exit, so its entry lingers
        // until a reaper observes it (process-exit truth); the
        // RecordingSpawner registers nothing, hence the count stays.
        assert_eq!(ctx.sessions().live_captures(), 1);
    }

    #[test]
    fn dispatch_capture_released_early_by_a_graceful_exit() {
        // A 500 ms grace and an exit arriving ~30 ms in: the supersede
        // must proceed on the exit, not on the bound.
        let ctx =
            CaptureDispatch::with_graces(Duration::from_millis(500), Duration::from_millis(50));
        ctx.sessions().register_capture(9002, fake_session_handle());
        let sessions = ctx.sessions();
        let reaper = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            sessions.exited(9002);
        });

        let started = Instant::now();
        let spawner = RecordingSpawner::new();
        let mut events = Vec::new();
        ctx.dispatch_capture_blocking(
            &Request::CaptureFullscreen { monitor: None },
            &spawner,
            &DispatchState::new(),
            &mut |ev| events.push(ev),
        )
        .unwrap();
        reaper.join().expect("reaper thread");

        assert!(
            started.elapsed() < Duration::from_millis(300),
            "the graceful exit released the supersede, not the grace"
        );
        assert!(
            events.iter().any(|e| matches!(e, Event::CaptureSuperseded)),
            "the supersede was still broadcast first"
        );
        assert_eq!(spawner.snapshot().len(), 1);
        assert_eq!(ctx.sessions().live_captures(), 0);
    }

    #[test]
    fn dispatch_capture_double_trigger_latest_wins_serialized() {
        // Two sequential triggers with a straggler session: each is
        // supersede → spawn as one serialized unit; no spawn happens
        // before its own supersede.
        let ctx =
            CaptureDispatch::with_graces(Duration::from_millis(20), Duration::from_millis(10));
        ctx.sessions().register_capture(9003, fake_session_handle());
        let spawner = RecordingSpawner::new();
        let mut events = Vec::new();

        for _ in 0..2 {
            ctx.dispatch_capture_blocking(
                &Request::CaptureRegion {
                    monitor: None,
                    delay_ms: 0,
                },
                &spawner,
                &DispatchState::new(),
                &mut |ev| events.push(ev),
            )
            .unwrap();
        }

        assert_eq!(spawner.snapshot().len(), 2, "both triggers spawned");
        let mut superseded_seen = 0;
        let mut started_seen = 0;
        for ev in &events {
            match ev {
                Event::CaptureSuperseded => superseded_seen += 1,
                Event::CaptureStarted => started_seen += 1,
                _ => {}
            }
        }
        assert_eq!(
            superseded_seen, 2,
            "each trigger superseded the lingering session first"
        );
        assert_eq!(started_seen, 2);
        // Strict alternation: Superseded, Started, Superseded, Started.
        let kinds: Vec<&Event> = events
            .iter()
            .filter(|e| matches!(e, Event::CaptureSuperseded) || matches!(e, Event::CaptureStarted))
            .collect();
        assert!(
            matches!(
                kinds.as_slice(),
                [
                    Event::CaptureSuperseded,
                    Event::CaptureStarted,
                    Event::CaptureSuperseded,
                    Event::CaptureStarted
                ]
            ),
            "serialized units, never spawn-before-supersede: {kinds:?}"
        );
    }

    #[test]
    fn non_capture_dispatch_leaves_sessions_untouched() {
        let ctx = CaptureDispatch::new();
        ctx.sessions().register_capture(9004, fake_session_handle());
        let state = DispatchState::new();
        state.last_saved.set(std::path::PathBuf::from("/tmp/x.png"));
        let spawner = RecordingSpawner::new();
        let mut events = Vec::new();

        dispatch(&Request::OpenLast, &spawner, &state, &mut |ev| {
            events.push(ev);
        })
        .unwrap();

        assert!(
            !events.iter().any(|e| matches!(e, Event::CaptureSuperseded)),
            "OpenLast must not supersede anything"
        );
        assert_eq!(
            ctx.sessions().live_captures(),
            1,
            "the live capture session survives a non-capture dispatch"
        );
    }

    #[test]
    fn dispatch_capture_kills_a_real_straggler_and_proceeds() {
        // Kill -9 resilience with a real process: a `sleep 30` stands
        // in for a hung worker. The supersede hard-kills it after the
        // (tiny) grace; the poll-based reap confirms the exit.
        let ctx =
            CaptureDispatch::with_graces(Duration::from_millis(100), Duration::from_millis(100));
        let child = std::process::Command::new("sleep")
            .arg("30")
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("spawning sleep");
        let pid = child.id();
        ctx.sessions()
            .register_capture(pid, Arc::new(Mutex::new(Some(child))));

        let spawner = RecordingSpawner::new();
        let started = Instant::now();
        ctx.dispatch_capture_blocking(
            &Request::CaptureFullscreen { monitor: None },
            &spawner,
            &DispatchState::new(),
            &mut |_| {},
        )
        .unwrap();

        assert_eq!(spawner.snapshot().len(), 1, "the new capture proceeded");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "bounded: grace + kill wait only"
        );
        assert!(
            ctx.sessions().poll_exit(pid).is_some(),
            "the straggler was killed and reaped"
        );
        assert_eq!(ctx.sessions().live_captures(), 0);
    }

    #[test]
    fn command_spawner_registers_and_reaps_capture_sessions() {
        // A real (promptly-exiting) child: the spawn registers it in
        // the shared registry, and the poll reaper deregisters on
        // exit — SessionEnded broadcast behavior is covered by the
        // existing reaper test.
        let bus = crate::ipc_server::SubscriberBus::new();
        let sessions = Arc::new(CaptureSessions::with_graces(
            Duration::from_millis(100),
            Duration::from_millis(100),
        ));
        let spawner = CommandSpawner::with_bus_and_sessions(bus, Arc::clone(&sessions));

        spawner
            .spawn(&["region", "--standalone"])
            .expect("spawning the stand-in worker");

        let deadline = Instant::now() + Duration::from_secs(5);
        while sessions.live_captures() == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(sessions.live_captures(), 1, "the child was registered");

        while sessions.live_captures() != 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            sessions.live_captures(),
            0,
            "the poll reaper deregistered the exited child"
        );
    }

    #[test]
    fn is_app_argv_classifies_only_the_app_worker() {
        assert!(is_app_argv(&["app"]));
        assert!(!is_app_argv(&["region", "--standalone"]));
        assert!(!is_app_argv(&["upload", "/tmp/x.png"]));
    }
}
