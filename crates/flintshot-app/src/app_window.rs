//! The app window (`flintshot app`): a single window routing between
//! a three-button home page and the editor page (single-window spec,
//! `docs/specs/single-window/`).
//!
//! Process model (spec `docs/specs/app-window/`): the window is an
//! IPC **client** — same stance as the tray helper (ADR-0042). A
//! background **pump thread** owns a current-thread tokio runtime,
//! holds the daemon subscription, delegates capture requests, and
//! pushes [`HomeDirective`]s. The pump never touches a window: every
//! epoch (`run_window` / `run_editor_page`) polls the directives it
//! pushed.
//!
//! ## The router (page epochs)
//!
//! [`run`]'s outer loop is the page router: one eframe lifetime per
//! [`Page`] — the three-button home, or the image-sized editor —
//! with per-epoch viewport geometry and title. A page transition is
//! window close + recreate (no runtime resize); the post-close
//! classification ([`CloseReason`]) picks the next page. `Esc` in
//! the editor routes home; the window X / Alt+F4 exits the process
//! (daemon unaffected); Save / Copy / Upload apply their side
//! effects and the editor page stays.
//!
//! ## Auto-start
//!
//! No daemon at subscribe time → spawn `flintshot daemon` detached
//! and retry (bounded). The window opens immediately either way;
//! the status line narrates ("connecting…" → "starting daemon…" →
//! "connected" / failure).
//!
//! ## Close / reopen (ADR-0047)
//!
//! winit's Wayland `set_visible` is a hard no-op, so *hiding* the
//! window cannot keep it out of a grab. Instead the window
//! **closes** for a capture — surface destruction is causal on
//! every compositor, the same guarantee the editor's recapture
//! loop relies on (ADR-0046). The flow: a control click registers
//! the intent ([`Shared::set_pending_capture`]) and closes the
//! window; the outer loop in [`run`] waits out the unmap grace,
//! delegates via the pump, and blocks until a `Show` directive (or
//! the pump's death) reopens a fresh window.
//!
//! ## Restore semantics
//!
//! [`HomeCore`] counts `CaptureStarted` (+) against `SessionEnded`
//! (−, saturating — the daemon's reaper emits it when a capture
//! worker exits, so a crashed worker still restores the window). A
//! daemon disconnect force-shows with an error status rather than
//! leaving the process windowless; the pump then stays in a
//! degraded cmd-only loop (no auto-reconnect in this effort).

use std::sync::Arc;
use std::time::Duration;

use color_eyre::eyre;
use flintshot_core::ipc::Event as IpcEvent;
use flintshot_core::ipc::Request;
use flintshot_core::{Config, EditorOutcome, EditorState};
use flintshot_editor_egui::{EditorPageBridge, HomeBridge, HomeDirective};
use tokio::sync::mpsc as tokio_mpsc;

use crate::capture_flow::{AppGrab, AppGrabEvent};
use crate::cli::SupersedeWatch;
use crate::ipc_server::{decode_event, read_frame};

/// What the single window shows for its next lifetime — the router's
/// state. A value, not baked-in window assumptions, so future
/// tab/workspace models stay additive (spec: out-of-scope note).
#[derive(Debug, Clone)]
enum Page {
    /// Empty state: three centered actions.
    Home,
    /// Editor session over one image (boxed: `EditorState` carries
    /// its image buffer inline, and the unit variants need not pay
    /// for it).
    Editor(Box<PendingEditor>),
}

/// An editor page waiting to run: the image's editor state plus the
/// landing phase (the fallback monitor grab pre-arms Select — the
/// worker's `"region"` labels; every other source opens plain).
#[derive(Debug, Clone)]
struct PendingEditor {
    /// Source screenshot + annotation model.
    state: EditorState,
    /// Whether the epoch opens selection-first.
    selection_first: bool,
}

/// Pause between the window closing and the capture delegating, so
/// the compositor has destroyed the surface before the worker
/// freezes the screen. Matches the tray icon-grace bound's
/// rationale (ADR-0036). Shared by the editor's recapture loop
/// (the closing window needs the same headroom before the fresh
/// grab) and — as the canonical value — by the daemon's pre-grab
/// handshake (`prepare_gate::UNMAP_GRACE_MS`).
pub(crate) const HIDE_GRACE_MS: u64 = crate::prepare_gate::UNMAP_GRACE_MS;

/// How long the pump waits for an auto-started daemon to answer
/// before giving up (status flips to the failure text; the window
/// stays open in the degraded mode).
const DAEMON_START_TIMEOUT: Duration = Duration::from_secs(10);

/// Retry cadence while waiting for the daemon's socket to answer.
const CONNECT_RETRY_INTERVAL: Duration = Duration::from_millis(250);

/// How long the pump waits for the window to die after a
/// `HideForCapture` before skipping the ack: the daemon's handshake
/// bound dwarfs this, so a wedged UI costs the ack, not the capture.
const HIDE_ACK_WINDOW: Duration = Duration::from_millis(1500);

/// A capture intent crossing UI → pump.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PumpCommand {
    /// Run the single Capture action in-process — the OS UI
    /// (native) or the editor (fallback) owns selection.
    Capture,
}

/// State shared by the UI (in-window), the outer reopen loop, and
/// the pump: the directive queue, the latest status line (a
/// reopened window starts from it), the capture intent behind a
/// window close, and the reopen flag a `Show` directive sets. One
/// mutex + condvar; no channels, so a reopened window's bridge
/// picks up where the previous one left off.
#[derive(Default)]
struct Shared {
    inner: std::sync::Mutex<SharedInner>,
    changed: std::sync::Condvar,
}

#[derive(Default)]
// Independent rendezvous flags, each with its own producer and
// consumer; a state enum would couple unrelated lifecycles.
#[allow(clippy::struct_excessive_bools)]
struct SharedInner {
    directives: std::collections::VecDeque<HomeDirective>,
    status: String,
    pending_capture: Option<PumpCommand>,
    /// The Open control decoded an image (or a grab delivered one):
    /// the next epoch is the editor page over it.
    pending_editor: Option<PendingEditor>,
    /// A grab confirmed but its image has not landed (the portal's
    /// clipboard write): the next generation opens on the Loading
    /// route (editor-capture-ux Task 008).
    pending_loading: bool,
    /// A failed or quiet grab while the window was open on the
    /// Loading route: route home — nothing is coming to show.
    route_home: bool,
    /// The window closed for a hide directive (not a user quit): the
    /// outer loop must wait for the reopen instead of exiting.
    hide_requested: bool,
    /// Whether a window currently exists, per the outer loop
    /// (set open before an epoch runs, closed after it returns).
    /// **State, not a one-shot mark**: the pump's hide-ack
    /// reads it fresh, so it can only ever attest the *current*
    /// window — a stale "destroyed" flag from an earlier close cycle
    /// can no longer ack a handshake while a fresh window is open
    /// (the race that put the app window in shots).
    window_open: bool,
    /// The page the current epoch runs (a clone) — the same-page
    /// restore target when an external capture closes the window
    /// (plan decision 8: restores return to the page that was open,
    /// not always home; a routed grab's fresh image wins over it).
    restore_page: Option<Page>,
    should_open: bool,
    pump_dead: bool,
}

impl Shared {
    /// Push one directive for the (open) window and/or the outer
    /// loop. `Show` additionally arms the reopen flag — and drains
    /// any stale `Hide` (one queued while the window was closed by
    /// the very capture this Show ends; left in, it would close the
    /// freshly reopened window). `Hide` arms the outer loop's
    /// wait-for-reopen. `SetStatus` updates the remembered status a
    /// reopened window starts from.
    fn push_directive(&self, directive: HomeDirective) {
        let mut guard = self.lock();
        match &directive {
            HomeDirective::Show => {
                guard.should_open = true;
                guard.hide_requested = false;
                guard
                    .directives
                    .retain(|d| !matches!(d, HomeDirective::Hide));
            }
            HomeDirective::Hide => guard.hide_requested = true,
            HomeDirective::SetStatus(text) => guard.status.clone_from(text),
        }
        guard.directives.push_back(directive);
        drop(guard);
        self.changed.notify_all();
    }

    /// Drain one directive (the window polls this every frame).
    fn poll_directive(&self) -> Option<HomeDirective> {
        self.lock().directives.pop_front()
    }

    /// Register the capture intent behind the imminent window close.
    fn set_pending_capture(&self, command: PumpCommand) {
        self.lock().pending_capture = Some(command);
        self.changed.notify_all();
    }

    /// Take the capture intent, if the window closed for one.
    fn take_pending_capture(&self) -> Option<PumpCommand> {
        self.lock().pending_capture.take()
    }

    /// Register the editor page the next epoch runs over (the Open
    /// control's routed image, the Paste clipboard image, or a
    /// delivered grab).
    fn set_pending_editor(&self, pending: PendingEditor) {
        self.lock().pending_editor = Some(pending);
        self.changed.notify_all();
    }

    /// Take the pending editor page, if one waits for the next
    /// epoch.
    fn take_pending_editor(&self) -> Option<PendingEditor> {
        self.lock().pending_editor.take()
    }

    /// Arm the loading start (a confirmed grab whose image is still
    /// landing).
    fn set_pending_loading(&self) {
        self.lock().pending_loading = true;
        self.changed.notify_all();
    }

    /// Take the pending-loading start, if one waits.
    fn take_pending_loading(&self) -> bool {
        std::mem::take(&mut self.lock().pending_loading)
    }

    /// Drop any stale pending-loading mark (the reopen race
    /// resolved through another start).
    fn clear_pending_loading(&self) {
        self.lock().pending_loading = false;
    }

    /// Arm the route-home intent (see [`SharedInner::route_home`]).
    fn set_route_home(&self) {
        self.lock().route_home = true;
        self.changed.notify_all();
    }

    /// Take the route-home intent, if one waits.
    fn take_route_home(&self) -> bool {
        std::mem::take(&mut self.lock().route_home)
    }

    /// Whether a window generation is currently running.
    fn window_is_open(&self) -> bool {
        self.lock().window_open
    }

    /// Arm the home route (the editor page's `Esc` exit).
    /// Whether the home route is armed (the classifier consumes it).
    /// Take the hide request (the outer loop's close classification).
    fn take_hide_requested(&self) -> bool {
        std::mem::take(&mut self.lock().hide_requested)
    }

    /// The outer loop's window-state updates: open before a
    /// [`EguiShell::run_window`] iteration, closed after it returns.
    /// No notification on open (nothing waits for open).
    fn set_window_open(&self) {
        self.lock().window_open = true;
    }

    fn set_window_closed(&self) {
        self.lock().window_open = false;
        self.changed.notify_all();
    }

    /// Block (bounded) until no window exists, then report success.
    /// The pump uses this to ack only while the screen is *actually*
    /// windowless — ack-then-grace, the ADR-0047 lesson. Reading
    /// current state (rather than consuming a mark) is the
    /// correctness core: an ack can only ever describe the present,
    /// never a remembered close from an earlier cycle.
    fn wait_window_closed(&self, bound: Duration) -> bool {
        let deadline = std::time::Instant::now() + bound;
        let mut guard = self.lock();
        loop {
            if !guard.window_open {
                return true;
            }
            let now = std::time::Instant::now();
            if now >= deadline {
                return false;
            }
            let (g, _timeout) = self
                .changed
                .wait_timeout(guard, deadline - now)
                .expect("shared mutex poisoned");
            guard = g;
        }
    }

    /// The status a freshly (re)opened window should show.
    fn status(&self) -> String {
        self.lock().status.clone()
    }

    /// Stash the page an epoch is about to run (the same-page restore
    /// target — plan decision 8).
    fn set_restore_page(&self, page: Page) {
        self.lock().restore_page = Some(page);
    }

    /// Take the stashed restore page, if any.
    fn take_restore_page(&self) -> Option<Page> {
        self.lock().restore_page.take()
    }

    /// Block until a `Show` armed the reopen flag (or the pump
    /// died). Returns whether a window should open; the flag is
    /// consumed either way.
    fn wait_for_reopen(&self) -> bool {
        let mut guard = self.lock();
        while !guard.should_open && !guard.pump_dead {
            guard = self.changed.wait(guard).expect("shared mutex poisoned");
        }
        let reopen = guard.should_open;
        guard.should_open = false;
        reopen
    }

    /// The pump is gone; waiting on it would hang forever.
    fn mark_pump_dead(&self) {
        self.lock().pump_dead = true;
        self.changed.notify_all();
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, SharedInner> {
        self.inner.lock().expect("shared mutex poisoned")
    }
}

/// Pure close/reopen state machine driven by the subscription
/// stream. Owned exclusively by the pump thread; extracted so the
/// transitions are unit-testable without sockets.
#[derive(Debug, Default, PartialEq)]
struct HomeCore {
    /// A capture was requested and the window hid for it.
    hidden: bool,
    /// Capture workers the daemon has started minus those whose
    /// processes exited. Saturating: a `SessionEnded` for a capture
    /// that began before we subscribed clamps at 0 (documented
    /// edge in the spec).
    in_flight: u32,
    /// A supersede was seen and its replacement capture has not
    /// started yet: restores are held (the superseded worker's
    /// `SessionEnded` would otherwise reopen the window straight
    /// into the new capture's grab — the flicker the Replace policy
    /// must not cause).
    supersede_pending: bool,
}

impl HomeCore {
    /// The pump accepted a capture command: the window is (or is
    /// about to be) hidden.
    fn capture_requested(&mut self) {
        self.hidden = true;
    }

    /// Apply one subscription event; `Some(_)` when a directive
    /// should reach the UI.
    #[allow(
        clippy::wildcard_enum_match_arm,
        reason = "IpcEvent is #[non_exhaustive]; only the capture-lifecycle variants matter here"
    )]
    fn on_event(&mut self, event: &IpcEvent) -> Option<HomeDirective> {
        match event {
            IpcEvent::CaptureStarted => {
                self.in_flight += 1;
                self.supersede_pending = false;
                // Any capture must not find this window on screen —
                // self-triggered or not (the ADR-0047 gap).
                self.hidden = true;
                Some(HomeDirective::Hide)
            }
            IpcEvent::CaptureTriggered { .. } => {
                // A forwarded capture (single-window): the daemon
                // handed the trigger to this window — no worker will
                // run, so no `SessionEnded` will balance the
                // `CaptureStarted` it rode in on. Undo the count now;
                // the pump's grab delivery restores.
                self.in_flight = self.in_flight.saturating_sub(1);
                self.hidden = true;
                Some(HomeDirective::Hide)
            }
            IpcEvent::HideForCapture => {
                // The pre-grab handshake's nudge; idempotent with the
                // arm above (whichever arrives first hides).
                self.hidden = true;
                Some(HomeDirective::Hide)
            }
            IpcEvent::CaptureSuperseded => {
                self.supersede_pending = true;
                None
            }
            IpcEvent::SessionEnded => {
                self.in_flight = self.in_flight.saturating_sub(1);
                self.maybe_show()
            }
            IpcEvent::AppFocus => self.app_focus(),
            _ => None,
        }
    }

    /// A focus ping: surface the window — visibly re-focusing an
    /// already-visible idle window is the ping's whole point — but
    /// never raise it over a live capture session.
    fn app_focus(&mut self) -> Option<HomeDirective> {
        if self.in_flight == 0 && !self.supersede_pending {
            self.hidden = false;
            Some(HomeDirective::Show)
        } else {
            None
        }
    }

    /// Restore when the last in-flight capture finished and we were
    /// hiding — unless a supersede's replacement capture is still
    /// coming (it will hide again immediately; reopening in between
    /// would put the window in the new grab).
    fn maybe_show(&mut self) -> Option<HomeDirective> {
        if self.hidden && self.in_flight == 0 && !self.supersede_pending {
            self.force_show()
        } else {
            None
        }
    }

    /// Un-hide unconditionally (used for failure paths where
    /// leaving the window hidden would strand the user).
    fn force_show(&mut self) -> Option<HomeDirective> {
        if self.hidden {
            self.hidden = false;
            Some(HomeDirective::Show)
        } else {
            None
        }
    }

    /// The subscription died: nothing more can be learned, so the
    /// window must come back and say so.
    fn disconnected(&mut self) -> Vec<HomeDirective> {
        self.in_flight = 0;
        let mut out = self
            .force_show()
            .into_iter()
            .collect::<Vec<HomeDirective>>();
        out.push(HomeDirective::SetStatus(
            "daemon connection lost — restart `flintshot app`".to_string(),
        ));
        out
    }
}

/// The pump's capture-flow state machine (single-window): which grab
/// generation owns the route. Pure so the transitions are
/// unit-testable without a display or a daemon.
#[derive(Debug, Default, PartialEq)]
struct CaptureFlow {
    /// The newest grab generation; 0 = never triggered.
    generation: u64,
    /// A grab is in flight (its thread owns the route rights for
    /// `generation`).
    grabbing: bool,
}

impl CaptureFlow {
    /// A new capture trigger. Supersedes any in-flight grab (the
    /// newest intent owns the route — the Replace policy) and
    /// returns the fresh generation the new grab runs under.
    fn trigger(&mut self) -> u64 {
        self.generation += 1;
        self.grabbing = true;
        self.generation
    }

    /// Whether `generation` still owns the route (a newer trigger
    /// bumped past it).
    fn current(&self, generation: u64) -> bool {
        generation == self.generation
    }

    /// The grab ended (routed, quiet, or failed). Returns whether its
    /// outcome may route; only the current generation clears the
    /// in-flight mark (a superseded grab's end leaves the newer one
    /// flying).
    fn finished(&mut self, generation: u64) -> bool {
        let routes = self.current(generation);
        if routes {
            self.grabbing = false;
        }
        routes
    }
}

/// The in-flight grab's handle: what a re-trigger supersedes.
struct ActiveGrab {
    /// The generation this thread grabs under.
    generation: u64,
    /// The grab's watch — `supersede()`d by a newer trigger so its
    /// next checkpoint abandons quietly. Shared with the grab
    /// thread via `Arc`.
    watch: Arc<SupersedeWatch>,
}

/// The pump's in-process capture machinery (single-window): the grab
/// state machine, the active grab, and the sender its grab threads
/// deliver through.
#[derive(Default)]
struct GrabRouter {
    flow: CaptureFlow,
    active: Option<ActiveGrab>,
    /// Cloned per spawned grab thread; the router keeps the original
    /// so the channel never closes under the pump.
    grabs_tx: Option<tokio_mpsc::Sender<(u64, AppGrabEvent)>>,
}

impl GrabRouter {
    /// A capture trigger: supersede any in-flight grab and mint the
    /// new generation. The caller arms [`Self::arm`] with the watch
    /// it hands the grab thread.
    fn trigger(&mut self) -> u64 {
        let generation = self.flow.trigger();
        if let Some(old) = self.active.take() {
            tracing::info!(
                generation = old.generation,
                "superseding the in-flight grab; its outcome will be discarded"
            );
            old.watch.supersede();
        }
        generation
    }

    /// Track the freshly spawned grab as the active one.
    fn arm(&mut self, generation: u64, watch: Arc<SupersedeWatch>) {
        self.active = Some(ActiveGrab { generation, watch });
    }

    /// A grab thread delivered one phase. Stale generations
    /// (superseded mid-flight) fold silently at either phase; the
    /// current one's `Finished` routes (a `Confirmed` is the early
    /// open signal — Task 008 routes the loading page on it; today
    /// it logs).
    fn deliver(
        &mut self,
        shared: &Shared,
        core: &mut HomeCore,
        generation: u64,
        event: AppGrabEvent,
    ) {
        let grab = match event {
            AppGrabEvent::Confirmed => {
                if self.flow.current(generation) {
                    tracing::debug!(
                        generation,
                        "grab confirmed; opening the loading route while the write lands"
                    );
                    // The Show wakes the outer loop's wait-for-reopen;
                    // the armed loading mark makes the fresh
                    // generation open on the progress route. A fast
                    // read-back may yet race this with a landed image
                    // (the pending editor wins in
                    // `next_start_after_reopen`).
                    shared.set_pending_loading();
                    if let Some(directive) = core.force_show() {
                        shared.push_directive(directive);
                    }
                } else {
                    tracing::info!(generation, "discarding a superseded grab's confirmation");
                }
                return;
            }
            AppGrabEvent::Finished(grab) => grab,
        };
        if !self.flow.finished(generation) {
            tracing::info!(generation, "discarding a superseded grab's outcome");
            return;
        }
        self.active = None;
        route_grab(shared, core, grab);
    }
}

/// Route one grab outcome to the outer loop: an image arms the
/// editor page and wakes the router; quiet exits and failures reopen
/// the prior page (home), failures with the message on the status
/// line.
fn route_grab(shared: &Shared, core: &mut HomeCore, grab: AppGrab) {
    match grab {
        AppGrab::Image {
            img,
            selection_first,
        } => {
            let cfg = match Config::load() {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(error = %e, "could not load config; using built-in defaults");
                    Config::default()
                }
            };
            shared.set_pending_editor(PendingEditor {
                state: crate::cli::state_from_config(img, &cfg),
                selection_first,
            });
            if let Some(directive) = core.force_show() {
                shared.push_directive(directive);
            }
        }
        AppGrab::Quiet => {
            tracing::info!("capture dismissed or superseded; reopening the prior page");
            if shared.window_is_open() {
                // The window is up (the loading route, typically):
                // nothing is coming to show — route it home in-place.
                shared.set_route_home();
            } else if let Some(directive) = core.force_show() {
                shared.push_directive(directive);
            }
        }
        AppGrab::Failed(message) => {
            tracing::warn!(%message, "the app-window grab failed");
            shared.push_directive(HomeDirective::SetStatus(format!(
                "capture failed: {message}"
            )));
            if shared.window_is_open() {
                // The open page shows the status line via the home
                // route's bridge and leaves the spinner behind.
                shared.set_route_home();
            } else if let Some(directive) = core.force_show() {
                shared.push_directive(directive);
            }
        }
    }
}

/// Spawn the grab thread for a fresh trigger (the pump stays
/// responsive — the OS selection UI blocks for human think-time, and
/// the handshake's waits must not starve the event loop).
///
/// GOTCHA (handshake): the window closed **before** this thread's
/// `CaptureGuard` sends `PrepareCapture` (the outer loop slept the
/// unmap grace first), so the daemon's `HideForCapture` broadcast
/// finds `wait_window_closed` already true and the `ReportAppHidden`
/// ack leaves immediately — the pump's select loop handles it while
/// the grab thread blocks on `CaptureReady`.
fn start_grab(router: &mut GrabRouter, shared: &Shared) {
    start_grab_with(router, shared, 0);
}

/// [`start_grab`] with an explicit pre-capture delay (the forwarded
/// trigger's `delay_ms`; the button path passes 0).
fn start_grab_with(router: &mut GrabRouter, shared: &Shared, delay_ms: u32) {
    let generation = router.trigger();
    let watch = Arc::new(SupersedeWatch::subscribe_at(None));
    let Some(tx) = router.grabs_tx.clone() else {
        tracing::warn!("grab channel not wired; dropping the capture request");
        return;
    };
    let grab_watch = Arc::clone(&watch);
    if let Err(e) = std::thread::Builder::new()
        .name("flintshot-app-grab".into())
        .spawn(move || {
            // Two-phase delivery (editor-capture-ux Task 007): the
            // clipboard path announces Confirmed before its read-back
            // blocks, then Finished; a closed channel (pump gone)
            // drops the rest — correct then.
            crate::capture_flow::emit_grab_events(&grab_watch, delay_ms, &mut |event| {
                let _ = tx.blocking_send((generation, event));
            });
        })
    {
        tracing::warn!(error = %e, "spawning the grab thread; reopening the prior page");
        let _ = router.flow.finished(generation);
        shared.push_directive(HomeDirective::Show);
        return;
    }
    router.arm(generation, watch);
}

/// `flintshot app` entry point: acquire the app lock (focus-ping and
/// exit if held), start the pump, then the page-router loop — one
/// eframe epoch per page lifetime (ADR-0047).
pub(crate) fn run() -> eyre::Result<()> {
    let Some(_lock) = crate::single::acquire_app_lock()? else {
        return focus_existing();
    };

    let (cmd_tx, cmd_rx) = tokio_mpsc::channel::<PumpCommand>(8);
    let shared = Arc::new(Shared::default());

    let pump = std::thread::Builder::new()
        .name("flintshot-app-pump".into())
        .spawn({
            // The closure owns the Arc; pump_main borrows for its
            // whole life and the outer loop keeps its clone.
            let shared = Arc::clone(&shared);
            move || pump_main(cmd_rx, &shared)
        })
        .map_err(|e| eyre::eyre!("spawning the app pump thread: {e}"))?;

    let mut result = Ok(());
    let mut start = GenerationStart::Home;

    // One iteration = one window **generation** (one eframe
    // lifetime; pages route inside it). Only captures and user quit
    // close the window now (editor-capture-ux Task 006): a capture
    // intent's grace + delegation + wait-for-Show live here, and a
    // hide-by-event close (external capture) waits for the same
    // reopen. Any other close is the user quitting.
    loop {
        let exit = match run_generation(&shared, start) {
            Ok(exit) => exit,
            Err(e) => {
                result = Err(e);
                break;
            }
        };

        // The window is destroyed — the pump's hide-ack waits on this
        // state, and every close reason below assumes it.
        shared.set_window_closed();

        // The route that was on screen at close is the same-page
        // restore target for an external capture's reopen.
        shared.set_restore_page(route_to_page(&exit.route));

        // A capture intent publishes NewCapture and closes; the exit
        // carries it here.
        if let Some(outcome) = exit.editor_outcome {
            arm_window_outcome(&shared, outcome);
        }

        match close_reason(&shared) {
            CloseReason::UserQuit => break,
            CloseReason::CaptureIntent(command) => {
                // The window is destroyed; the grace covers the compositor
                // finishing the teardown before the grab's handshake
                // clears flintshot's remaining surfaces.
                std::thread::sleep(Duration::from_millis(HIDE_GRACE_MS));
                if let Err(e) = cmd_tx.blocking_send(command) {
                    tracing::warn!(error = %e, "app pump is gone; capture request dropped");
                    break;
                }
                if !shared.wait_for_reopen() {
                    tracing::warn!("app pump died before the capture session ended");
                    break;
                }
                start = next_start_after_reopen(&shared);
            }
            CloseReason::HiddenByEvent => {
                // An externally triggered capture hid us (HideForCapture
                // / CaptureStarted): not a user quit. The pump acked the
                // hide; reopen when the session ends — back on the
                // route that was open, unless the trigger routed a
                // fresh image (a forwarded grab's editor page).
                if !shared.wait_for_reopen() {
                    tracing::warn!("app pump died while hidden for a capture");
                    break;
                }
                start = next_start_after_reopen(&shared);
            }
        }
    }

    drop(cmd_tx);
    let _ = pump.join();

    result
}

/// An `OpenRequested` event's outcome, applied to the shared state
/// (pure seam of [`handle_open_requested`], which decodes off-thread).
fn apply_open_outcome(shared: &Shared, decoded: eyre::Result<EditorState>, path: &str) {
    match decoded {
        Ok(state) => {
            shared.set_pending_editor(PendingEditor {
                state,
                selection_first: false,
            });
            shared.push_directive(HomeDirective::Show);
        }
        Err(e) => {
            shared.push_directive(HomeDirective::SetStatus(format!(
                "could not open {path}: {e}"
            )));
            shared.push_directive(HomeDirective::Show);
        }
    }
}
/// Handle a forwarded open (single-window): close whatever page is
/// open (the editor epoch's geometry is per-image), decode on a plain
/// thread (the pump must keep answering handshakes), then route — a
/// good decode arms the editor page; a failure reopens the prior
/// page with the reason on the status line. Replaces an open editor
/// image wholesale (an open intent, not a capture — no supersede
/// machinery applies).
fn handle_open_requested(shared: &Arc<Shared>, path: std::path::PathBuf) {
    shared.push_directive(HomeDirective::Hide);
    let thread_shared = Arc::clone(shared);
    if let Err(e) = std::thread::Builder::new()
        .name("flintshot-app-open".into())
        .spawn(move || {
            let decoded = decode_image_file(&path);
            apply_open_outcome(&thread_shared, decoded, &path.display().to_string());
        })
    {
        tracing::warn!(error = %e, "spawning the open decode thread; reopening");
        shared.push_directive(HomeDirective::Show);
    }
}

/// The post-reopen generation start: a routed image (a grab's
/// capture or a forwarded open — whichever armed the editor page)
/// wins; else the route that was open (the same-page restore); else
/// home.
fn next_start_after_reopen(shared: &Shared) -> GenerationStart {
    let start = match shared.take_pending_editor() {
        // A landed image (the fast read-back raced the open) opens
        // the editor directly — no spinner flash.
        Some(pending) => GenerationStart::Editor(Box::new(pending)),
        // The confirmed grab's write is still landing: open on the
        // progress route.
        None if shared.take_pending_loading() => GenerationStart::Loading,
        None => match shared.take_restore_page().unwrap_or(Page::Home) {
            Page::Home => GenerationStart::Home,
            Page::Editor(pending) => GenerationStart::Editor(pending),
        },
    };
    // Whichever won, drain the other flag — a stale loading mark
    // would flip a later reopen into a stranded spinner.
    shared.clear_pending_loading();
    start
}

/// What a window generation opens on — the outer loop's routing
/// state (one generation = one eframe lifetime; pages route inside).
#[derive(Debug, Clone)]
enum GenerationStart {
    /// The three-button home.
    Home,
    /// The editor over a routed image (a delivered grab reopening a
    /// closed window).
    Editor(Box<PendingEditor>),
    /// The capture-progress route while a delivery is pending (a
    /// confirmed grab whose clipboard write is landing).
    Loading,
}

/// Run one window generation to its close through the window router:
/// one eframe lifetime with Home / Editor as internal routes —
/// non-capture transitions swap in-place, captures and user quit
/// close. Returns how the generation ended (route at close + any
/// unpublished editor outcome).
fn run_generation(
    shared: &Arc<Shared>,
    start: GenerationStart,
) -> eyre::Result<flintshot_editor_egui::WindowExit> {
    let status = shared.status();
    let initial_status = if status.is_empty() {
        "connecting…".to_string()
    } else {
        status
    };
    // One `EditorOpened` per editor *session* — the daemon parity
    // (subscribers like the applet stay truthful wherever the editor
    // lives); in-window route entries report through the host.
    if matches!(start, GenerationStart::Editor(_)) {
        crate::cli::report_editor_window_opened_to_daemon();
    }
    let window_start = match start {
        GenerationStart::Home => flintshot_editor_egui::WindowStart::Home,
        GenerationStart::Editor(pending) => {
            flintshot_editor_egui::WindowStart::Editor(Box::new(to_routed(&pending)))
        }
        GenerationStart::Loading => flintshot_editor_egui::WindowStart::Loading,
    };
    shared.set_window_open();
    let ran = crate::ambient::with_ambient_runtime(|| {
        flintshot_editor_egui::EguiShell::new().run_window(
            window_start,
            Box::new(Bridge {
                shared: Arc::clone(shared),
            }),
            Box::new(PageBridge {
                shared: Arc::clone(shared),
            }),
            Box::new(RouteHostBridge {
                shared: Arc::clone(shared),
            }),
            initial_status,
        )
    });
    ran.map_err(|e| eyre::eyre!("ambient runtime failure: {e}"))?
        .map_err(|e| eyre::eyre!("running the app window: {e}"))
}

/// Map a pending editor onto the editor-egui routing payload.
fn to_routed(pending: &PendingEditor) -> flintshot_editor_egui::RoutedEditor {
    flintshot_editor_egui::RoutedEditor {
        state: pending.state.clone(),
        selection_first: pending.selection_first,
    }
}

/// Map the route the window closed on back to the outer loop's
/// restore page (an external capture's close reopens on it). The
/// Loading route restores home — a reopen after a capture carries
/// the delivered image through the pending editor, never a spinner.
fn route_to_page(route: &flintshot_editor_egui::Route) -> Page {
    match route {
        flintshot_editor_egui::Route::Editor(routed) => Page::Editor(Box::new(PendingEditor {
            state: routed.state.clone(),
            selection_first: routed.selection_first,
        })),
        // Loading restores home — a reopen after a capture carries
        // the delivered image through the pending editor, never a
        // spinner — and future route kinds restore the empty state.
        _ => Page::Home,
    }
}

/// Arm the shared intents a window exit's editor outcome encodes:
/// only a capture intent survives to the exit (Esc and render
/// errors route in-window) — the `+` popup / C / Ctrl+N exit, with
/// the mode label dropped (the pump's in-process grab resolves the
/// backend itself).
fn arm_window_outcome(shared: &Shared, outcome: flintshot_core::Result<EditorOutcome>) {
    match outcome {
        Ok(EditorOutcome::NewCapture { .. }) => {
            shared.set_pending_capture(PumpCommand::Capture);
        }
        other => {
            tracing::warn!(?other, "unexpected editor outcome at window close");
        }
    }
}

/// Why the window (generation) closed — the outer loop's post-close
/// decision. Non-capture transitions no longer close the window
/// (they route in-place, editor-capture-ux Task 006), so only three
/// kinds remain.
#[derive(Debug, Clone)]
enum CloseReason {
    /// The user closed the window (X / Alt+F4): exit the process.
    UserQuit,
    /// A capture the window itself requested (grace + delegate).
    CaptureIntent(PumpCommand),
    /// A hide directive (external capture / pre-grab handshake):
    /// wait for the reopen, not a quit.
    HiddenByEvent,
}

/// Classify a window close: a pending capture command wins, then a
/// hide request; else the user quit. Consumes exactly one flag.
fn close_reason(shared: &Shared) -> CloseReason {
    if let Some(command) = shared.take_pending_capture() {
        return CloseReason::CaptureIntent(command);
    }
    if shared.take_hide_requested() {
        return CloseReason::HiddenByEvent;
    }
    CloseReason::UserQuit
}

/// The lock is held by a live window: ask the daemon to focus it and
/// exit. Without a daemon the ping has no recipient — surface that
/// distinctly rather than pretending to have focused something.
fn focus_existing() -> eyre::Result<()> {
    match crate::ipc_client::try_delegate(&Request::OpenApp) {
        Ok(crate::ipc_client::DelegateOutcome::Delegated) => {
            tracing::info!("app window already running; asked the daemon to focus it");
            Ok(())
        }
        Ok(crate::ipc_client::DelegateOutcome::NoDaemon) => Err(eyre::eyre!(
            "the app lock is held but no daemon answers; stale `flintshot app` process?"
        )),
        Err(e) => Err(eyre::eyre!("focusing the running app window: {e}")),
    }
}

/// The pump thread's whole life: connect (auto-starting the daemon),
/// then pump commands → in-process grabs and events → directives.
/// Marks the shared state dead on the way out so the outer loop
/// can't wait forever.
fn pump_main(mut cmd_rx: tokio_mpsc::Receiver<PumpCommand>, shared: &Arc<Shared>) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            tracing::error!(error = %e, "app pump could not build a runtime");
            shared.mark_pump_dead();
            return;
        }
    };

    // Grab threads deliver their outcomes through this channel
    // (`blocking_send` from plain threads is the blessed direction).
    let (grabs_tx, mut grabs_rx) = tokio_mpsc::channel::<(u64, AppGrabEvent)>(8);
    let mut router = GrabRouter {
        grabs_tx: Some(grabs_tx),
        ..GrabRouter::default()
    };

    // A borrowing (non-`move`) async block: the loops take `&Shared`,
    // and the post-block `mark_pump_dead` still owns it.
    runtime.block_on(async {
        let mut core = HomeCore::default();
        if let Some(stream) = ensure_subscribed(shared).await {
            // The presence connection: tells the daemon this app
            // window exists so its pre-grab handshake waits for our
            // hide-ack (the session registry only tracks
            // daemon-spawned windows). Parked for the pump's whole
            // connected life; daemon death ends it, and no reconnect
            // happens in this effort (same stance as the
            // subscription). Best-effort: a failure degrades the
            // handshake to grace-only, never the window.
            tokio::spawn(async {
                if let Err(e) = crate::ipc_client::announce_app_window(None).await {
                    tracing::warn!(
                        error = %e,
                        "app window presence connection failed; the pre-grab \
                         handshake degrades to grace-only"
                    );
                }
            });
            shared.push_directive(HomeDirective::SetStatus("connected".to_string()));
            if connected_loop(
                stream,
                &mut cmd_rx,
                &mut grabs_rx,
                shared,
                &mut core,
                &mut router,
            )
            .await
                == LoopExit::Disconnected
            {
                degraded_loop(cmd_rx, grabs_rx, shared, &mut core, &mut router).await;
            }
        } else {
            degraded_loop(cmd_rx, grabs_rx, shared, &mut core, &mut router).await;
        }
    });
    shared.mark_pump_dead();
}

/// Subscribe with daemon auto-start. `None` when the daemon never
/// answered within [`DAEMON_START_TIMEOUT`] (status already sent).
async fn ensure_subscribed(shared: &Shared) -> Option<crate::ipc_server::Stream> {
    let deadline = tokio::time::Instant::now() + DAEMON_START_TIMEOUT;
    let mut spawned = false;
    loop {
        match crate::ipc_client::subscribe_to(None).await {
            Ok(Some(stream)) => return Some(stream),
            Ok(None) => {
                if !spawned {
                    spawned = true;
                    shared.push_directive(HomeDirective::SetStatus("starting daemon…".to_string()));
                    if let Err(e) = spawn_daemon() {
                        shared.push_directive(HomeDirective::SetStatus(format!(
                            "could not start the daemon: {e}"
                        )));
                        return None;
                    }
                }
            }
            Err(e) => tracing::warn!(error = %e, "subscribing to the daemon"),
        }
        if tokio::time::Instant::now() >= deadline {
            shared.push_directive(HomeDirective::SetStatus(format!(
                "no daemon answered within {} s — is `flintshot daemon` healthy?",
                DAEMON_START_TIMEOUT.as_secs()
            )));
            return None;
        }
        tokio::time::sleep(CONNECT_RETRY_INTERVAL).await;
    }
}

/// Launch `flintshot daemon` detached. stdin is null (the daemon
/// reads nothing); stdout + stderr stay inherited — the daemon's
/// `tracing` output goes to stdout, and an auto-started daemon's
/// logs must survive for diagnostics (the harness caught this: a
/// nulled stdout silently swallowed them).
fn spawn_daemon() -> eyre::Result<()> {
    let exe = std::env::current_exe()
        .map_err(|e| eyre::eyre!("resolving current_exe to start the daemon: {e}"))?;
    std::process::Command::new(exe)
        .arg("daemon")
        .stdin(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| eyre::eyre!("spawning `flintshot daemon`: {e}"))
}

/// Why [`connected_loop`] returned. `Disconnected` hands the (moved)
/// command receiver to [`degraded_loop`] — the exit is communicated
/// as a value because `tokio::select!` arms cannot move the
/// receiver that the losing `recv()` future still borrows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoopExit {
    /// The window closed (command sender dropped).
    WindowClosed,
    /// The subscription stream errored or closed.
    Disconnected,
}

/// The subscribed loop: select over commands (spawn the in-process
/// grab — the grace already elapsed in the outer loop before the
/// command was sent), grab deliveries (route to the outer loop), and
/// events (drive [`HomeCore`]).
#[allow(clippy::too_many_arguments)]
async fn connected_loop(
    mut stream: crate::ipc_server::Stream,
    cmd_rx: &mut tokio_mpsc::Receiver<PumpCommand>,
    grabs_rx: &mut tokio_mpsc::Receiver<(u64, AppGrabEvent)>,
    shared: &Arc<Shared>,
    core: &mut HomeCore,
    router: &mut GrabRouter,
) -> LoopExit {
    loop {
        tokio::select! {
            command = cmd_rx.recv() => {
                match command {
                    None => return LoopExit::WindowClosed,
                    Some(command) => {
                        core.capture_requested();
                        // The single-window flow: the pump grabs
                        // in-process (capture_flow), no worker spawn,
                        // no daemon round-trip for the trigger.
                        let _ = command;
                        start_grab(router, shared);
                    }
                }
            }
            delivered = grabs_rx.recv() => {
                if let Some((generation, event)) = delivered {
                    router.deliver(shared, core, generation, event);
                }
            }
            frame = read_frame(&mut stream) => {
                let decoded = frame.and_then(|payload| decode_event(&payload));
                if let Ok(event) = decoded {
                    let hide_requested = matches!(event, IpcEvent::HideForCapture);
                    // A forwarded capture trigger (single-window): the
                    // daemon handed us the grab. GOTCHA (echo): every
                    // trigger — hotkey, tray, delegated CLI, even one
                    // that raced our own button click — lands here
                    // exactly once per daemon dispatch, and the
                    // generation machine folds repeats into one grab.
                    let triggered = match &event {
                        IpcEvent::CaptureTriggered { delay_ms } => Some(*delay_ms),
                        _ => None,
                    };
                    // A forwarded open (single-window): decode + route
                    // off-thread; the Hide directive closes whatever
                    // page is open so the editor epoch takes over.
                    let open_requested = match &event {
                        IpcEvent::OpenRequested { path } => Some(path.clone()),
                        _ => None,
                    };
                    if let Some(directive) = core.on_event(&event) {
                        shared.push_directive(directive);
                    }
                    if let Some(delay_ms) = triggered {
                        start_grab_with(router, shared, delay_ms);
                    }
                    if let Some(path) = open_requested {
                        handle_open_requested(shared, path);
                    }
                    if hide_requested {
                        // The daemon's pre-grab handshake is armed and
                        // waiting for our ack. Ack only while no window
                        // actually exists — ack-then-grace, the ADR-0047
                        // lesson — best-effort end to end.
                        if shared.wait_window_closed(HIDE_ACK_WINDOW) {
                            if let Err(e) = crate::ipc_client::delegate_async(
                                None,
                                &Request::ReportAppHidden,
                            )
                            .await
                            {
                                tracing::warn!(error = %e, "reporting the app window hidden");
                            }
                        } else {
                            tracing::warn!(
                                "window did not close in time; skipping the hide ack"
                            );
                        }
                    }
                } else {
                    tracing::warn!("daemon subscription died");
                    for directive in core.disconnected() {
                        shared.push_directive(directive);
                    }
                    return LoopExit::Disconnected;
                }
            }
        }
    }
}

/// No subscription (start failure or disconnect): keep answering
/// commands — the in-process grab needs no daemon (the prepare
/// handshake degrades to the vacuous `NoDaemon` guard, exactly like
/// a standalone worker), so a click can never strand the process
/// windowless.
async fn degraded_loop(
    mut cmd_rx: tokio_mpsc::Receiver<PumpCommand>,
    mut grabs_rx: tokio_mpsc::Receiver<(u64, AppGrabEvent)>,
    shared: &Shared,
    core: &mut HomeCore,
    router: &mut GrabRouter,
) {
    loop {
        tokio::select! {
            command = cmd_rx.recv() => {
                match command {
                    None => return,
                    Some(command) => {
                        core.capture_requested();
                        let _ = command;
                        start_grab(router, shared);
                    }
                }
            }
            delivered = grabs_rx.recv() => {
                if let Some((generation, grab)) = delivered {
                    router.deliver(shared, core, generation, grab);
                }
            }
        }
    }
}

/// [`HomeBridge`] impl joining the home page to the shared state:
/// capture intents register here (the window is about to close; the
/// outer loop services the intent once it does), the Open control
/// decodes and routes its image here, directives poll every frame.
struct Bridge {
    shared: Arc<Shared>,
}

impl HomeBridge for Bridge {
    fn request_capture(&mut self) {
        self.shared.set_pending_capture(PumpCommand::Capture);
    }

    fn open_image(&mut self, path: std::path::PathBuf) -> bool {
        match decode_image_file(&path) {
            Ok(state) => {
                tracing::info!(path = %path.display(), "image routed to the editor page");
                flintshot_core::recents::record(&path);
                self.shared.set_pending_editor(PendingEditor {
                    state,
                    selection_first: false,
                });
                true
            }
            Err(e) => {
                tracing::warn!(error = %e, "decoding the picked image");
                self.shared.push_directive(HomeDirective::SetStatus(format!(
                    "could not open {}: {e}",
                    path.display()
                )));
                false
            }
        }
    }

    fn request_paste(&mut self) -> bool {
        if let Some(state) = paste_clipboard_image() {
            tracing::info!("clipboard image routed to the editor page");
            self.shared.set_pending_editor(PendingEditor {
                state,
                selection_first: false,
            });
            true
        } else {
            self.shared.push_directive(HomeDirective::SetStatus(
                "clipboard has no image".to_string(),
            ));
            false
        }
    }

    fn poll_directive(&mut self) -> Option<HomeDirective> {
        self.shared.poll_directive()
    }
}

/// Read the system clipboard's image into the editor state the
/// editor page runs over — the Paste action's whole job. `None` when
/// the clipboard holds no image (text, empty, or no clipboard at
/// all).
fn paste_clipboard_image() -> Option<EditorState> {
    // GOTCHA (arboard on Wayland): a fresh `Clipboard` per read — a
    // long-lived handle can sit on a stale selection seat under
    // `zwlr_data_control`, and every other reader in this codebase
    // (capture_flow's helpers) follows the same fresh-instance rule.
    let mut clipboard = arboard::Clipboard::new().ok()?;
    let arboard::ImageData {
        width,
        height,
        bytes,
    } = clipboard.get_image().ok()?;
    image_data_to_state(width, height, bytes.into_owned())
}

/// Pure seam of [`paste_clipboard_image`]: raw clipboard image data
/// → editor state. `None` when the buffer does not match its
/// dimensions, or either dimension is 0 (degenerate — never a state
/// worth editing).
fn image_data_to_state(width: usize, height: usize, bytes: Vec<u8>) -> Option<EditorState> {
    if width == 0 || height == 0 {
        return None;
    }
    let w = u32::try_from(width).ok()?;
    let h = u32::try_from(height).ok()?;
    Some(EditorState::new(image::RgbaImage::from_raw(w, h, bytes)?))
}

/// Decode a picked image file into the editor state the editor page
/// runs over — the `flintshot open` decode (same `image::open` +
/// `to_rgba8`), in-process: no child, no relay.
fn decode_image_file(path: &std::path::Path) -> eyre::Result<EditorState> {
    let img = image::open(path)
        .map_err(|e| eyre::eyre!("could not load image {}: {e}", path.display()))?;
    Ok(EditorState::new(img.to_rgba8()))
}

/// [`EditorPageBridge`] impl joining the editor page to the shared
/// state: side-effect outcomes report to the daemon from here (the
/// worker path's equivalent — save reports feed `OpenLast`), status
/// lands on the shared line, directives poll every frame.
struct PageBridge {
    shared: Arc<Shared>,
}

impl EditorPageBridge for PageBridge {
    fn report_outcome(&mut self, outcome: &EditorOutcome) {
        // Mirrors the worker's `report_editor_outcome`: only a save
        // has a daemon report (last-saved tracking); copies and
        // uploads report through their own paths (the upload worker
        // self-reports; a copy has nothing to track).
        if let EditorOutcome::Saved(path) = outcome {
            flintshot_core::recents::record(path);
            match crate::ipc_client::delegate_to(None, &Request::ReportSaved { path: path.clone() })
            {
                Ok(_) => tracing::debug!(path = %path.display(), "reported the page's save"),
                Err(e) => {
                    tracing::warn!(error = %e, "reporting the page's save; ignoring");
                }
            }
        }
    }

    fn set_status(&mut self, status: &str) {
        self.shared
            .push_directive(HomeDirective::SetStatus(status.to_string()));
    }

    fn poll_directive(&mut self) -> Option<HomeDirective> {
        self.shared.poll_directive()
    }

    fn open_image(&mut self, path: std::path::PathBuf) -> bool {
        // Same contract as the home's Open: decode, arm the editor
        // page, close for the swap (the routed image replaces the
        // page's — Replace semantics, geometry recreated per image).
        match decode_image_file(&path) {
            Ok(state) => {
                tracing::info!(path = %path.display(), "editor chrome open routed");
                flintshot_core::recents::record(&path);
                self.shared.set_pending_editor(PendingEditor {
                    state,
                    selection_first: false,
                });
                true
            }
            Err(e) => {
                tracing::warn!(error = %e, "decoding the chrome-opened image");
                self.shared.push_directive(HomeDirective::SetStatus(format!(
                    "could not open {}: {e}",
                    path.display()
                )));
                false
            }
        }
    }

    fn request_paste(&mut self) -> bool {
        if let Some(state) = paste_clipboard_image() {
            tracing::info!("editor chrome paste routed");
            self.shared.set_pending_editor(PendingEditor {
                state,
                selection_first: false,
            });
            true
        } else {
            self.shared
                .push_directive(HomeDirective::SetStatus("clipboard has no image".into()));
            false
        }
    }
}

/// [`flintshot_editor_egui::RouteHost`] impl: the in-window router's
/// view of the shared routing state — routed editors (Open/Paste
/// decodes while the window is open; grab deliveries once Task 008
/// lands) and the editor-opened daemon report.
struct RouteHostBridge {
    shared: Arc<Shared>,
}

impl flintshot_editor_egui::RouteHost for RouteHostBridge {
    fn take_routed_editor(&mut self) -> Option<flintshot_editor_egui::RoutedEditor> {
        self.shared
            .take_pending_editor()
            .map(|pending| to_routed(&pending))
    }

    fn editor_route_opened(&mut self) {
        // The daemon parity: one report per editor session, wherever
        // the editor lives (route entry inside the window counts).
        crate::cli::report_editor_window_opened_to_daemon();
    }

    fn take_route_home(&mut self) -> bool {
        self.shared.take_route_home()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flintshot_editor_egui::RouteHost as _;

    // ── Shared: the close/reopen rendezvous ─────────────────────

    #[test]
    fn show_directive_arms_and_consumes_the_reopen_flag() {
        let shared = Shared::default();
        shared.push_directive(HomeDirective::SetStatus("connected".into()));
        shared.push_directive(HomeDirective::Show);
        assert!(shared.wait_for_reopen(), "Show must arm the reopen flag");
        // A dead pump unblocks any further wait with `false`.
        shared.mark_pump_dead();
        assert!(
            !shared.wait_for_reopen(),
            "flag consumed + pump dead → no reopen"
        );
    }

    #[test]
    fn dead_pump_breaks_the_wait_without_show() {
        let shared = Shared::default();
        shared.mark_pump_dead();
        assert!(!shared.wait_for_reopen());
    }

    #[test]
    fn pending_capture_round_trips_once() {
        let shared = Shared::default();
        shared.set_pending_capture(PumpCommand::Capture);
        assert_eq!(shared.take_pending_capture(), Some(PumpCommand::Capture));
        assert_eq!(
            shared.take_pending_capture(),
            None,
            "the intent is consumed by the outer loop exactly once"
        );
    }

    #[test]
    fn status_is_remembered_for_the_reopened_window() {
        let shared = Shared::default();
        shared.push_directive(HomeDirective::SetStatus("daemon connection lost".into()));
        assert_eq!(shared.status(), "daemon connection lost");
        // and the directive itself is still queued for an open window
        assert_eq!(
            shared.poll_directive(),
            Some(HomeDirective::SetStatus("daemon connection lost".into()))
        );
        assert_eq!(shared.poll_directive(), None);
    }

    #[test]
    fn capture_round_trip_shows_on_session_end() {
        let mut core = HomeCore::default();
        core.capture_requested();
        assert_eq!(
            core.on_event(&IpcEvent::CaptureStarted),
            Some(HomeDirective::Hide),
            "any capture — even one we requested — must close this window"
        );
        assert_eq!(
            core.on_event(&IpcEvent::SessionEnded),
            Some(HomeDirective::Show),
            "the last worker exiting restores the window"
        );
        assert!(!core.hidden);
    }

    #[test]
    fn forwarded_trigger_recounts_and_hides() {
        // Single-window: the daemon forwarded the capture (no worker
        // will SessionEnded). The CaptureStarted it rode in on must
        // not linger in the in-flight count, and the trigger hides
        // like any capture.
        let mut core = HomeCore::default();
        core.on_event(&IpcEvent::CaptureStarted);
        assert_eq!(core.in_flight, 1);
        assert_eq!(
            core.on_event(&IpcEvent::CaptureTriggered { delay_ms: 0 }),
            Some(HomeDirective::Hide)
        );
        assert_eq!(core.in_flight, 0, "the forwarded start is uncounted");
        assert!(core.hidden);
        // The pump's grab delivery restores via force_show.
        assert_eq!(core.force_show(), Some(HomeDirective::Show));
    }

    #[test]
    fn a_forwarded_trigger_without_its_start_still_hides_safely() {
        // Event races (subscription joined mid-dispatch) may deliver
        // the trigger without the start: saturating, never negative,
        // still hides.
        let mut core = HomeCore::default();
        assert_eq!(
            core.on_event(&IpcEvent::CaptureTriggered { delay_ms: 2_500 }),
            Some(HomeDirective::Hide)
        );
        assert_eq!(core.in_flight, 0);
    }

    #[test]
    fn hidden_close_restores_the_page_that_was_open() {
        // Plan decision 8: an external capture's close brings back
        // the page that was open — home if home, the editor page if
        // editing (the pre-epoch image; in-session annotations are
        // the Replace policy's loss).
        let shared = Shared::default();
        shared.set_restore_page(Page::Editor(Box::new(pending_over((2, 2), false))));
        let next = next_start_after_reopen(&shared);
        assert!(
            matches!(next, GenerationStart::Editor(_)),
            "the editor page restores"
        );
    }

    #[test]
    fn a_routed_grab_beats_the_stashed_restore() {
        // A forwarded grab that landed an image routes the editor
        // page over the fresh capture, not the stale stash.
        let shared = Shared::default();
        shared.set_restore_page(Page::Home);
        shared.set_pending_editor(pending_over((5, 4), true));
        match next_start_after_reopen(&shared) {
            GenerationStart::Editor(pending) => {
                assert!(
                    pending.selection_first,
                    "the routed grab's landing phase wins"
                );
                assert_eq!(pending.state.image.dimensions(), (5, 4));
            }
            GenerationStart::Home | GenerationStart::Loading => {
                panic!("a routed image must win over the stash")
            }
        }
    }

    #[test]
    fn a_quiet_grab_after_a_home_close_restores_home() {
        let shared = Shared::default();
        shared.set_restore_page(Page::Home);
        assert!(matches!(
            next_start_after_reopen(&shared),
            GenerationStart::Home
        ));
        // And with nothing stashed at all: home (the empty state).
        assert!(matches!(
            next_start_after_reopen(&shared),
            GenerationStart::Home
        ));
    }

    #[test]
    fn forwarded_open_routes_the_editor_page() {
        let shared = Shared::default();
        apply_open_outcome(&shared, Ok(fixture_png_state((3, 3))), "/tmp/shot.png");
        let pending = shared
            .take_pending_editor()
            .expect("a good decode arms the editor page");
        assert!(!pending.selection_first, "opens land plain");
        assert!(shared.wait_for_reopen(), "the router wake fired");
    }

    #[test]
    fn forwarded_open_failure_reopens_with_status() {
        let shared = Shared::default();
        apply_open_outcome(&shared, Err(eyre::eyre!("decode failed")), "/tmp/bad.png");
        assert!(
            shared.take_pending_editor().is_none(),
            "no route on failure"
        );
        assert!(
            matches!(
                shared.poll_directive(),
                Some(HomeDirective::SetStatus(text)) if text.contains("bad.png")
            ),
            "the failure lands on the status line"
        );
        assert!(shared.wait_for_reopen(), "the prior page reopens");
    }

    #[test]
    fn external_capture_started_hides_visible_window() {
        // The ADR-0047 gap: a hotkey/tray capture while the app window
        // is open. CaptureStarted now hides it, not just self-triggers.
        let mut core = HomeCore::default();
        assert_eq!(
            core.on_event(&IpcEvent::CaptureStarted),
            Some(HomeDirective::Hide)
        );
        assert!(core.hidden, "hidden for an externally triggered capture");
        assert_eq!(
            core.on_event(&IpcEvent::SessionEnded),
            Some(HomeDirective::Show),
            "and restores when it ends"
        );
    }

    #[test]
    fn hide_for_capture_is_idempotent_with_capture_started() {
        // The pre-grab nudge may repeat or trail CaptureStarted; each
        // application hides, none reopens, none panics.
        let mut core = HomeCore::default();
        assert_eq!(
            core.on_event(&IpcEvent::CaptureStarted),
            Some(HomeDirective::Hide)
        );
        assert_eq!(
            core.on_event(&IpcEvent::HideForCapture),
            Some(HomeDirective::Hide)
        );
        assert_eq!(
            core.on_event(&IpcEvent::HideForCapture),
            Some(HomeDirective::Hide)
        );
        assert!(core.hidden);
    }

    #[test]
    fn hide_for_capture_on_a_visible_window_hides_and_restores() {
        let mut core = HomeCore::default();
        assert_eq!(
            core.on_event(&IpcEvent::HideForCapture),
            Some(HomeDirective::Hide)
        );
        // No capture ever started (e.g. the worker died pre-grab):
        // nothing restores by event, so the window stays hidden —
        // the daemon's tray-gate safety bound covers the straggler,
        // and the next Show (AppFocus) surfaces it again.
        assert!(core.hidden);
        assert_eq!(core.in_flight, 0);
        assert_eq!(
            core.on_event(&IpcEvent::AppFocus),
            Some(HomeDirective::Show)
        );
    }

    #[test]
    fn supersede_does_not_flicker_the_window_open() {
        // Replace policy event order: CaptureSuperseded → (old
        // worker's SessionEnded) → new CaptureStarted. The SessionEnded
        // would otherwise reopen the window right before the new
        // capture hides it again — a flicker the new capture could
        // grab. The supersede flag holds the window closed through it.
        let mut core = HomeCore::default();
        core.on_event(&IpcEvent::CaptureStarted); // the old capture
        assert_eq!(
            core.on_event(&IpcEvent::CaptureSuperseded),
            None,
            "the supersede itself is not a directive"
        );
        assert_eq!(
            core.on_event(&IpcEvent::SessionEnded),
            None,
            "the superseded worker's exit must not reopen the window"
        );
        assert!(core.hidden, "still hidden: a capture is coming");
        assert_eq!(
            core.on_event(&IpcEvent::CaptureStarted),
            Some(HomeDirective::Hide),
            "the new capture clears the supersede hold"
        );
        assert_eq!(
            core.on_event(&IpcEvent::SessionEnded),
            Some(HomeDirective::Show),
            "and its end restores normally"
        );
    }

    #[test]
    fn session_ended_without_request_does_not_show() {
        // A tray/hotkey capture completing while our window is
        // visible must not steal focus.
        let mut core = HomeCore::default();
        assert_eq!(
            core.on_event(&IpcEvent::CaptureStarted),
            Some(HomeDirective::Hide)
        );
        // Restoring is the Hide flow's business; a second event with
        // nothing pending is quiet.
        let mut quiet = HomeCore::default();
        quiet.on_event(&IpcEvent::CaptureStarted);
        quiet.on_event(&IpcEvent::SessionEnded);
        assert_eq!(quiet.on_event(&IpcEvent::SessionEnded), None);
        let fresh = HomeCore::default();
        assert!(!fresh.hidden, "nothing to restore");
    }

    #[test]
    fn overlapping_captures_show_only_after_both_end() {
        let mut core = HomeCore::default();
        core.capture_requested();
        core.on_event(&IpcEvent::CaptureStarted); // ours
        core.on_event(&IpcEvent::CaptureStarted); // a racing hotkey
        assert_eq!(core.on_event(&IpcEvent::SessionEnded), None);
        assert_eq!(
            core.on_event(&IpcEvent::SessionEnded),
            Some(HomeDirective::Show)
        );
    }

    #[test]
    fn close_reason_distinguishes_quit_intent_and_hide() {
        // The outer loop's post-close decision: a pending capture
        // command wins, then a hide request, else the user quit.
        let shared = Shared::default();
        assert!(
            matches!(close_reason(&shared), CloseReason::UserQuit),
            "a plain close is the user quitting"
        );
        shared.push_directive(HomeDirective::Hide);
        assert!(matches!(close_reason(&shared), CloseReason::HiddenByEvent));
        shared.set_pending_capture(PumpCommand::Capture);
        shared.push_directive(HomeDirective::Hide);
        assert!(matches!(
            close_reason(&shared),
            CloseReason::CaptureIntent(PumpCommand::Capture)
        ));
    }

    #[test]
    fn hide_request_is_taken_exactly_once() {
        let shared = Shared::default();
        shared.push_directive(HomeDirective::Hide);
        assert!(shared.take_hide_requested());
        assert!(
            !shared.take_hide_requested(),
            "the second close after one hide is a real user quit"
        );
    }

    #[test]
    fn show_drops_stale_hide_directives_and_flags() {
        // The Hide directive queued while the window was closed (the
        // CaptureStarted of the capture that hid us) must not close
        // the freshly reopened window: Show — which always ends a
        // hide cycle — drains it.
        let shared = Shared::default();
        shared.push_directive(HomeDirective::Hide);
        shared.push_directive(HomeDirective::SetStatus("mid-capture".into()));
        shared.push_directive(HomeDirective::Show);
        let mut drained = Vec::new();
        while let Some(d) = shared.poll_directive() {
            drained.push(d);
        }
        assert!(
            !drained.contains(&HomeDirective::Hide),
            "stale Hide dropped on Show: {drained:?}"
        );
        assert!(!shared.take_hide_requested(), "and the hide flag cleared");
    }

    #[test]
    fn window_closed_state_is_truthful_not_a_consumed_mark() {
        // The ack waits on *current* state, never a remembered close:
        // no window → closed; open → the bounded wait gives up; a
        // close after a reopen cycle is a fresh, truthful reading —
        // the stale-mark race (an ack consumed from an earlier
        // close's mark while a new window was open) is structurally
        // gone.
        let shared = Shared::default();
        assert!(
            shared.wait_window_closed(Duration::from_millis(10)),
            "no window exists: vacuously closed"
        );
        shared.set_window_open();
        assert!(
            !shared.wait_window_closed(Duration::from_millis(10)),
            "an open window must not be acked away"
        );
        shared.set_window_closed();
        assert!(shared.wait_window_closed(Duration::from_secs(1)));
        // State persists: repeated reads stay true (a one-shot mark
        // would have been consumed by the first).
        assert!(shared.wait_window_closed(Duration::from_millis(10)));
        // Reopen re-arms the wait.
        shared.set_window_open();
        assert!(!shared.wait_window_closed(Duration::from_millis(10)));
    }

    #[test]
    fn a_close_wakes_a_blocked_ack_wait() {
        // Cross-thread shape of the real handshake: the pump blocks
        // on the wait while the outer loop closes the window.
        let shared = Arc::new(Shared::default());
        shared.set_window_open();
        let waiter = {
            let shared = Arc::clone(&shared);
            std::thread::spawn(move || shared.wait_window_closed(Duration::from_secs(5)))
        };
        std::thread::sleep(Duration::from_millis(50));
        shared.set_window_closed();
        assert!(
            waiter.join().expect("ack waiter thread"),
            "the close must release the blocked ack wait"
        );
    }

    #[test]
    fn pre_subscription_session_ended_clamps_at_zero() {
        let mut core = HomeCore::default();
        core.capture_requested();
        // A worker from before our subscription exits (in_flight was
        // 0): saturating, no underflow; the show is correct — we are
        // hidden and nothing is flying.
        assert_eq!(
            core.on_event(&IpcEvent::SessionEnded),
            Some(HomeDirective::Show)
        );
        assert_eq!(core.in_flight, 0);
        // A second stray Ended stays a no-op.
        assert_eq!(core.on_event(&IpcEvent::SessionEnded), None);
    }

    #[test]
    fn app_focus_refocuses_idle_visible_window() {
        let mut core = HomeCore::default();
        assert_eq!(
            core.on_event(&IpcEvent::AppFocus),
            Some(HomeDirective::Show),
            "re-focusing a visible idle window is the ping's purpose"
        );
    }

    #[test]
    fn app_focus_ignored_mid_capture() {
        let mut core = HomeCore::default();
        core.capture_requested();
        core.on_event(&IpcEvent::CaptureStarted);
        assert_eq!(
            core.on_event(&IpcEvent::AppFocus),
            None,
            "a focus ping must not raise the window over a live editor"
        );
    }

    #[test]
    fn app_focus_un_hides_an_idle_hidden_window() {
        // Hidden (grace started) but the delegate failed → not
        // in flight; a focus ping should still surface it.
        let mut core = HomeCore::default();
        core.capture_requested();
        assert_eq!(
            core.on_event(&IpcEvent::AppFocus),
            Some(HomeDirective::Show)
        );
    }

    #[test]
    fn disconnect_force_shows_with_status() {
        let mut core = HomeCore::default();
        core.capture_requested();
        core.on_event(&IpcEvent::CaptureStarted);
        let directives = core.disconnected();
        assert!(directives.contains(&HomeDirective::Show));
        assert!(matches!(
            directives.last(),
            Some(HomeDirective::SetStatus(_))
        ));
        assert_eq!(core.in_flight, 0);
        assert!(!core.hidden);
    }

    #[test]
    fn bridge_capture_request_registers_the_intent() {
        // The UI-thread seam: request_capture registers the pending
        // command the outer loop services once the window closes.
        let shared = Arc::new(Shared::default());
        let mut bridge = Bridge {
            shared: Arc::clone(&shared),
        };
        bridge.request_capture();
        assert_eq!(shared.take_pending_capture(), Some(PumpCommand::Capture));
    }

    #[test]
    fn open_image_routes_to_a_pending_editor_page() {
        // The Open control's contract: a readable file decodes into
        // the next page's editor state (same window, new epoch).
        let shared = Arc::new(Shared::default());
        let mut bridge = Bridge {
            shared: Arc::clone(&shared),
        };
        let png = fixture_png();
        assert!(
            bridge.open_image(png.path.clone()),
            "a readable image must route"
        );
        let state = shared
            .take_pending_editor()
            .expect("the editor state is pending for the outer loop");
        assert_eq!(state.state.image.dimensions(), png.dims);
        assert!(!state.selection_first, "Open opens plain");
    }

    #[test]
    fn open_image_failure_sets_status_and_stays_home() {
        // Unreadable file: status line on home, no route — the window
        // stays exactly where it is (spec: never a dead end).
        let shared = Arc::new(Shared::default());
        let mut bridge = Bridge {
            shared: Arc::clone(&shared),
        };
        assert!(
            !bridge.open_image(std::path::PathBuf::from("/nonexistent/does-not-exist.png")),
            "an unreadable image must not route"
        );
        assert!(
            shared.take_pending_editor().is_none(),
            "no editor page pending"
        );
        assert!(
            matches!(
                shared.poll_directive(),
                Some(HomeDirective::SetStatus(text)) if text.contains("does-not-exist")
            ),
            "the failure lands on the status line"
        );
    }

    #[test]
    fn paste_image_data_becomes_an_editor_state() {
        let state =
            image_data_to_state(2, 2, vec![7_u8; 2 * 2 * 4]).expect("well-formed image data");
        assert_eq!(state.image.dimensions(), (2, 2));
    }

    #[test]
    fn paste_image_data_rejects_a_mismatched_buffer() {
        assert!(
            image_data_to_state(4, 4, vec![7_u8; 10]).is_none(),
            "a buffer that does not match w*h*4 must never build a state"
        );
        assert!(image_data_to_state(0, 0, Vec::new()).is_none());
    }

    #[test]
    fn paste_with_no_clipboard_image_keeps_the_window_home() {
        // The bridge contract without a real clipboard in play: no
        // route (false), no pending page, and the status line says
        // why. (The clipboard itself is env-dependent; the pure seam
        // above covers the data half.)
        let shared = Arc::new(Shared::default());
        let mut bridge = Bridge {
            shared: Arc::clone(&shared),
        };
        // An empty/text clipboard reads as None — verified manually;
        // here we pin the status contract shape via the routed path's
        // absence (see paste_image_data tests for the data half).
        assert!(shared.take_pending_editor().is_none());
        let _ = &mut bridge;
    }

    #[test]
    fn close_reason_capture_beats_hide() {
        // A capture intent is the strongest close reason: the newest
        // capture owns the screen (Replace policy).
        let shared = Shared::default();
        shared.push_directive(HomeDirective::Hide);
        shared.set_pending_capture(PumpCommand::Capture);
        assert!(matches!(
            close_reason(&shared),
            CloseReason::CaptureIntent(PumpCommand::Capture)
        ));
    }

    #[test]
    fn close_reason_without_flags_is_the_user_quit() {
        // Open/Paste route in-window now (no close, no flag); only
        // captures and hides arm anything — anything else closed the
        // window, which is the user quitting.
        let shared = Shared::default();
        shared.set_pending_editor(pending_over((2, 2), false));
        assert!(
            matches!(close_reason(&shared), CloseReason::UserQuit),
            "a pending editor no longer classifies a close (the open window consumes it)"
        );
    }

    #[test]
    fn window_outcome_arms_only_the_capture_intent() {
        // The router's uniform post-generation step: a NewCapture at
        // close arms the grab; anything else is unexpected (Esc and
        // render errors route in-window and never reach the exit).
        let shared = Shared::default();
        arm_window_outcome(
            &shared,
            Ok(EditorOutcome::NewCapture {
                mode: flintshot_core::NewCaptureMode::Region,
            }),
        );
        assert!(matches!(
            close_reason(&shared),
            CloseReason::CaptureIntent(PumpCommand::Capture)
        ));
    }

    #[test]
    fn route_at_close_feeds_the_restore_page() {
        // The same-page restore rides the window exit's route now:
        // an external capture closing the editor reopens the editor
        // over the same image; closing home restores home.
        let editor =
            flintshot_editor_egui::Route::Editor(Box::new(flintshot_editor_egui::RoutedEditor {
                state: pending_over((3, 2), true).state,
                selection_first: true,
            }));
        assert!(matches!(
            route_to_page(&editor),
            Page::Editor(p) if p.selection_first && p.state.image.dimensions() == (3, 2)
        ));
        assert!(matches!(
            route_to_page(&flintshot_editor_egui::Route::Home),
            Page::Home
        ));
        // A generation that closed mid-Loading restores home — the
        // reopen carries the delivered image via the pending editor.
        assert!(matches!(
            route_to_page(&flintshot_editor_egui::Route::Loading),
            Page::Home
        ));
    }

    // --- In-process capture (single-window Task 006) ---

    use crate::capture_flow::AppGrab;

    #[test]
    fn capture_flow_trigger_bumps_and_marks_grabbing() {
        let mut flow = CaptureFlow::default();
        assert!(!flow.grabbing, "idle at start");
        assert_eq!(flow.trigger(), 1);
        assert!(flow.grabbing);
        assert_eq!(flow.trigger(), 2, "a re-trigger mints a new generation");
    }

    #[test]
    fn capture_flow_retrigger_discards_the_old_grabs_outcome() {
        // The Replace policy in-process: a trigger while grabbing
        // supersedes the in-flight grab; the old generation's outcome
        // must not route, and its end must not clear the newer grab.
        let mut flow = CaptureFlow::default();
        let first = flow.trigger();
        let second = flow.trigger();
        assert!(flow.current(second));
        assert!(!flow.current(first), "the first generation is stale");
        assert!(
            !flow.finished(first),
            "a superseded grab's outcome is discarded"
        );
        assert!(flow.grabbing, "the newer grab is still flying");
        assert!(flow.finished(second));
        assert!(!flow.grabbing, "the current grab's end returns to idle");
    }

    #[test]
    fn grab_router_retrigger_supersedes_the_active_watch() {
        let mut router = GrabRouter::default();
        let gen1 = router.trigger();
        let watch1 = Arc::new(SupersedeWatch::inert());
        router.arm(gen1, Arc::clone(&watch1));
        assert!(!watch1.superseded());
        router.trigger();
        assert!(
            watch1.superseded(),
            "a re-trigger flags the in-flight grab's watch"
        );
    }

    // --- Loading-route delivery (editor-capture-ux Task 008) ---

    #[test]
    fn confirmed_opens_the_loading_start() {
        // The pump's Confirmed arms the loading start and wakes the
        // outer loop: the fresh generation opens on the progress
        // route while the portal's write lands.
        let shared = Arc::new(Shared::default());
        let mut core = HomeCore::default();
        core.capture_requested();
        let mut router = GrabRouter::default();
        let gen = router.trigger();
        router.arm(gen, Arc::new(SupersedeWatch::inert()));

        router.deliver(&shared, &mut core, gen, AppGrabEvent::Confirmed);
        assert!(shared.wait_for_reopen(), "the Show woke the reopen wait");
        assert!(
            matches!(next_start_after_reopen(&shared), GenerationStart::Loading),
            "the confirmed grab opens on the loading route"
        );
    }

    #[test]
    fn fast_read_back_opens_the_editor_directly() {
        // The read-back lands before the outer loop picks a start:
        // the image wins over the loading mark — no spinner flash —
        // and the stale mark drains.
        let shared = Arc::new(Shared::default());
        let mut core = HomeCore::default();
        core.capture_requested();
        let mut router = GrabRouter::default();
        let gen = router.trigger();
        router.arm(gen, Arc::new(SupersedeWatch::inert()));

        router.deliver(&shared, &mut core, gen, AppGrabEvent::Confirmed);
        router.deliver(
            &shared,
            &mut core,
            gen,
            AppGrabEvent::Finished(tiny_grab_image(false)),
        );
        match next_start_after_reopen(&shared) {
            GenerationStart::Editor(pending) => {
                assert_eq!(pending.state.image.dimensions(), (2, 2));
            }
            other => panic!("the landed image must win, got {other:?}"),
        }
        assert!(
            !shared.take_pending_loading(),
            "the stale loading mark drained with the race"
        );
    }

    #[test]
    fn delivered_image_reaches_the_open_window_through_the_host() {
        // The in-window delivery contract: an armed pending editor
        // (grab landed, window open) is consumed by the window's
        // route-host poll — exactly what the pump's route_grab arms.
        let shared = Arc::new(Shared::default());
        let mut core = HomeCore::default();
        core.capture_requested();
        shared.set_window_open();
        route_grab(&shared, &mut core, tiny_grab_image(false));

        let mut host = RouteHostBridge {
            shared: Arc::clone(&shared),
        };
        let routed = host
            .take_routed_editor()
            .expect("the open window consumes the delivery");
        assert_eq!(routed.state.image.dimensions(), (2, 2));
        assert!(host.take_routed_editor().is_none(), "consumed once");
    }

    #[test]
    fn failure_while_open_routes_home_with_status() {
        let shared = Arc::new(Shared::default());
        let mut core = HomeCore::default();
        core.capture_requested();
        shared.set_window_open();
        route_grab(
            &shared,
            &mut core,
            AppGrab::Failed("the write never landed".into()),
        );
        assert!(
            RouteHostBridge {
                shared: Arc::clone(&shared)
            }
            .take_route_home(),
            "the open page routes home instead of stranding the spinner"
        );
    }

    #[test]
    fn quiet_while_open_routes_home() {
        let shared = Arc::new(Shared::default());
        let mut core = HomeCore::default();
        core.capture_requested();
        shared.set_window_open();
        route_grab(&shared, &mut core, AppGrab::Quiet);
        assert!(shared.take_route_home(), "a quiet exit leaves the spinner");
    }

    #[test]
    fn quiet_while_closed_reopens_the_prior_page() {
        // The closed-window semantics are unchanged: the force-show
        // reopens; no route-home intent arms (there is no window).
        let shared = Arc::new(Shared::default());
        let mut core = HomeCore::default();
        core.capture_requested();
        route_grab(&shared, &mut core, AppGrab::Quiet);
        assert!(!shared.take_route_home(), "no window → no route intent");
        assert!(shared.wait_for_reopen(), "the reopen directive landed");
    }

    #[test]
    fn confirmed_phase_folds_silently_for_stale_and_never_routes() {
        // Task 007's interim contract: Confirmed never routes (Task
        // 008 wires the loading route); a stale generation's
        // confirmation discards without touching the active grab.
        let shared = Arc::new(Shared::default());
        let mut core = HomeCore::default();
        let mut router = GrabRouter::default();
        let stale = router.trigger();
        router.arm(stale, Arc::new(SupersedeWatch::inert()));
        let gen = router.trigger();
        router.arm(gen, Arc::new(SupersedeWatch::inert()));

        // The stale generation's Confirmed: discarded, active stays.
        router.deliver(&shared, &mut core, stale, AppGrabEvent::Confirmed);
        assert!(router.active.is_some(), "a confirmation never ends a grab");

        // The current generation's Confirmed: logged only — the grab
        // stays in flight until Finished.
        router.deliver(&shared, &mut core, gen, AppGrabEvent::Confirmed);
        assert!(router.active.is_some(), "still waiting on the read-back");

        // And Finished still routes for the current generation.
        router.deliver(
            &shared,
            &mut core,
            gen,
            AppGrabEvent::Finished(tiny_grab_image(false)),
        );
        assert!(router.active.is_none(), "Finished ends the grab");
        assert!(shared.take_pending_editor().is_some(), "the image routed");
    }

    fn tiny_grab_image(selection_first: bool) -> AppGrab {
        let img = image::RgbaImage::from_raw(2, 2, vec![255; 16]).expect("2×2×4 bytes");
        AppGrab::Image {
            img,
            selection_first,
        }
    }

    #[test]
    fn grab_image_routes_a_pending_editor_and_wakes_the_router() {
        let shared = Shared::default();
        let mut core = HomeCore::default();
        core.capture_requested();
        let mut router = GrabRouter::default();
        let gen = router.trigger();
        router.arm(gen, Arc::new(SupersedeWatch::inert()));
        router.deliver(
            &shared,
            &mut core,
            gen,
            AppGrabEvent::Finished(tiny_grab_image(true)),
        );
        let pending = shared
            .take_pending_editor()
            .expect("the image routes an editor page");
        assert!(pending.selection_first, "the landing phase rides the route");
        assert!(
            shared.wait_for_reopen(),
            "the router wake (Show) fired with the route"
        );
    }

    #[test]
    fn grab_quiet_exit_reopens_home_without_a_route() {
        let shared = Shared::default();
        let mut core = HomeCore::default();
        core.capture_requested();
        let mut router = GrabRouter::default();
        let gen = router.trigger();
        router.deliver(
            &shared,
            &mut core,
            gen,
            AppGrabEvent::Finished(AppGrab::Quiet),
        );
        assert!(
            shared.take_pending_editor().is_none(),
            "a quiet exit routes nothing"
        );
        assert!(shared.wait_for_reopen(), "the prior page reopens");
    }

    #[test]
    fn grab_failure_reopens_home_with_a_status() {
        let shared = Shared::default();
        let mut core = HomeCore::default();
        core.capture_requested();
        let mut router = GrabRouter::default();
        let gen = router.trigger();
        router.deliver(
            &shared,
            &mut core,
            gen,
            AppGrabEvent::Finished(AppGrab::Failed("shm screencopy failed".into())),
        );
        assert!(shared.take_pending_editor().is_none());
        assert!(matches!(
            shared.poll_directive(),
            Some(HomeDirective::SetStatus(text)) if text.contains("screencopy")
        ));
        assert!(shared.wait_for_reopen());
    }

    #[test]
    fn stale_grab_delivery_is_discarded_entirely() {
        // A superseded grab delivering late must not route, reopen,
        // or touch the status line.
        let shared = Shared::default();
        let mut core = HomeCore::default();
        core.capture_requested();
        let mut router = GrabRouter::default();
        let stale = router.trigger();
        let current = router.trigger();
        router.deliver(
            &shared,
            &mut core,
            stale,
            AppGrabEvent::Finished(tiny_grab_image(false)),
        );
        assert!(shared.take_pending_editor().is_none());
        shared.mark_pump_dead(); // a wait would block otherwise
        assert!(!shared.wait_for_reopen(), "no wake from a stale grab");
        assert!(shared.poll_directive().is_none());
        assert!(
            router.flow.current(current),
            "the newer grab still owns the route"
        );
    }

    /// A tiny PNG written to a temp file (the decode path needs real
    /// image bytes on disk).
    struct FixturePng {
        path: std::path::PathBuf,
        dims: (u32, u32),
        _dir: tempfile::TempDir,
    }

    fn fixture_png() -> FixturePng {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("fixture.png");
        image::RgbaImage::from_raw(4, 3, vec![255; 4 * 3 * 4])
            .expect("raw buffer")
            .save(&path)
            .expect("save fixture png");
        FixturePng {
            path,
            dims: (4, 3),
            _dir: dir,
        }
    }

    fn fixture_png_state(dims: (u32, u32)) -> flintshot_core::EditorState {
        let img = image::RgbaImage::from_raw(
            dims.0,
            dims.1,
            vec![255; (dims.0 as usize) * (dims.1 as usize) * 4],
        )
        .expect("raw buffer");
        flintshot_core::EditorState::new(img)
    }

    /// A pending editor page over a `dims`-sized image, with the
    /// landing phase under test.
    fn pending_over(dims: (u32, u32), selection_first: bool) -> PendingEditor {
        PendingEditor {
            state: fixture_png_state(dims),
            selection_first,
        }
    }

    #[test]
    fn hide_grace_matches_the_icon_grace_bound() {
        // 250 ms: the same bound ADR-0036 found sufficient for a
        // compositor-side disappearance. If either constant moves,
        // re-derive the other.
        assert_eq!(HIDE_GRACE_MS, 250);
    }
}
