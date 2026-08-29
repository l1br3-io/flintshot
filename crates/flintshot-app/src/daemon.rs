//! Daemon event loop + tokio/platform bridge (Phase 2 de-risk spike).
//!
//! This module proves the hardest integration in Phase 2: running a
//! tokio runtime (future IPC server + config watcher) in a background
//! thread while the platform event sources `tray-icon` and
//! `global-hotkey` need main-thread attention. The bridge is a plain
//! [`std::sync::mpsc`] channel: the tokio side forwards heartbeats
//! (and, later, IPC-derived actions) to the main thread, which also
//! drains `tray-icon` / `global-hotkey`'s crossbeam receivers and
//! forwards every event to a single [`ActionHandler`].
//!
//! See ADR-0018 for the bridge rationale. Event delivery is
//! per-source: `global-hotkey` hands events to a static crossbeam
//! receiver, so raw polling suffices for that half — but the
//! GTK-backed tray half (SNI registration via libappindicator, muda
//! menu-event delivery) only progresses while the default `GLib`
//! main context is iterated, which no library thread does for us.
//! [`pump_glib_context`] does it from the daemon loop each
//! iteration (ADR-0031). macOS would need a run loop — recorded as
//! a future-work note.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use color_eyre::eyre;
use tokio::sync::Notify;

use crate::single;

/// Heartbeat interval for the tokio → main bridge tick. Proves the
/// channel works; not a performance-sensitive path.
const TICK_INTERVAL: Duration = Duration::from_millis(500);
/// Max time the main loop blocks on the tokio channel before polling
/// tray/hotkey again. Caps click/hotkey latency at this value.
const POLL_TIMEOUT: Duration = Duration::from_millis(100);

/// Hard cap on `GLib` iterations per [`pump_glib_context`] call. Bounds
/// the worst case — a pathological always-ready source — so one pump
/// can never wedge the daemon loop; leftover work spreads over later
/// ticks.
#[cfg(all(feature = "tray", target_os = "linux"))]
const MAX_GLIB_PUMP_ITERATIONS: usize = 64;

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
    //    Hotkey backend is display-server-dependent (portal-first on
    //    Wayland: XWayland X11 grabs can register-but-never-fire, so
    //    they are skipped entirely there; the portal session runs on
    //    the tokio side instead — see hotkeys_portal).
    let wayland = crate::platform::session_is_wayland();
    let bus = crate::ipc_server::SubscriberBus::new();
    let state = crate::worker::DispatchState::new();
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
            move || run_tokio_side(tx, bus, state, wayland, hotkeys, bind_requested)
        })
        .map_err(|e| eyre::eyre!("spawning tokio side thread: {e}"))?;

    // 3. Platform sources on the main thread. The tray's
    // "Bind shortcuts…" item exists only where it works: Wayland
    // portal sessions.
    #[cfg(feature = "tray")]
    let _tray = match crate::tray::build(wayland) {
        Ok(t) => Some(t),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "system tray unavailable; continuing without it (IPC + hotkeys still work)"
            );
            None
        }
    };
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
    //    drops the channel sender → the loop exits cleanly.
    tracing::info!("daemon event loop running; press Ctrl-C to exit");
    let mut hotkey_registry = hotkey_registry;
    run_event_loop(
        &rx,
        &bus,
        &state,
        &mut hotkey_registry,
        config_watcher.as_ref(),
        wayland,
        &hotkeys_shared,
        &bind_requested,
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

/// The main event loop: drain tray menu + hotkey receivers, then block
/// (briefly) on the tokio heartbeat channel. Tray capture/quit clicks
/// and hotkey fires route through the worker dispatch table (same path
/// as IPC); the heartbeat just keeps the loop polling.
///
/// Returns when the tokio side drops its sender (Ctrl-C / IPC `Quit`)
/// or a tray *Quit* is clicked.
//
// Eight wiring params by design: this loop is the daemon's integration
// point, and each param is a distinct event source or shared state; a
// context struct would only relocate the list, not shrink it.
#[allow(clippy::too_many_arguments)]
fn run_event_loop(
    rx: &mpsc::Receiver<()>,
    #[cfg_attr(not(feature = "tray"), allow(unused_variables))]
    bus: &crate::ipc_server::SubscriberBus,
    state: &crate::worker::DispatchState,
    hotkeys: &mut crate::hotkeys::HotkeyRegistry,
    config_watcher: Option<&crate::config_watcher::ConfigWatcher>,
    wayland: bool,
    hotkeys_shared: &crate::hotkeys_portal::SharedHotkeys,
    #[cfg_attr(not(feature = "tray"), allow(unused_variables))] bind_requested: &Notify,
) {
    loop {
        // Before draining the tray: muda delivers menu events through
        // the default GLib context, so without this the crossbeam
        // receiver below would never see them (nor would the SNI
        // item ever register).
        #[cfg(all(feature = "tray", target_os = "linux"))]
        pump_glib_context();

        #[cfg(feature = "tray")]
        if drain_tray_menu(bus, state, bind_requested) {
            // Tray *Quit*: stop polling. `run` then returns, the lock
            // is released, and the daemon exits.
            tracing::info!("tray Quit clicked; exiting event loop");
            break;
        }
        drain_hotkeys(hotkeys, bus, state);

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

/// Drain the process-default `GLib` main context: up to
/// [`MAX_GLIB_PUMP_ITERATIONS`] non-blocking `iteration(false)` calls,
/// stopping early once nothing is pending.
///
/// Why: on Linux the tray stack (libappindicator behind `tray-icon`,
/// plus muda's menu plumbing) drives from `GLib` sources on the default
/// context — `RegisterStatusNotifierItem` and menu-event delivery only
/// happen while that context is iterated. The daemon owns no `GLib` main
/// loop, so without this pump the SNI object is exported on the bus but
/// never registers with the `StatusNotifierWatcher` (`TEST_MATRIX`
/// finding 3; full rationale: ADR-0031).
#[cfg(all(feature = "tray", target_os = "linux"))]
fn pump_glib_context() {
    let context = gtk::glib::MainContext::default();
    for _ in 0..MAX_GLIB_PUMP_ITERATIONS {
        if !context.iteration(false) {
            break;
        }
    }
}

/// Drain pending tray menu clicks, routing each through the worker
/// dispatch table (capture → spawn + `CaptureStarted`/`EditorOpened`;
/// quit → shutdown) or opening the config. Returns `true` if *Quit* was
/// clicked. Best-effort: a no-op if no tray was built.
#[cfg(feature = "tray")]
fn drain_tray_menu(
    bus: &crate::ipc_server::SubscriberBus,
    state: &crate::worker::DispatchState,
    bind_requested: &Notify,
) -> bool {
    use tray_icon::menu::MenuEvent;

    let rx = MenuEvent::receiver();
    while let Ok(event) = rx.try_recv() {
        match crate::tray::action_for(&event) {
            Some(crate::tray::TrayAction::Request(request)) => {
                match crate::worker::dispatch(
                    &request,
                    &crate::worker::CommandSpawner,
                    state,
                    &mut |ev| {
                        bus.broadcast(ev);
                    },
                ) {
                    Ok(crate::worker::DispatchOutcome::Shutdown) => return true,
                    Ok(_) => {}
                    Err(e) => tracing::warn!(error = %e, "tray dispatch failed"),
                }
            }
            Some(crate::tray::TrayAction::BindShortcuts) => {
                // Crosses into the portal session task (tokio side);
                // the dialog itself opens over there. notify_one, not
                // notify_waiters: the session task spends long windows
                // outside select (startup awaits, an open dialog), and
                // notify_waiters only wakes an already-parked waiter —
                // a click in those windows would vanish silently.
                // notify_one stores one permit, so the click survives
                // until the loop re-enters select (repeat clicks
                // collapse into that single queued bind).
                bind_requested.notify_one();
            }
            Some(crate::tray::TrayAction::OpenConfig) => {
                if let Err(e) = open_config_file() {
                    tracing::warn!(error = %e, "opening config from tray");
                }
            }
            None => {}
        }
    }
    false
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

/// Open `config.toml` in `$EDITOR` (fallback `$VISUAL`); print the path
/// if neither is set.
#[cfg(feature = "tray")]
fn open_config_file() -> eyre::Result<()> {
    use std::process::Command;

    let path = flintshot_core::config_path()
        .ok_or_else(|| eyre::eyre!("no platform config_dir available"))?;
    if let Some(editor) = std::env::var_os("EDITOR").or_else(|| std::env::var_os("VISUAL")) {
        Command::new(&editor)
            .arg(&path)
            .spawn()
            .map_err(|e| eyre::eyre!("launching {editor:?} on {}: {e}", path.display()))?;
        tracing::info!(editor = ?editor, path = %path.display(), "opened config");
    } else {
        println!("{}", path.display());
        tracing::info!(path = %path.display(), "no $EDITOR/$VISUAL; printed config path");
    }
    Ok(())
}

/// Drain pending *pressed* hotkey events, dispatching each through the
/// worker table (same path as tray/IPC). `Released` is ignored to avoid
/// double-firing. A no-op when hotkeys are unavailable (the registry is
/// empty).
fn drain_hotkeys(
    registry: &crate::hotkeys::HotkeyRegistry,
    bus: &crate::ipc_server::SubscriberBus,
    state: &crate::worker::DispatchState,
) {
    use global_hotkey::{GlobalHotKeyEvent, HotKeyState};

    let rx = GlobalHotKeyEvent::receiver();
    while let Ok(event) = rx.try_recv() {
        if event.state != HotKeyState::Pressed {
            continue;
        }
        if let Some(request) = registry.lookup(event.id) {
            if let Err(e) =
                crate::worker::dispatch(request, &crate::worker::CommandSpawner, state, &mut |ev| {
                    bus.broadcast(ev);
                })
            {
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
fn run_tokio_side(
    tx: mpsc::Sender<()>,
    bus: crate::ipc_server::SubscriberBus,
    state: crate::worker::DispatchState,
    wayland: bool,
    hotkeys: crate::hotkeys_portal::SharedHotkeys,
    bind_requested: Arc<Notify>,
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
            crate::hotkeys_portal::start(bus.clone(), state.clone(), hotkeys, bind_requested)
        } else {
            crate::hotkeys_portal::PortalHandle::unavailable(
                "hotkeys bind is Wayland-portal-only; on X11/macOS/Windows configure \
                 [hotkeys] chords in config.toml instead",
            )
        };

        // Bind the IPC server inside the reactor, sharing the daemon's
        // bus so tray/hotkey-initiated captures reach IPC subscribers.
        let (serve_handle, shutdown) =
            match crate::ipc_server::IpcServer::bind_with_bus(None, bus, state, portal) {
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

    /// [`super::pump_glib_context`] tests. The default `GLib` context is
    /// process-global and cargo runs `#[test]`s on parallel threads, so
    /// every test that attaches sources to it (or iterates it) must hold
    /// [`CONTEXT_LOCK`] — otherwise one test's pump would dispatch
    /// another test's pending sources.
    #[cfg(all(feature = "tray", target_os = "linux"))]
    mod glib_pump {
        use super::*;
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        use std::sync::{Arc, Mutex, MutexGuard};
        use std::time::{Duration, Instant};

        use gtk::glib;
        use gtk::glib::ControlFlow;

        /// Serializes the tests in this module around the shared
        /// default context.
        static CONTEXT_LOCK: Mutex<()> = Mutex::new(());

        fn context_lock() -> MutexGuard<'static, ()> {
            // Poisoning would only mean a sibling test already failed;
            // its panic is the real signal.
            CONTEXT_LOCK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        }

        #[test]
        fn idle_source_never_fires_without_a_pump() {
            let _serial = context_lock();
            let fired = Arc::new(AtomicBool::new(false));
            let flag = Arc::clone(&fired);
            glib::idle_add(move || {
                flag.store(true, Ordering::SeqCst);
                ControlFlow::Break
            });

            // Bounded wait with no context iteration: however long the
            // thread waits, GLib dispatches nothing — a source only runs
            // when someone iterates its context. This is the RED
            // condition the pump exists to fix.
            let deadline = Instant::now() + Duration::from_millis(50);
            while Instant::now() < deadline {
                thread::sleep(Duration::from_millis(5));
            }

            assert!(
                !fired.load(Ordering::SeqCst),
                "idle source fired without any context iteration"
            );
        }

        #[test]
        fn idle_source_fires_after_pumping() {
            let _serial = context_lock();
            let fired = Arc::new(AtomicBool::new(false));
            let flag = Arc::clone(&fired);
            glib::idle_add(move || {
                flag.store(true, Ordering::SeqCst);
                ControlFlow::Break
            });

            pump_glib_context();

            assert!(
                fired.load(Ordering::SeqCst),
                "idle source should dispatch during the pump"
            );
        }

        /// The wiring this branch exists for: [`super::run_event_loop`]
        /// must pump the default context every iteration. Drive the
        /// real loop headless (empty registry, no watcher, tray/hotkey
        /// receivers idle) with a channel that carries one heartbeat
        /// then disconnects; the idle source can only have fired if
        /// the loop itself pumped the context.
        #[test]
        fn run_event_loop_pumps_the_default_context() {
            let _serial = context_lock();
            let fired = Arc::new(AtomicBool::new(false));
            let flag = Arc::clone(&fired);
            glib::idle_add(move || {
                flag.store(true, Ordering::SeqCst);
                ControlFlow::Break
            });

            let (tx, rx) = mpsc::channel::<()>();
            tx.send(()).expect("send heartbeat");
            drop(tx);

            let bus = crate::ipc_server::SubscriberBus::new();
            let state = crate::worker::DispatchState::new();
            let mut hotkeys = crate::hotkeys::HotkeyRegistry::empty();
            let shared = Arc::new(Mutex::new(HotkeyConfig::default()));
            let bind_requested = Notify::new();

            run_event_loop(
                &rx,
                &bus,
                &state,
                &mut hotkeys,
                None,
                false,
                &shared,
                &bind_requested,
            );

            assert!(
                fired.load(Ordering::SeqCst),
                "idle source dispatched only if the loop pumped the GLib context"
            );
        }

        #[test]
        fn self_rearming_source_cannot_run_the_pump_forever() {
            // Every dispatch queues a successor, so the context is
            // never empty — the cap is the only thing that ends this.
            fn rearm_once(count: Arc<AtomicUsize>) {
                glib::idle_add(move || {
                    count.fetch_add(1, Ordering::SeqCst);
                    rearm_once(Arc::clone(&count));
                    ControlFlow::Break
                });
            }

            let _serial = context_lock();
            let count = Arc::new(AtomicUsize::new(0));
            rearm_once(Arc::clone(&count));

            pump_glib_context();

            let observed = count.load(Ordering::SeqCst);
            assert!(
                (1..=MAX_GLIB_PUMP_ITERATIONS).contains(&observed),
                "pump dispatched {observed} time(s); expected the \
                 MAX_GLIB_PUMP_ITERATIONS cap to bound it"
            );
        }
    }
}
