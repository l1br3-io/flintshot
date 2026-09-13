//! Daemon event loop + tokio/platform bridge (Phase 2 de-risk spike).
//!
//! This module proves the hardest integration in Phase 2: running a
//! tokio runtime (future IPC server + config watcher) in a background
//! thread while the platform event source `global-hotkey` needs
//! main-thread attention. The bridge is a plain [`std::sync::mpsc`]
//! channel: the tokio side forwards heartbeats (and, later,
//! IPC-derived actions) to the main thread, which also drains
//! `global-hotkey`'s crossbeam receiver and forwards every event to a
//! single [`ActionHandler`].
//!
//! See ADR-0018 for the bridge rationale. Since ADR-0042 the tray
//! icon lives in a supervised **helper process** (`flintshot tray`),
//! not in this loop: the helper's process-exit is the only causal
//! unregistration SNI supports, which is what closes the panel's
//! open menu popup before a grab. [`TraySupervisor`] here only
//! spawns/stops that process on the tray gate's orders — no GTK
//! here, and menu clicks arrive as ordinary IPC requests.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use color_eyre::eyre;
use tokio::sync::Notify;

use crate::single;
use crate::tray_gate::{TrayAction, TrayGate};

/// Heartbeat interval for the tokio → main bridge tick. Proves the
/// channel works; not a performance-sensitive path.
const TICK_INTERVAL: Duration = Duration::from_millis(500);
/// Max time the main loop blocks on the tokio channel before polling
/// hotkeys (and the tray helper's lifecycle) again. Caps hotkey
/// latency and exit-observation delay at this value.
const POLL_TIMEOUT: Duration = Duration::from_millis(100);

/// Run the daemon: acquire the single-instance lock, start the tokio
/// side thread, set up the tray (if built with `--features tray`) and a
/// dummy global hotkey, then block on the event loop until Ctrl-C.
///
/// `replace` (`flintshot daemon --replace`) is accepted but not yet
/// functional — the IPC `Request::Quit` → takeover path lands in Task
/// 006. For now a running daemon simply holds its lock and a second
/// invocation fails with "already running".
///
/// # Errors
/// - If the single-instance lock cannot be acquired.
/// - If the tokio side thread cannot be spawned.
/// - If global-hotkey registration fails (the hotkey path needs an X11
///   display on Linux; without one this surfaces as a clear error
///   rather than a silent no-op).
pub fn run(replace: bool) -> eyre::Result<()> {
    // `--replace`: ask any running daemon to quit, then wait for it to
    // release its socket before taking over. (Task 006 / ADR-0020.)
    if replace {
        replace_running_daemon()?;
    }

    // 1. Single-instance lock (daemon role only). Hold for the lifetime.
    //    After `--replace`, retry briefly to absorb the window between
    //    the old daemon's socket-file removal and its lock release.
    let _lock = if replace {
        single::acquire_daemon_lock_with_retry(Duration::from_secs(3))
    } else {
        single::acquire_daemon_lock()
    }?;
    tracing::info!("single-instance lock acquired");

    // Load config once at start (hot-reload is Task 010). A load failure
    // is non-fatal — fall back to defaults so the daemon still runs.
    let config = flintshot_core::Config::load().unwrap_or_else(|e| {
        tracing::warn!(error = %e, "could not load config; using built-in defaults");
        flintshot_core::Config::default()
    });

    // 2. Spawn the tokio side thread + the main→tokio channel. The
    //    subscriber bus is shared between the IPC server (tokio side)
    //    and the tray/hotkey dispatch (main loop), so a capture started
    //    from any source reaches every subscriber.
    //
    //    The tray gate crosses the other way (tokio IPC → main loop):
    //    capture workers ask it to hide this daemon's tray icon for
    //    the grab window (ADR-0036); only the main loop may touch the
    //    icon, so the gate hands it actions to apply.
    let wayland = crate::platform::session_is_wayland();
    let bus = crate::ipc_server::SubscriberBus::new();
    let state = crate::worker::DispatchState::new();
    let tray_gate = Arc::new(TrayGate::new());
    // Capture dispatch context: the session registry + the mutex
    // serializing supersede-then-spawn across every entry point
    // (IPC server, X11 hotkeys, portal hotkeys) — the Replace
    // policy's home. The prepare gate is the pre-grab handshake's
    // coordination state (stepped by this loop, awaited by the IPC
    // side) — the tray-gate pattern, generalized to every surface.
    let capture = crate::worker::CaptureDispatch::new();
    let prepare = Arc::new(crate::prepare_gate::PrepareGate::new());
    // The live `[hotkeys]` section: written by the config hot-reload
    // below, read by the portal task at bind time (fresh suggestions
    // without a daemon restart). The tray's bind trigger crosses the
    // main → tokio boundary through the `Notify` — `notify_waiters()`
    // is sync-callable from the main loop's tray handler.
    let hotkeys_shared = Arc::new(Mutex::new(config.hotkeys.clone()));
    let bind_requested = Arc::new(Notify::new());
    let (tx, rx) = mpsc::channel::<()>();
    let builder = thread::Builder::new().name("flintshot-tokio".into());
    let tokio_join = builder
        .spawn({
            let bus = bus.clone();
            let state = state.clone();
            let hotkeys = Arc::clone(&hotkeys_shared);
            let bind_requested = Arc::clone(&bind_requested);
            let tray_gate = Arc::clone(&tray_gate);
            let capture = capture.clone();
            let prepare = Arc::clone(&prepare);
            move || {
                run_tokio_side(
                    tx,
                    bus,
                    state,
                    wayland,
                    hotkeys,
                    bind_requested,
                    tray_gate,
                    capture,
                    prepare,
                );
            }
        })
        .map_err(|e| eyre::eyre!("spawning tokio side thread: {e}"))?;

    // 3. Platform sources on the main thread. The tray lives in a
    //    supervised helper process since ADR-0042: its process-exit
    //    is the only causal SNI unregistration (the panel destroys
    //    the open menu popup on it), which no in-process lever can
    //    achieve — libappindicator's wrapper never unrefs the
    //    GObject, and dropping a `TrayIcon` merely leaks a registered
    //    item (ADR-0039). The supervisor stops it for the grab window
    //    and respawns after; the watcher barrier and GLib pumping
    //    moved into the helper with the registration.
    #[cfg(feature = "tray")]
    let mut tray = TraySupervisor::start();
    #[cfg(not(feature = "tray"))]
    let mut tray = TraySupervisor;

    let hotkey_registry = if wayland {
        tracing::info!("Wayland session: global hotkeys via xdg-desktop-portal GlobalShortcuts");
        crate::hotkeys::HotkeyRegistry::empty()
    } else {
        crate::hotkeys::HotkeyRegistry::from_config(&config.hotkeys)
    };

    // Start the config hot-reload watcher (best-effort — a missing
    // config dir or notify failure just means no hot-reload).
    let config_watcher =
        flintshot_core::config_path().and_then(
            |p| match crate::config_watcher::ConfigWatcher::start(&p) {
                Ok(w) => Some(w),
                Err(e) => {
                    tracing::warn!(error = %e, "config hot-reload unavailable; continuing");
                    None
                }
            },
        );

    // 4. Block on the event loop. Ctrl-C (caught in the tokio side)
    //    drops the channel sender → the loop exits cleanly; an IPC
    //    `Request::Quit` (tray click, `--replace`) does the same from
    //    the tokio side. The loop takes `rx` by value so returning
    //    drops it, which is the tokio side's stop signal (its next
    //    heartbeat `send` fails).
    tracing::info!("daemon event loop running; press Ctrl-C to exit");
    let mut hotkey_registry = hotkey_registry;
    run_event_loop(
        rx,
        &bus,
        &state,
        &mut hotkey_registry,
        config_watcher.as_ref(),
        wayland,
        &hotkeys_shared,
        &tray_gate,
        &mut tray,
        &capture,
        &prepare,
    );

    // Wait for the tokio side to finish so we don't leak the runtime.
    let _ = tokio_join.join();

    tracing::info!("daemon exited cleanly");
    Ok(())
}

/// `--replace` handshake: send `Request::Quit` to any running daemon,
/// then wait (bounded) for it to release its IPC socket. A missing
/// daemon (no socket / connection refused) is not an error — there's
/// simply nothing to replace.
fn replace_running_daemon() -> eyre::Result<()> {
    use flintshot_core::ipc::Request;

    match crate::ipc_server::send_request_blocking(None, &Request::Quit) {
        Ok(()) => {
            tracing::info!("--replace: asked the running daemon to quit; waiting for it to exit");
        }
        Err(e) => {
            // No daemon reachable — start fresh.
            tracing::debug!(
                error = %e,
                "--replace: no running daemon to replace; starting fresh"
            );
            return Ok(());
        }
    }
    crate::ipc_server::wait_for_socket_to_free(None, Duration::from_secs(3))?;
    Ok(())
}

/// The main event loop: poll the tray helper's lifecycle, drain the
/// hotkey receiver, then block (briefly) on the tokio heartbeat
/// channel. Hotkey fires route through the worker dispatch table
/// (same path as IPC); the heartbeat just keeps the loop polling.
///
/// Returns when the tokio side drops its sender (Ctrl-C / IPC `Quit`).
/// The receiver is **owned** so both exits drop it: without the drop,
/// `run` would hold `rx` alive, the tokio side's heartbeat `send`s
/// would keep succeeding, and `run`'s `join()` would wait forever on a
/// thread that no longer has a reason to stop — the daemon hung
/// on-screen with a dead tray (every later click silently dropped,
/// L5 2026-08-31).
//
// Eleven wiring params by design: this loop is the daemon's integration
// point, and each param is a distinct event source or shared state; a
// context struct would only relocate the list, not shrink it.
#[allow(clippy::too_many_arguments)]
// Owned on purpose: dropping the receiver on return is the tokio
// side's stop signal (see the doc comment); a `&Receiver` here is
// exactly the hung-daemon bug.
#[allow(
    clippy::needless_pass_by_value,
    reason = "ownership is the shutdown contract; the body only reads it"
)]
fn run_event_loop(
    rx: mpsc::Receiver<()>,
    bus: &crate::ipc_server::SubscriberBus,
    state: &crate::worker::DispatchState,
    hotkeys: &mut crate::hotkeys::HotkeyRegistry,
    config_watcher: Option<&crate::config_watcher::ConfigWatcher>,
    wayland: bool,
    hotkeys_shared: &crate::hotkeys_portal::SharedHotkeys,
    tray_gate: &TrayGate,
    tray: &mut TraySupervisor,
    capture: &crate::worker::CaptureDispatch,
    prepare: &Arc<crate::prepare_gate::PrepareGate>,
) {
    loop {
        // Helper lifecycle first: a self-exit (capture/quit click, or
        // a crash) is the ADR-0042 pre-hide — arm the gate's
        // menu-anchored hold before anything waits on its graces.
        tray.poll(tray_gate);

        // Tray-gate tick: apply any stop/respawn a waiting capture
        // worker is owed before that worker's own reply bound
        // expires, and restart helpers whose captures have reported.
        service_tray_gate(tray_gate, tray);

        // Prepare-gate tick: drive any armed pre-grab handshake
        // (app-ack, live-capture re-assert) toward its reply.
        prepare.step(
            Instant::now(),
            capture.sessions().app_alive(),
            capture.sessions().live_captures(),
        );

        drain_hotkeys(hotkeys, bus, state, tray_gate, tray, capture);

        // Config hot-reload: re-read on change; re-register hotkeys only
        // when [hotkeys] actually changed. A mid-write parse failure is
        // tolerated (keep last-known-good, log, retry next change).
        if let Some(watcher) = config_watcher {
            if watcher.poll_changed() {
                match flintshot_core::Config::load() {
                    Ok(new_config) => {
                        if wayland {
                            apply_wayland_hotkey_reload(hotkeys_shared, &new_config.hotkeys);
                        } else if hotkeys.refresh(&new_config.hotkeys) {
                            tracing::info!("applied hotkey change from config");
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "config reload failed (likely mid-write); keeping last-known-good hotkeys"
                        );
                    }
                }
            }
        }

        match rx.recv_timeout(POLL_TIMEOUT) {
            Ok(()) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                tracing::info!("daemon channel closed; exiting event loop");
                break;
            }
        }
    }
}

/// The daemon's handle on the tray-helper process (ADR-0042). The
/// helper owns the SNI registration; this supervisor applies the tray
/// gate's actions as process lifecycle — `Hide` stops the helper
/// (its exit unregisters the item, which is what causally closes the
/// panel's open popup), `Show` respawns it (a fresh process gets a
/// fresh bus name, so the re-registration cannot duplicate — the
/// ADR-0039 drop/rebuild leak). Exists in no-tray builds as an inert
/// placeholder so the loop keeps one signature.
#[cfg(feature = "tray")]
struct TraySupervisor {
    /// The live helper, if one is running. Reaped lazily by [`Self::poll`].
    child: Option<std::process::Child>,
    /// We asked for the current child's stop (gate `Hide`); its exit
    /// is expected and must not re-arm the click pre-hide.
    stop_initiated: bool,
}

#[cfg(feature = "tray")]
impl TraySupervisor {
    /// Spawn the first helper. A spawn failure is logged, not fatal —
    /// the gate's restore cycle retries by construction.
    fn start() -> Self {
        let mut supervisor = Self {
            child: None,
            stop_initiated: false,
        };
        supervisor.apply(TrayAction::Show);
        supervisor
    }

    /// Reap an exited helper and teach the gate about exits we did
    /// not order. A self-exit is by design a capture/quit click (the
    /// helper exits to unregister; ADR-0042) or a crash — either way
    /// the item is already gone, so the pre-hide has no side effect
    /// to apply: it only anchors the gate's menu grace here and
    /// holds a respawn through the handshake window.
    fn poll(&mut self, gate: &TrayGate) {
        let Some(child) = &mut self.child else {
            return;
        };
        match child.try_wait() {
            Ok(Some(_)) => {
                self.child = None;
                if !self.stop_initiated {
                    tracing::info!(
                        "tray helper exited on its own (capture/quit click, or a crash)"
                    );
                    let _ = gate.pre_hide(
                        Instant::now(),
                        true,
                        crate::tray_gate::PreHideOrigin::TrayMenu,
                    );
                }
                self.stop_initiated = false;
            }
            Ok(None) => {}
            Err(e) => tracing::warn!(error = %e, "polling the tray helper"),
        }
    }

    /// Apply a gate-decided lifecycle change. Failures are non-fatal
    /// and logged — a wedged helper must never wedge the daemon loop.
    fn apply(&mut self, action: TrayAction) {
        match action {
            TrayAction::Hide => {
                if let Some(child) = &mut self.child {
                    match child.kill() {
                        Ok(()) => self.stop_initiated = true,
                        // Already-exited (or otherwise unkillable):
                        // `poll` classifies the exit; the gate is
                        // already `Hidden` by its own transition.
                        Err(e) => tracing::warn!(error = %e, "stopping the tray helper"),
                    }
                }
            }
            TrayAction::Show | TrayAction::ForceShow => {
                if let Some(mut old) = self.child.take() {
                    let _ = old.kill();
                    let _ = old.wait();
                }
                match spawn_helper() {
                    Ok(child) => {
                        tracing::debug!("tray helper spawned");
                        self.child = Some(child);
                    }
                    Err(e) => tracing::warn!(error = %e, "spawning the tray helper"),
                }
            }
        }
    }

    /// Whether a tray is supposed to exist — with the feature on,
    /// always (the supervisor owns the respawn, so "down" is a
    /// transient state the gate already knows about).
    #[allow(
        clippy::unused_self,
        reason = "mirrors the no-tray arm; a per-instance answer is meaningless"
    )]
    fn is_present(&self) -> bool {
        true
    }
}

/// Launch `flintshot tray` (this binary) as the helper. stderr is
/// inherited so helper logs land wherever the daemon's do; stdout is
/// null (the helper prints only config-path fallbacks).
#[cfg(feature = "tray")]
fn spawn_helper() -> std::io::Result<std::process::Child> {
    let exe = std::env::current_exe().map_err(|e| {
        std::io::Error::other(format!("resolving current_exe for the tray helper: {e}"))
    })?;
    std::process::Command::new(exe)
        .arg("tray")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .spawn()
}

/// The no-tray-build placeholder: nothing to supervise.
#[cfg(not(feature = "tray"))]
struct TraySupervisor;

#[cfg(not(feature = "tray"))]
impl TraySupervisor {
    #[allow(
        clippy::unused_self,
        reason = "no-tray builds have no helper; the method exists so callers keep one signature"
    )]
    fn poll(&mut self, _gate: &TrayGate) {}

    #[allow(
        clippy::unused_self,
        reason = "no-tray builds have no helper; the method exists so callers keep one signature"
    )]
    fn apply(&mut self, _action: TrayAction) {}

    #[allow(
        clippy::unused_self,
        reason = "no-tray builds have no helper; the method exists so callers keep one signature"
    )]
    fn is_present(&self) -> bool {
        false
    }
}

/// One tray-gate tick: advance the state machine and apply whatever
/// side effect it decided on. Runs every loop iteration (≤
/// [`POLL_TIMEOUT`]), so stops owed to waiting workers land well
/// inside their reply bound.
fn service_tray_gate(gate: &TrayGate, tray: &mut TraySupervisor) {
    if let Some(action) = gate.step(Instant::now(), tray.is_present()) {
        if action == TrayAction::ForceShow {
            tracing::warn!(
                "tray stayed hidden past the safety bound; restoring \
                 (a capture worker may have died before reporting)"
            );
        }
        tracing::debug!(?action, "tray gate transition");
        tray.apply(action);
    }
}

/// Pre-hide the tray for a dispatch that is about to spawn a capture
/// worker (hotkey fire): stops the helper at the interaction itself —
/// instant feedback, and the handshake's grace counts from here —
/// without touching the capture depth (the worker's
/// `HideTrayForCapture` owns that; one capture, one count). Hotkeys
/// close no menu, so the icon grace applies (ADR-0037); the helper
/// exit itself covers any popup left open elsewhere. See
/// ADR-0036/0037/0042.
fn pre_hide_for_capture(
    gate: &TrayGate,
    tray: &mut TraySupervisor,
    origin: crate::tray_gate::PreHideOrigin,
) {
    if gate.pre_hide(Instant::now(), tray.is_present(), origin) {
        tracing::debug!("stopping the tray helper for an imminent capture");
        tray.apply(TrayAction::Hide);
    }
}

/// Pure check: did the `[hotkeys]` section change between two config
/// reads? (`HotkeyConfig` deliberately has no `PartialEq` — fields
/// could grow without anyone noticing the comparison going stale.)
fn hotkeys_differ(new: &flintshot_core::HotkeyConfig, last: &flintshot_core::HotkeyConfig) -> bool {
    new.region != last.region || new.fullscreen != last.fullscreen
}

/// Wayland arm of the config hot-reload. This is *not* a missed
/// re-registration: on the portal path the daemon does not own the
/// grabs — the portal does, and swapping them requires the user-facing
/// bind dialog (`HotkeyRegistry::refresh` is X11-only by design). A
/// `[hotkeys]` edit therefore only refreshes the suggestion source the
/// next bind reads, plus this notice; the existing portal bindings
/// keep firing untouched.
fn apply_wayland_hotkey_reload(
    shared: &crate::hotkeys_portal::SharedHotkeys,
    fresh: &flintshot_core::HotkeyConfig,
) {
    let mut last = shared.lock().expect("hotkeys config mutex poisoned");
    if hotkeys_differ(fresh, &last) {
        last.clone_from(fresh);
        drop(last);
        tracing::info!(
            "hotkey suggestions changed; re-run `flintshot hotkeys bind` \
             (or the tray item) to update portal bindings — existing bindings keep firing"
        );
    }
}

/// Drain pending *pressed* hotkey events, dispatching each through the
/// worker table (same path as tray/IPC). `Released` is ignored to avoid
/// double-firing. A no-op when hotkeys are unavailable (the registry is
/// empty). Hotkey captures pre-hide the tray (ADR-0036) — the helper
/// stops here, its exit unregistering the item; the handshake that
/// follows owns the depth (ADR-0042) — and take the
/// supersede-then-spawn path (Replace policy): a new capture replaces
/// any live session before spawning, serialized with every other entry
/// point by the capture dispatch context.
fn drain_hotkeys(
    registry: &crate::hotkeys::HotkeyRegistry,
    bus: &crate::ipc_server::SubscriberBus,
    state: &crate::worker::DispatchState,
    tray_gate: &TrayGate,
    tray: &mut TraySupervisor,
    capture: &crate::worker::CaptureDispatch,
) {
    use global_hotkey::{GlobalHotKeyEvent, HotKeyState};

    let rx = GlobalHotKeyEvent::receiver();
    while let Ok(event) = rx.try_recv() {
        if event.state != HotKeyState::Pressed {
            continue;
        }
        if let Some(request) = registry.lookup(event.id) {
            if crate::worker::spawns_capture_worker(request) {
                // Hotkey: no menu is closing, so the icon grace alone.
                pre_hide_for_capture(tray_gate, tray, crate::tray_gate::PreHideOrigin::Other);
                if let Err(e) = capture.dispatch_capture_blocking(
                    request,
                    &crate::worker::CommandSpawner::with_bus_and_sessions(
                        bus.clone(),
                        capture.sessions(),
                    ),
                    state,
                    &mut |ev| {
                        bus.broadcast(ev);
                    },
                ) {
                    tracing::warn!(error = %e, "hotkey capture dispatch failed");
                }
            } else if let Err(e) = crate::worker::dispatch(
                request,
                &crate::worker::CommandSpawner::with_bus_and_sessions(
                    bus.clone(),
                    capture.sessions(),
                ),
                state,
                &mut |ev| {
                    bus.broadcast(ev);
                },
            ) {
                tracing::warn!(error = %e, "hotkey dispatch failed");
            }
        }
    }
}

/// Run a multi-thread tokio runtime that binds the IPC server (the
/// `interprocess` tokio listener must be created inside a reactor),
/// drives its accept loop, emits heartbeat ticks on `tx`, and catches
/// Ctrl-C / IPC `Quit` for a clean shutdown. When the tokio side stops
/// (Ctrl-C or `Request::Quit` via `--replace`), the sender drops, the
/// main loop's `recv_timeout` returns `Disconnected`, and the daemon
/// exits.
// Eight wiring params by design (mirrors `run_event_loop`): each is a
// distinct event source or shared state crossing into the tokio side.
#[allow(clippy::too_many_arguments)]
fn run_tokio_side(
    tx: mpsc::Sender<()>,
    bus: crate::ipc_server::SubscriberBus,
    state: crate::worker::DispatchState,
    wayland: bool,
    hotkeys: crate::hotkeys_portal::SharedHotkeys,
    bind_requested: Arc<Notify>,
    tray_gate: Arc<TrayGate>,
    capture: crate::worker::CaptureDispatch,
    prepare: Arc<crate::prepare_gate::PrepareGate>,
) {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            tracing::error!(error = %e, "failed to build tokio runtime; daemon will be degraded");
            return;
        }
    };

    runtime.block_on(async move {
        // Hotkey backend. Wayland: portal GlobalShortcuts session
        // (replaces X11 global-hotkey grabs); the handle is the IPC
        // layer's entry point for `Request::BindShortcuts`. Non-Wayland:
        // a permanently-unavailable handle so a stray bind request
        // gets a clear "portal-only" error instead of a hang.
        let portal = if wayland {
            crate::hotkeys_portal::start(
                bus.clone(),
                state.clone(),
                hotkeys,
                bind_requested,
                capture.clone(),
            )
        } else {
            crate::hotkeys_portal::PortalHandle::unavailable(
                "hotkeys bind is Wayland-portal-only; on X11/macOS/Windows configure \
                 [hotkeys] chords in config.toml instead",
            )
        };

        // Bind the IPC server inside the reactor, sharing the daemon's
        // bus so tray/hotkey-initiated captures reach IPC subscribers,
        // the tray gate so worker handshakes reach the main loop, and
        // the capture dispatch context so every capture trigger runs
        // the same serialized supersede-then-spawn.
        let (serve_handle, shutdown) = match crate::ipc_server::IpcServer::bind_with_bus(
            None, bus, state, portal, tray_gate, capture, prepare,
        ) {
            Ok(ipc) => {
                let shutdown = ipc.shutdown_signal();
                let handle = tokio::spawn(async move {
                    if let Err(e) = ipc.serve().await {
                        tracing::error!(error = %e, "IPC server accept loop ended with error");
                    }
                });
                (handle, Some(shutdown))
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "IPC server unavailable; continuing without it (tray + hotkeys still work)"
                );
                (tokio::spawn(async {}), None)
            }
        };

        let mut interval = tokio::time::interval(TICK_INTERVAL);
        // Discard the immediate first tick so heartbeats are paced.
        interval.tick().await;
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if tx.send(()).is_err() {
                        // Main loop gone — shut down.
                        break;
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("Ctrl-C received; shutting down daemon");
                    break;
                }
                // `Request::Quit` over IPC (the `--replace` path): stop
                // the ticker so `tx` drops and the main loop exits too.
                () = async {
                    if let Some(s) = shutdown.as_ref() {
                        s.notified().await;
                    } else {
                        std::future::pending::<()>().await;
                    }
                } => {
                    tracing::info!("IPC shutdown requested; stopping daemon");
                    break;
                }
            }
        }
        serve_handle.abort();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Wayland config hot-reload semantics (Task 004) ──────────

    use flintshot_core::HotkeyConfig;
    use std::sync::{Arc, Mutex};

    #[test]
    fn hotkeys_differ_detects_region_or_fullscreen_change() {
        let base = HotkeyConfig::default();
        let mut region_only = base.clone();
        region_only.region = "Ctrl+Shift+KeyR".into();
        let mut fullscreen_only = base.clone();
        fullscreen_only.fullscreen = "Ctrl+Alt+KeyF".into();
        assert!(hotkeys_differ(&region_only, &base));
        assert!(hotkeys_differ(&fullscreen_only, &base));
        assert!(hotkeys_differ(&base, &region_only));
    }

    #[test]
    fn hotkeys_identical_means_no_diff() {
        let a = HotkeyConfig::default();
        let b = HotkeyConfig::default();
        assert!(!hotkeys_differ(&a, &b));
    }

    #[test]
    fn wayland_reload_updates_the_suggestion_source() {
        let shared = Arc::new(Mutex::new(HotkeyConfig::default()));
        let mut fresh = HotkeyConfig::default();
        fresh.region = "Ctrl+Shift+KeyR".into();
        apply_wayland_hotkey_reload(&shared, &fresh);
        assert_eq!(
            shared.lock().expect("hotkeys mutex poisoned").region,
            "Ctrl+Shift+KeyR"
        );
    }

    #[test]
    fn wayland_reload_is_idempotent_for_unchanged_hotkeys() {
        // The watcher fires on ANY config edit; an unrelated section
        // change must not touch the suggestion source (or spam the
        // notice).
        let shared = Arc::new(Mutex::new(HotkeyConfig::default()));
        apply_wayland_hotkey_reload(&shared, &HotkeyConfig::default());
        assert_eq!(
            shared.lock().expect("hotkeys mutex poisoned").region,
            HotkeyConfig::default().region
        );
    }

    /// The main loop drains tray/hotkey then blocks on the heartbeat
    /// channel. We can't exercise the real OS receivers headless, but we
    /// can drive the channel half: feed a tick, then drop the sender to
    /// prove the loop terminates on disconnect (the Ctrl-C / `--replace`
    /// shutdown path).
    #[test]
    fn loop_terminates_when_channel_disconnects() {
        let (tx, rx) = mpsc::channel::<()>();
        let mut saw_tick = false;
        tx.send(()).expect("send heartbeat");
        drop(tx);

        // Drain exactly like run_event_loop's channel handling (no
        // tray/hotkey — no display in CI).
        loop {
            match rx.recv_timeout(Duration::from_millis(10)) {
                Ok(()) => saw_tick = true,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }

        assert!(
            saw_tick,
            "the heartbeat should have arrived before disconnect"
        );
    }

    /// A tray *Quit* breaks the event loop from the main side while
    /// the tokio side is still sending heartbeats. `run_event_loop`
    /// owns its receiver, so its return must drop it — that drop is
    /// the tokio side's only stop signal on this path. Model the
    /// handshake: hold the sender, "return" from the loop (drop the
    /// receiver), and require the next send to fail. With the old
    /// `&Receiver` signature the send kept succeeding forever — the
    /// exact mechanism behind the hung-after-Quit daemon.
    #[test]
    fn loop_return_drops_the_receiver_the_tokio_side_stops_on() {
        let (tx, rx) = mpsc::channel::<()>();
        tx.send(()).expect("heartbeat");

        // Loop body: one heartbeat, then a tray-Quit-style return.
        // Dropping `rx` here stands in for run_event_loop returning.
        {
            let rx = rx;
            assert!(rx.recv_timeout(Duration::from_millis(10)).is_ok());
        }

        assert!(
            tx.send(()).is_err(),
            "receiver must be gone once the event loop returns"
        );
    }

    // ── TraySupervisor: helper lifecycle under test ─────────────

    /// A helper we stopped via `apply(Hide)` is reaped without arming
    /// the click pre-hide (`stop_initiated`): far past the hold
    /// window the gate is still idle — an armed pre-hide would have
    /// produced `Show` there. `Show` then respawns.
    #[cfg(all(feature = "tray", unix))]
    #[test]
    fn stopped_helper_exit_is_not_treated_as_a_click() {
        let gate = TrayGate::new();
        let mut supervisor = TraySupervisor {
            child: Some(
                std::process::Command::new("sleep")
                    .arg("30")
                    .spawn()
                    .unwrap(),
            ),
            stop_initiated: false,
        };

        supervisor.apply(TrayAction::Hide);
        // SIGKILL is asynchronous — reap with a bounded spin.
        let deadline = Instant::now() + Duration::from_secs(2);
        while supervisor.child.is_some() && Instant::now() < deadline {
            supervisor.poll(&gate);
            if supervisor.child.is_some() {
                thread::sleep(Duration::from_millis(10));
            }
        }
        assert!(
            supervisor.child.is_none(),
            "the stopped child must be reaped"
        );
        assert!(!supervisor.stop_initiated, "the flag resets on reap");
        assert_eq!(
            gate.step(Instant::now() + Duration::from_secs(1), true),
            None,
            "an ordered stop must not arm the click pre-hide (no Show owed)"
        );

        supervisor.apply(TrayAction::Show);
        assert!(
            supervisor.child.is_some(),
            "Show respawns a helper (this test binary's `tray` arg — harmless)"
        );
        // Reap the spawned helper so the test leaves nothing behind.
        supervisor.apply(TrayAction::Hide);
        if let Some(mut child) = supervisor.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// A helper that dies on its own (capture/quit click, or crash)
    /// arms the gate's click-anchored pre-hide: a handshake landing
    /// afterwards confirms only after the **menu** grace — the exit
    /// started the popup-destruction chain, it did not finish it.
    #[cfg(all(feature = "tray", unix))]
    #[test]
    fn self_exited_helper_arms_the_click_pre_hide() {
        let gate = TrayGate::new();
        let mut supervisor = TraySupervisor {
            // `true` exits immediately.
            child: Some(std::process::Command::new("true").spawn().unwrap()),
            stop_initiated: false,
        };

        // Bounded spin: the exit may not be visible on the first poll.
        let deadline = Instant::now() + Duration::from_secs(2);
        while supervisor.child.is_some() && Instant::now() < deadline {
            supervisor.poll(&gate);
            if supervisor.child.is_some() {
                thread::sleep(Duration::from_millis(10));
            }
        }
        assert!(supervisor.child.is_none(), "the exit was reaped");

        let t0 = Instant::now();
        let baseline = gate.begin_capture(); // the worker's handshake lands
        assert_eq!(
            gate.step(t0, true),
            None,
            "hidden by the armed pre-hide; no new side effect"
        );
        assert_eq!(
            gate.confirmed_epoch(),
            baseline,
            "icon grace (250 ms) alone must not confirm a click-anchored hide"
        );
        assert_eq!(
            gate.step(t0 + Duration::from_millis(250), true),
            None,
            "icon-grace elapsed: still waiting the menu grace"
        );
        assert_eq!(
            gate.confirmed_epoch(),
            baseline,
            "no ack before the menu grace (600 ms from the observed exit)"
        );
        gate.step(t0 + Duration::from_millis(600), true);
        assert!(
            gate.confirmed_epoch() > baseline,
            "menu grace elapsed: the ack may leave (the popup is gone)"
        );
    }
}
