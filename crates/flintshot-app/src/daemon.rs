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
    let bus = crate::ipc_server::SubscriberBus::new();
    let (tx, rx) = mpsc::channel::<()>();
    let builder = thread::Builder::new().name("flintshot-tokio".into());
    let tokio_join = builder
        .spawn({
            let bus = bus.clone();
            move || run_tokio_side(tx, bus)
        })
        .map_err(|e| eyre::eyre!("spawning tokio side thread: {e}"))?;

    // 3. Platform sources on the main thread.
    #[cfg(feature = "tray")]
    let _tray = match crate::tray::build() {
        Ok(t) => Some(t),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "system tray unavailable; continuing without it (IPC + hotkeys still work)"
            );
            None
        }
    };
    let hotkey_registry = crate::hotkeys::HotkeyRegistry::from_config(&config.hotkeys);

    // 4. Block on the event loop. Ctrl-C (caught in the tokio side)
    //    drops the channel sender → the loop exits cleanly.
    tracing::info!("daemon event loop running; press Ctrl-C to exit");
    run_event_loop(&rx, &bus, &hotkey_registry);

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
fn run_event_loop(
    rx: &mpsc::Receiver<()>,
    #[cfg_attr(not(feature = "tray"), allow(unused_variables))]
    bus: &crate::ipc_server::SubscriberBus,
    hotkeys: &crate::hotkeys::HotkeyRegistry,
) {
    loop {
        #[cfg(feature = "tray")]
        if drain_tray_menu(bus) {
            // Tray *Quit*: stop polling. `run` then returns, the lock
            // is released, and the daemon exits.
            tracing::info!("tray Quit clicked; exiting event loop");
            break;
        }
        drain_hotkeys(hotkeys, bus);

        match rx.recv_timeout(POLL_TIMEOUT) {
            Ok(()) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                tracing::info!("daemon channel closed; exiting event loop");
                break;
            }
        }
    }
}

/// Drain pending tray menu clicks, routing each through the worker
/// dispatch table (capture → spawn + `CaptureStarted`/`EditorOpened`;
/// quit → shutdown) or opening the config. Returns `true` if *Quit* was
/// clicked. Best-effort: a no-op if no tray was built.
#[cfg(feature = "tray")]
fn drain_tray_menu(bus: &crate::ipc_server::SubscriberBus) -> bool {
    use tray_icon::menu::MenuEvent;

    let rx = MenuEvent::receiver();
    while let Ok(event) = rx.try_recv() {
        match crate::tray::action_for(&event) {
            Some(crate::tray::TrayAction::Request(request)) => {
                match crate::worker::dispatch(&request, &crate::worker::CommandSpawner, &mut |ev| {
                    bus.broadcast(ev);
                }) {
                    Ok(crate::worker::DispatchOutcome::Shutdown) => return true,
                    Ok(_) => {}
                    Err(e) => tracing::warn!(error = %e, "tray dispatch failed"),
                }
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
) {
    use global_hotkey::{GlobalHotKeyEvent, HotKeyState};

    let rx = GlobalHotKeyEvent::receiver();
    while let Ok(event) = rx.try_recv() {
        if event.state != HotKeyState::Pressed {
            continue;
        }
        if let Some(request) = registry.lookup(event.id) {
            if let Err(e) =
                crate::worker::dispatch(request, &crate::worker::CommandSpawner, &mut |ev| {
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
fn run_tokio_side(tx: mpsc::Sender<()>, bus: crate::ipc_server::SubscriberBus) {
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
        // Bind the IPC server inside the reactor, sharing the daemon's
        // bus so tray/hotkey-initiated captures reach IPC subscribers.
        let (serve_handle, shutdown) = match crate::ipc_server::IpcServer::bind_with_bus(None, bus)
        {
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
}
