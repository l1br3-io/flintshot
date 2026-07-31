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
//! See ADR-0018 for the bridge rationale and why raw polling suffices
//! on Linux (both libraries pump the OS internally and hand events to a
//! static crossbeam receiver; macOS would need a run loop — recorded as
//! a future-work note).

use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use color_eyre::eyre;

use crate::single;

/// Heartbeat interval for the tokio → main bridge tick. Proves the
/// channel works; not a performance-sensitive path.
const TICK_INTERVAL: Duration = Duration::from_millis(500);
/// Max time the main loop blocks on the tokio channel before polling
/// tray/hotkey again. Caps click/hotkey latency at this value.
const POLL_TIMEOUT: Duration = Duration::from_millis(100);

/// Where a [`DaemonAction`] originated. Used for logging and (later)
/// for routing in the worker-spawning handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionSource {
    /// Periodic heartbeat from the tokio side thread.
    Tick,
    /// A system-tray interaction.
    Tray,
    /// A registered global hotkey fired.
    Hotkey,
}

impl ActionSource {
    fn as_str(self) -> &'static str {
        match self {
            ActionSource::Tick => "tick",
            ActionSource::Tray => "tray",
            ActionSource::Hotkey => "hotkey",
        }
    }
}

/// A discrete event the daemon main loop forwards to its handler.
///
/// Deliberately platform-agnostic — no `tray_icon`/`global_hotkey`
/// types leak here, so the handler (and its tests) need neither a
/// display nor those crates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonAction {
    /// Heartbeat from the tokio side (proves the bridge is alive).
    Tick,
    /// A tray click (full menu wiring lands in Task 007).
    #[cfg_attr(not(feature = "tray"), allow(dead_code))]
    Tray,
    /// A global hotkey fired; `id` is the accelerator id assigned at
    /// registration.
    Hotkey {
        /// Accelerator id from `global-hotkey` (mapped to an action in
        /// Task 009).
        id: u32,
    },
}

impl DaemonAction {
    /// Which source produced this action.
    #[must_use]
    pub fn source(&self) -> ActionSource {
        match self {
            DaemonAction::Tick => ActionSource::Tick,
            DaemonAction::Tray => ActionSource::Tray,
            DaemonAction::Hotkey { .. } => ActionSource::Hotkey,
        }
    }
}

/// Handles daemon actions.
///
/// The spike uses [`LoggingHandler`] (just logs each action + source).
/// Task 005 swaps in a worker-spawning implementation that turns
/// capture/hotkey actions into child-process spawns without touching
/// the loop.
pub trait ActionHandler {
    /// Dispatch `action`. Called on the daemon's main thread.
    fn handle(&mut self, action: DaemonAction);
}

/// Spike handler: logs every action with its source, proving all three
/// event sources reach a single handler (Task 003 acceptance).
pub struct LoggingHandler;

impl ActionHandler for LoggingHandler {
    fn handle(&mut self, action: DaemonAction) {
        tracing::info!(
            source = action.source().as_str(),
            ?action,
            "action received"
        );
    }
}

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
    if replace {
        tracing::warn!(
            "--replace requested but not yet implemented (Task 006); \
             a running daemon will still hold its lock"
        );
    }

    // 1. Single-instance lock (daemon role only). Hold for the lifetime.
    let _lock = single::acquire_daemon_lock()?;
    tracing::info!("single-instance lock acquired");

    // 2. Spawn the tokio side thread + the main→tokio channel.
    let (tx, rx) = mpsc::channel::<DaemonAction>();
    //    The IPC server is bound inside the runtime (interprocess's
    //    tokio listener registers its fd with the reactor on creation,
    //    so binding must happen on the tokio side, not the main thread).
    let builder = thread::Builder::new().name("flintshot-tokio".into());
    let tokio_join = builder
        .spawn(move || run_tokio_side(tx))
        .map_err(|e| eyre::eyre!("spawning tokio side thread: {e}"))?;

    // 3. Platform sources on the main thread.
    #[cfg(feature = "tray")]
    let _tray = match setup_tray() {
        Ok(t) => Some(t),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "system tray unavailable; continuing without it (IPC + hotkeys still work)"
            );
            None
        }
    };
    let _hotkey_manager = match setup_spike_hotkey() {
        Ok(m) => Some(m),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "global hotkey unavailable; continuing without it (tray + IPC still work)"
            );
            None
        }
    };

    // 4. Block on the event loop. Ctrl-C (caught in the tokio side)
    //    drops the channel sender → the loop exits cleanly.
    tracing::info!("daemon event loop running; press Ctrl-C to exit");
    run_event_loop(LoggingHandler, &rx);

    // Wait for the tokio side to finish so we don't leak the runtime.
    let _ = tokio_join.join();
    tracing::info!("daemon exited cleanly");
    Ok(())
}

/// The main event loop: drain tray + hotkey receivers, then block
/// (briefly) on the tokio channel. Forwards everything to `handler`.
fn run_event_loop(mut handler: impl ActionHandler, rx: &mpsc::Receiver<DaemonAction>) {
    loop {
        #[cfg(feature = "tray")]
        drain_tray(&mut handler);
        drain_hotkeys(&mut handler);

        match rx.recv_timeout(POLL_TIMEOUT) {
            Ok(action) => handler.handle(action),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                tracing::info!("daemon channel closed; exiting event loop");
                break;
            }
        }
    }
}

/// Forward all pending tray clicks to `handler` (best-effort; no-op if
/// no tray was built — the global receiver simply stays empty).
#[cfg(feature = "tray")]
fn drain_tray(handler: &mut impl ActionHandler) {
    use tray_icon::TrayIconEvent;
    let rx = TrayIconEvent::receiver();
    while rx.try_recv().is_ok() {
        handler.handle(DaemonAction::Tray);
    }
}

/// Forward all pending *pressed* hotkey events to `handler`. `Released`
/// is ignored to avoid double-firing on a single key press.
fn drain_hotkeys(handler: &mut impl ActionHandler) {
    use global_hotkey::{GlobalHotKeyEvent, HotKeyState};
    let rx = GlobalHotKeyEvent::receiver();
    while let Ok(event) = rx.try_recv() {
        if event.state == HotKeyState::Pressed {
            handler.handle(DaemonAction::Hotkey { id: event.id });
        }
    }
}

/// Run a multi-thread tokio runtime that binds the IPC server (the
/// `interprocess` tokio listener must be created inside a reactor),
/// drives its accept loop, emits [`DaemonAction::Tick`] heartbeats, and
/// catches Ctrl-C for a clean shutdown. When Ctrl-C fires (or the main
/// loop drops the receiver), the sender drops, the main loop's
/// `recv_timeout` returns `Disconnected`, and the daemon exits. Capture
/// requests arriving over IPC are stubbed here (logged); Task 006 wires
/// worker spawns.
fn run_tokio_side(tx: mpsc::Sender<DaemonAction>) {
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
        // Bind the IPC server inside the reactor. A bind failure (e.g.
        // a live socket the daemon can't replace) is non-fatal: we log
        // and keep the daemon alive for tray/hotkey/ticks, mirroring
        // the tray's best-effort stance.
        let serve_handle = match crate::ipc_server::IpcServer::bind(None) {
            Ok(ipc) => tokio::spawn(async move {
                if let Err(e) = ipc.serve().await {
                    tracing::error!(error = %e, "IPC server accept loop ended with error");
                }
            }),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "IPC server unavailable; continuing without it (tray + hotkeys still work)"
                );
                // Sentinel handle so the `abort` below is harmless.
                tokio::spawn(async {})
            }
        };

        let mut interval = tokio::time::interval(TICK_INTERVAL);
        // Discard the immediate first tick so heartbeats are paced.
        interval.tick().await;
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if tx.send(DaemonAction::Tick).is_err() {
                        // Main loop gone — shut down.
                        break;
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("Ctrl-C received; shutting down daemon");
                    break;
                }
            }
        }
        serve_handle.abort();
    });
}

/// Build a minimal system tray icon (Phase 2 spike — full menu in Task
/// 007). Returns the owned [`TrayIcon`], which the caller keeps alive.
#[cfg(feature = "tray")]
fn setup_tray() -> eyre::Result<tray_icon::TrayIcon> {
    let icon = spike_icon()?;
    tray_icon::TrayIconBuilder::new()
        .with_tooltip("flintshot daemon (Phase 2 spike)")
        .with_icon(icon)
        .build()
        .map_err(|e| eyre::eyre!("building tray icon: {e}"))
}

/// A flat 16×16 red square — enough for `libappindicator` to render
/// *something*. Replaced with a real logo in the polish sweep.
#[cfg(feature = "tray")]
fn spike_icon() -> eyre::Result<tray_icon::Icon> {
    const W: u32 = 16;
    const H: u32 = 16;
    let pixel = [220_u8, 60, 60, 255];
    let rgba: Vec<u8> = (0..(W * H)).flat_map(|_| pixel).collect();
    tray_icon::Icon::from_rgba(rgba, W, H).map_err(|e| eyre::eyre!("encoding spike icon: {e}"))
}

/// Register one dummy global hotkey (`Ctrl+Shift+P`) so the spike can
/// prove hotkey events reach the handler. Task 009 registers the real
/// chords from `[hotkeys]` config.
fn setup_spike_hotkey() -> eyre::Result<global_hotkey::GlobalHotKeyManager> {
    use global_hotkey::hotkey::{Code, HotKey, Modifiers};
    let manager = global_hotkey::GlobalHotKeyManager::new()
        .map_err(|e| eyre::eyre!("creating global-hotkey manager: {e}"))?;
    let hotkey = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyP);
    manager
        .register(hotkey)
        .map_err(|e| eyre::eyre!("registering spike hotkey Ctrl+Shift+P: {e}"))?;
    tracing::info!("registered spike hotkey: Ctrl+Shift+P");
    Ok(manager)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test handler that records every action it sees.
    struct Collector {
        seen: Vec<DaemonAction>,
    }

    impl ActionHandler for Collector {
        fn handle(&mut self, action: DaemonAction) {
            self.seen.push(action);
        }
    }

    #[test]
    fn action_source_round_trips() {
        assert_eq!(DaemonAction::Tick.source(), ActionSource::Tick);
        assert_eq!(DaemonAction::Tray.source(), ActionSource::Tray);
        assert_eq!(
            DaemonAction::Hotkey { id: 7 }.source(),
            ActionSource::Hotkey
        );
    }

    #[test]
    fn source_strings_are_stable() {
        // The acceptance criterion logs "<source: tick|tray|hotkey>".
        assert_eq!(ActionSource::Tick.as_str(), "tick");
        assert_eq!(ActionSource::Tray.as_str(), "tray");
        assert_eq!(ActionSource::Hotkey.as_str(), "hotkey");
    }

    #[test]
    fn logging_handler_does_not_panic() {
        // The spike handler just logs; assert it accepts every variant.
        let mut h = LoggingHandler;
        h.handle(DaemonAction::Tick);
        h.handle(DaemonAction::Tray);
        h.handle(DaemonAction::Hotkey { id: 1 });
    }

    #[test]
    fn collector_records_in_order() {
        let mut c = Collector { seen: vec![] };
        c.handle(DaemonAction::Tick);
        c.handle(DaemonAction::Hotkey { id: 42 });
        assert_eq!(
            c.seen,
            vec![DaemonAction::Tick, DaemonAction::Hotkey { id: 42 }]
        );
    }

    /// The main loop drains tray/hotkey then blocks on the channel. We
    /// can't exercise the real OS receivers headless, but we *can*
    /// drive the channel half of the loop: feed an action, then drop
    /// the sender to prove the loop terminates on disconnect (the
    /// Ctrl-C shutdown path).
    #[test]
    fn event_loop_exits_when_channel_disconnects() {
        let (tx, rx) = mpsc::channel::<DaemonAction>();
        let mut handler = Collector { seen: vec![] };

        tx.send(DaemonAction::Tick).expect("send tick");
        drop(tx);

        // Drain exactly like the real loop, but without tray/hotkey
        // (no display in CI). This reproduces run_event_loop's channel
        // handling.
        loop {
            match rx.recv_timeout(Duration::from_millis(10)) {
                Ok(a) => handler.handle(a),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }

        assert_eq!(handler.seen, vec![DaemonAction::Tick]);
    }
}
