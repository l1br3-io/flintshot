//! Tray-helper process: the SNI registration lives here, not in the
//! daemon (ADR-0042).
//!
//! Why a separate process: COSMIC's status area never closes an SNI
//! menu popup on activation (upstream `TODO: Close menu?`) and the
//! only event that destroys an open popup is the item's **process
//! leaving the bus** (`Event::Unregistered`). A living process cannot
//! unregister — `libappindicator`'s wrapper never unrefs the
//! `GObject`
//! — so exiting is the one causal lever. The helper therefore:
//!
//! - owns the tray icon + menu for the daemon's whole session
//!   (spawned + supervised by the daemon's `TraySupervisor`);
//! - forwards menu clicks to the daemon over IPC like any client;
//! - **exits on capture/quit clicks** — its exit unregisters the SNI
//!   item, the panel destroys the popup, and the daemon's grab (gated
//!   on the ADR-0036/0037 handshake graces) lands on a popup-free
//!   frame. The daemon respawns the helper after the grab.
//! - exits when the daemon dies (a subscribe watchdog watches the
//!   event stream end), so no stale icon outlives the daemon.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use color_eyre::eyre;
use flintshot_core::ipc::Request;
use tray_icon::menu::MenuEvent;

use crate::tray::TrayAction;

/// How long the helper's event loop sleeps between drains. Menu
/// events buffer in muda's crossbeam channel, so this only bounds
/// click-to-forward latency, not correctness.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Run the tray helper until it should exit (capture/quit click, or
/// the daemon going away). Returns `Ok` on every planned exit path —
/// the supervisor's reaction to an exit is the same either way.
///
/// # Errors
/// - The tray icon cannot be built (no watcher / no session bus) —
///   the daemon logs it and retries after its hold window.
pub(crate) fn run() -> eyre::Result<()> {
    // Startup barrier first (same order the daemon used pre-ADR-0042):
    // the watcher must exist before the first registration attempt.
    #[cfg(all(feature = "tray", target_os = "linux"))]
    crate::sni_watchdog::ensure_watcher();
    #[cfg(all(feature = "tray", target_os = "linux"))]
    let watchdog = crate::sni_watchdog::spawn();

    // Held (underscore-named) for `run`'s remainder: the registration
    // lives exactly as long as this process — ADR-0039/0042. No
    // session flavor since the settings swap: the menu is identical
    // on every session (the Wayland-only bind item moved to the
    // settings page).
    let _icon = crate::tray::build()?;

    // Daemon liveness: a Subscribe connection whose stream end means
    // the daemon is gone → exit so the SNI item follows it.
    let daemon_gone = Arc::new(AtomicBool::new(false));
    spawn_daemon_watch(&daemon_gone);

    tracing::info!("tray helper running");
    let result = event_loop(&daemon_gone);

    // Orderly watchdog stop (bounded in-flight poke); the process exit
    // right after is the point — it unregisters the SNI item.
    #[cfg(all(feature = "tray", target_os = "linux"))]
    if let Some(watchdog) = watchdog {
        watchdog.shutdown();
    }

    result
}

/// The pump/drain loop; the caller's `icon` binding outlives it (the
/// registration drops with the process exit right after — ADR-0039/
/// ADR-0042). Every exit path is planned: a capture/quit click, or
/// the daemon going away.
fn event_loop(daemon_gone: &AtomicBool) -> eyre::Result<()> {
    loop {
        #[cfg(target_os = "linux")]
        pump_glib_context();

        if daemon_gone.load(Ordering::Acquire) {
            tracing::info!("daemon gone; tray helper exiting");
            return Ok(());
        }

        let rx = MenuEvent::receiver();
        while let Ok(event) = rx.try_recv() {
            if let Some(TrayAction::Request(request)) = crate::tray::action_for(&event) {
                forward(&request);
                if exits_after_action(&request) {
                    // The causal close: this process's exit
                    // unregisters the item; the panel destroys
                    // the open popup; the daemon's handshake
                    // graces cover the chain before the grab.
                    tracing::info!(?request, "tray helper exiting for action");
                    return Ok(());
                }
            }
        }

        std::thread::sleep(POLL_INTERVAL);
    }
}

/// Fire-and-forget a menu-derived request to the daemon. Failure is
/// logged, never fatal — the daemon-gone watch ends the helper when
/// there is really no daemon left.
fn forward(request: &Request) {
    match crate::ipc_client::try_delegate(request) {
        Ok(crate::ipc_client::DelegateOutcome::Delegated) => {}
        Ok(crate::ipc_client::DelegateOutcome::NoDaemon) => {
            tracing::warn!("no daemon answered a tray click; dropping it");
        }
        Err(e) => tracing::warn!(error = %e, "forwarding tray click"),
    }
}

/// Should the helper exit after forwarding this request? Captures:
/// always — the exit removes the popup the click left open
/// (ADR-0042). `Quit`: the daemon is going away; follow it promptly
/// instead of waiting for the stream-watch to notice.
fn exits_after_action(request: &Request) -> bool {
    crate::worker::spawns_capture_worker(request) || matches!(request, Request::Quit)
}

/// How long the daemon watch retries its connect before declaring
/// the daemon gone. Covers the daemon's own startup race: the
/// supervisor may spawn this helper before the daemon's IPC socket is
/// bound — and an IPC-less daemon ("continuing without it") never
/// binds one at all. A one-shot connect turned both into helper
/// respawn churn; a bounded window plus the supervisor's backoff
/// (see `daemon.rs`) contains it.
const WATCH_CONNECT_WINDOW: Duration = Duration::from_secs(2);
/// Retry cadence inside [`WATCH_CONNECT_WINDOW`].
const WATCH_CONNECT_INTERVAL: Duration = Duration::from_millis(250);

/// Watch the daemon's event stream; set `daemon_gone` when it ends.
/// A subscribe connection is the daemon's death signal by
/// construction: the socket EOFs the moment the daemon process exits
/// (or its IPC accept loop drops). Connects retry inside
/// [`WATCH_CONNECT_WINDOW`] (the startup race); once connected, any
/// stream end marks gone immediately — daemon death stays prompt.
async fn watch_daemon(path: &std::path::Path, gone: Arc<AtomicBool>) {
    use crate::ipc_server::{encode_msg, read_frame, socket_name, write_frame, Stream};
    use interprocess::local_socket::traits::tokio::Stream as _;

    let mark_gone = || gone.store(true, Ordering::Release);
    let deadline = tokio::time::Instant::now() + WATCH_CONNECT_WINDOW;
    loop {
        if path.exists() {
            if let Ok(name) = socket_name(path) {
                if let Ok(mut stream) = Stream::connect(name).await {
                    let Ok(bytes) = encode_msg(&Request::Subscribe) else {
                        mark_gone();
                        return;
                    };
                    if write_frame(&mut stream, &bytes).await.is_err() {
                        mark_gone();
                        return;
                    }
                    // Events themselves are ignored — only the
                    // stream's end matters.
                    while read_frame(&mut stream).await.is_ok() {}
                    mark_gone();
                    return;
                }
            }
        }
        if tokio::time::Instant::now() >= deadline {
            mark_gone();
            return;
        }
        tokio::time::sleep(WATCH_CONNECT_INTERVAL).await;
    }
}

/// Watch the daemon's event stream; set `daemon_gone` when it ends.
/// Best-effort — a failure to even connect also flags `gone` after
/// the bounded [`WATCH_CONNECT_WINDOW`] retry, and the supervisor's
/// respawn cycle recovers (the daemon may simply not be listening
/// *yet* during its own startup).
fn spawn_daemon_watch(daemon_gone: &Arc<AtomicBool>) {
    let gone = Arc::clone(daemon_gone);
    let _ = std::thread::Builder::new()
        .name("flintshot-tray-watch".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::warn!(error = %e, "tray watch runtime; assuming daemon gone");
                    gone.store(true, Ordering::Release);
                    return;
                }
            };
            let path = flintshot_core::ipc::socket_path(None);
            runtime.block_on(watch_daemon(&path, gone));
        });
}

/// Drain the process-default `GLib` main context: up to
/// [`MAX_GLIB_PUMP_ITERATIONS`] non-blocking `iteration(false)` calls,
/// stopping early once nothing is pending.
///
/// Why: on Linux the tray stack (libappindicator behind `tray-icon`,
/// plus muda's menu plumbing) drives from `GLib` sources on the
/// default context — `RegisterStatusNotifierItem` and menu-event
/// delivery only happen while that context is iterated. The helper
/// owns no `GLib` main loop, so it pumps (ADR-0031).
#[cfg(target_os = "linux")]
fn pump_glib_context() {
    let context = gtk::glib::MainContext::default();
    for _ in 0..MAX_GLIB_PUMP_ITERATIONS {
        if !context.iteration(false) {
            break;
        }
    }
}

/// Hard cap on `GLib` iterations per [`pump_glib_context`] call. Bounds
/// the worst case — a pathological always-ready source — so one pump
/// can never wedge the helper loop; leftover work spreads over later
/// ticks.
#[cfg(target_os = "linux")]
const MAX_GLIB_PUMP_ITERATIONS: usize = 64;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_and_quit_clicks_exit_the_helper() {
        // The exit is the point: it unregisters the SNI item, which is
        // the only event that closes the panel's open popup
        // (ADR-0042).
        let exiting = [Request::Capture { delay_ms: 0 }, Request::Quit];
        for request in &exiting {
            assert!(exits_after_action(request), "{request:?} must exit");
        }
    }

    /// Other clicks keep the helper alive: Open/upload still stream
    /// status through the daemon; the settings window opens through
    /// the daemon's broadcast + spawn. Nothing here needs the item
    /// unregistered.
    #[test]
    fn other_clicks_keep_the_helper_alive() {
        // Open/upload still stream status through the daemon; the
        // settings window opens through the daemon's broadcast +
        // spawn. Nothing here needs the item unregistered.
        let staying = [
            Request::OpenLast,
            Request::UploadLast {
                backend: "imgur".into(),
            },
            Request::BindShortcuts,
            // settings-improvement: the Settings click is the OpenApp
            // choreography — the helper stays alive (pinned like
            // `OpenApp`'s implicit case).
            Request::OpenApp,
            Request::OpenSettings,
            // tray-show-hide-toggle: the toggle never grabs the
            // screen in either direction — no popup to close, the
            // helper stays.
            Request::ToggleWindow,
        ];
        for request in &staying {
            assert!(!exits_after_action(request), "{request:?} must stay");
        }
    }

    /// [`super::watch_daemon`] — the daemon-watch core against real
    /// local sockets (headless-safe; local sockets need no display).
    mod watch {
        use std::path::PathBuf;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use std::time::Duration;

        use super::super::{watch_daemon, WATCH_CONNECT_INTERVAL, WATCH_CONNECT_WINDOW};

        /// A listener appearing mid-window (the daemon-startup race:
        /// the supervisor may spawn this helper before the daemon's
        /// IPC socket is bound) is accepted — the watch stays alive
        /// through the failed attempts and marks gone only once the
        /// connected stream actually ends.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn late_listener_is_accepted_within_the_window() {
            use interprocess::local_socket::traits::tokio::Listener as _;

            let dir = tempfile::tempdir().unwrap();
            let sock = dir.path().join("flintshot-watch-late.sock");
            let gone = Arc::new(AtomicBool::new(false));

            let watch_gone = Arc::clone(&gone);
            let watch_path: PathBuf = sock.clone();
            let watch = tokio::spawn(async move {
                watch_daemon(&watch_path, watch_gone).await;
            });

            // A few failed connect attempts pass (no socket yet),
            // then the daemon's listener appears — inside the window.
            tokio::time::sleep(WATCH_CONNECT_INTERVAL * 2).await;
            let name = crate::ipc_server::socket_name(&sock).unwrap();
            let listener = interprocess::local_socket::ListenerOptions::new()
                .name(name)
                .try_overwrite(true)
                .create_tokio()
                .unwrap();
            let conn = listener.accept().await.unwrap();

            // The watch connected and parked on its read: gone must
            // not be set even though the first attempts failed.
            tokio::time::sleep(WATCH_CONNECT_INTERVAL * 2).await;
            assert!(
                !gone.load(Ordering::SeqCst),
                "the watch accepted the late-appearing daemon"
            );

            // Stream end (the daemon dying) marks gone promptly.
            drop(conn);
            drop(listener);
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while !gone.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            assert!(
                gone.load(Ordering::SeqCst),
                "stream end after a connected life marks the daemon gone"
            );
            let _ = watch.await;
        }

        /// A daemon that never answers within the window is declared
        /// gone — bounded, no unbounded retry.
        #[tokio::test]
        async fn absent_daemon_marks_gone_after_the_window() {
            let dir = tempfile::tempdir().unwrap();
            let sock = dir.path().join("flintshot-watch-absent.sock");
            let gone = Arc::new(AtomicBool::new(false));
            let started = std::time::Instant::now();
            watch_daemon(&sock, Arc::clone(&gone)).await;
            assert!(gone.load(Ordering::SeqCst), "no daemon → gone");
            assert!(
                started.elapsed() >= WATCH_CONNECT_WINDOW,
                "the window bounded the retry ({} attempts' worth)",
                started.elapsed().as_millis()
            );
            assert!(
                started.elapsed() < WATCH_CONNECT_WINDOW + Duration::from_secs(2),
                "the watch must give up shortly past the window"
            );
        }
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
        use std::sync::{Mutex, MutexGuard};
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
                std::thread::sleep(Duration::from_millis(5));
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
