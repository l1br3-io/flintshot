//! Daemon-side IPC server: accepts connections, decodes [`Request`]s,
//! and fans [`Event`]s to subscribers.
//!
//! The transport (framing, socket helpers, client helpers) now lives in
//! the `flintshot-ipc` workspace crate so the daemon and the COSMIC
//! applet share one byte-identical wire-format implementation; this
//! module owns only the daemon-side state — [`SubscriberBus`],
//! [`IpcServer`], [`handle_connection`], `decode_request`, and
//! `wait_for_socket_to_free`. See ADR-0025.
//!
//! The server runs on the daemon's tokio side thread (ADR-0018): it
//! accepts connections via the re-exported transport primitives,
//! dispatches [`Request`]s, and fans [`Event`]s out to subscribers via
//! [`SubscriberBus`].
//!
//! [`Request`]: flintshot_core::ipc::Request
//! [`Event`]: flintshot_core::ipc::Event

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use color_eyre::eyre::{self, WrapErr};
use flintshot_core::ipc::{socket_path, Event as IpcEvent, Request as IpcRequest};
// The tokio Listener trait brings `accept` into scope (Stream's
// `connect` is used via a fully-qualified path in the integration test).
use interprocess::local_socket::traits::tokio::Listener as _;
// Transport primitives (framing + socket helpers) live in the shared
// `flintshot-ipc` crate; this module owns only the daemon-side server.
use tokio::sync::{broadcast, Notify};

use flintshot_ipc::{encode_msg, read_frame, socket_name, write_frame, Stream};

/// Per-subscriber broadcast buffer. A subscriber slower than this gets a
/// `Lagged` error (logged + skipped) instead of unbounded buffering.
const EVENT_BUS_CAPACITY: usize = 64;

/// Postcard-decode a [`Request`].
fn decode_request(buf: &[u8]) -> eyre::Result<IpcRequest> {
    postcard::from_bytes(buf).map_err(|e| eyre::eyre!("postcard decode request: {e}"))
}

// ── subscriber fan-out ──────────────────────────────────────────────

/// Cloneable fan-out handle for daemon → subscriber [`Event`]s. The
/// daemon (Task 006) holds one to broadcast capture outcomes; each
/// `Request::Subscribe` connection gets its own receiver off it.
#[derive(Clone)]
pub struct SubscriberBus {
    tx: broadcast::Sender<IpcEvent>,
}

impl SubscriberBus {
    /// Create a fresh bus with the default per-subscriber buffer.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(EVENT_BUS_CAPACITY)
    }

    /// Create a bus with an explicit per-subscriber buffer capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        let (tx, _receiver) = broadcast::channel(capacity.max(1));
        Self { tx }
    }

    /// Broadcast `event` to every current subscriber. No subscribers →
    /// silent no-op. A subscriber that has gone away is reaped
    /// automatically by the broadcast channel.
    pub fn broadcast(&self, event: IpcEvent) {
        let _ = self.tx.send(event);
    }

    /// Subscribe, returning a receiver of the [`Event`] stream.
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<IpcEvent> {
        self.tx.subscribe()
    }
}

impl Default for SubscriberBus {
    fn default() -> Self {
        Self::new()
    }
}

// ── server ───────────────────────────────────────────────────────────

/// The tokio local-socket listener type, aliased to keep the cfg noise
/// out of the struct body.
type Listener = interprocess::local_socket::tokio::Listener;

/// The IPC server: owns the listener, shared daemon state (subscriber
/// bus + `last_saved`), and a shutdown signal.
pub struct IpcServer {
    listener: Listener,
    state: crate::daemon_state::DaemonState,
    shutdown: Arc<Notify>,
}

impl IpcServer {
    /// Bind a listener on [`socket_path`] (`override_path` honoured),
    /// reclaiming any stale socket file on Unix via `try_overwrite`,
    /// with fresh daemon state.
    ///
    /// # Errors
    /// - If the socket name can't be resolved.
    /// - If binding the listener fails (e.g. a live socket already
    ///   holds the path and can't be replaced).
    ///
    // The daemon uses `bind_with_state` (it shares state with the
    // tray/hotkey path); this convenience is used by the integration
    // tests.
    #[allow(dead_code)]
    pub fn bind(override_path: Option<&Path>) -> eyre::Result<Self> {
        Self::bind_with_state(override_path, crate::daemon_state::DaemonState::new())
    }

    /// Like [`bind`](Self::bind) but reuses an externally-provided
    /// [`SubscriberBus`]. The bus is wrapped in fresh
    /// [`DaemonState`]; prefer [`bind_with_state`](Self::bind_with_state)
    /// for new call sites that want to share `last_saved` too.
    //
    // The daemon now uses `bind_with_state` directly (it shares the
    // whole `DaemonState`); this convenience is retained for callers
    // (and tests) that only have a bus.
    #[allow(dead_code)]
    pub fn bind_with_bus(override_path: Option<&Path>, bus: SubscriberBus) -> eyre::Result<Self> {
        Self::bind_with_state(
            override_path,
            crate::daemon_state::DaemonState::with_bus(bus),
        )
    }

    /// Like [`bind`](Self::bind) but uses externally-provided
    /// [`DaemonState`]. The daemon shares one state struct between the
    /// IPC server (which receives `ReportSaved` and fans `Event`s to
    /// subscribers) and its own tray/hotkey dispatch path, so a capture
    /// started from the tray reaches IPC subscribers and the
    /// `last_saved` path is consistent across readers.
    pub fn bind_with_state(
        override_path: Option<&Path>,
        state: crate::daemon_state::DaemonState,
    ) -> eyre::Result<Self> {
        let path = socket_path(override_path);
        let name = socket_name(&path)?;
        let listener = interprocess::local_socket::ListenerOptions::new()
            .name(name)
            .try_overwrite(true)
            .create_tokio()
            .wrap_err_with(|| format!("binding IPC listener at {}", path.display()))?;
        tracing::info!(path = %path.display(), "IPC server bound");
        Ok(Self {
            listener,
            state,
            shutdown: Arc::new(Notify::new()),
        })
    }

    /// A cloneable handle to broadcast [`Event`]s to subscribers. The
    /// daemon keeps one and calls [`SubscriberBus::broadcast`] as
    /// capture outcomes arrive.
    #[allow(dead_code)]
    pub fn bus(&self) -> SubscriberBus {
        self.state.bus.clone()
    }

    /// A cloneable handle to the shared daemon state (subscriber bus +
    /// `last_saved`). Used by integration tests to assert state changes
    /// after a round-trip.
    #[allow(dead_code)]
    pub fn state(&self) -> crate::daemon_state::DaemonState {
        self.state.clone()
    }

    /// The daemon-wide shutdown signal. `Request::Quit` (via
    /// `--replace`) fires it so the IPC accept loop, the heartbeat
    /// ticker, and thus the daemon's main loop all stop together.
    #[must_use]
    pub fn shutdown_signal(&self) -> Arc<Notify> {
        Arc::clone(&self.shutdown)
    }

    /// Run the accept loop until shutdown is signalled (`Request::Quit`
    /// via a connection, or the caller dropping this future). Each
    /// accepted connection is handled in its own task.
    ///
    /// # Errors
    /// Propagates a fatal accept-loop error (these are currently
    /// logged-and-continued per-connection, so a returned error would
    /// indicate the listener itself broke).
    pub async fn serve(self) -> eyre::Result<()> {
        let shutdown = Arc::clone(&self.shutdown);
        let state = self.state.clone();
        let listener = self.listener;
        loop {
            tokio::select! {
                biased;
                () = shutdown.notified() => {
                    tracing::info!("IPC server shutting down");
                    break;
                }
                res = listener.accept() => {
                    let stream = match res {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::warn!(error = %e, "IPC accept failed; continuing");
                            continue;
                        }
                    };
                    let state = state.clone();
                    let shutdown = Arc::clone(&shutdown);
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, state, shutdown).await {
                            tracing::debug!(error = %e, "IPC connection ended");
                        }
                    });
                }
            }
        }
        Ok(())
    }
}

/// Handle one connection: read framed [`Request`]s until the client
/// disconnects. `Subscribe` converts the connection into a pure
/// [`Event`] sink; `Quit` signals daemon shutdown; `ReportSaved`
/// (worker → daemon feedback) stores the path + broadcasts
/// [`Event::Saved`]; other requests route through the worker dispatch
/// table (capture requests spawn a `flintshot` child; unbuilt phases
/// broadcast an `unsupported` Error).
async fn handle_connection(
    mut stream: Stream,
    state: crate::daemon_state::DaemonState,
    shutdown: Arc<Notify>,
) -> eyre::Result<()> {
    loop {
        let payload = read_frame(&mut stream).await?;
        let request = decode_request(&payload)?;

        // Worker → daemon outcome reporting: store the save path and
        // broadcast Event::Saved. Fire-and-forget — no response is sent
        // (matching Subscribe's no-response shape). Intercepted before
        // the dispatch table so it never reaches the "unsupported" arm.
        if let Some(path) = crate::daemon_state::extract_report_saved_path(&request) {
            state.record_save(path.to_path_buf());
            continue;
        }

        if matches!(request, IpcRequest::Subscribe) {
            tracing::debug!("IPC subscriber registered");
            // Subscribe synchronously — the receiver is registered
            // the moment Subscribe is read, before the forwarder
            // task is scheduled, so a client that sends Subscribe
            // then waits for events doesn't race the task spawn.
            let rx = state.bus.subscribe();
            tokio::spawn(async move {
                let mut rx = rx;
                let mut stream = stream;
                loop {
                    match rx.recv().await {
                        Ok(event) => {
                            let bytes = match encode_msg(&event) {
                                Ok(b) => b,
                                Err(e) => {
                                    tracing::warn!(error = %e, "encode event failed");
                                    continue;
                                }
                            };
                            if write_frame(&mut stream, &bytes).await.is_err() {
                                break; // subscriber gone
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!(skipped = n, "IPC subscriber lagged; skipping events");
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            });
            return Ok(());
        }

        // Everything else routes through the worker dispatch table:
        // capture requests spawn a `flintshot` child + broadcast
        // CaptureStarted; PinLast spawns a pin worker; Quit signals
        // shutdown; unbuilt phases broadcast an `unsupported` Error.
        let outcome = crate::worker::dispatch(&request, &crate::worker::CommandSpawner, &state)?;
        if matches!(outcome, crate::worker::DispatchOutcome::Shutdown) {
            tracing::info!("IPC Request::Quit received; signalling daemon shutdown");
            // Wake every waiter (the IPC accept loop AND the daemon's
            // heartbeat ticker) so the whole daemon shuts down — this
            // is the `--replace` takeover path.
            shutdown.notify_waiters();
            return Ok(());
        }
    }
}

/// Poll until the IPC socket file is gone (the old daemon released it),
/// up to `timeout`. Used by `--replace` to wait for the previous daemon
/// to exit before taking the single-instance lock.
pub(crate) fn wait_for_socket_to_free(
    override_path: Option<&Path>,
    timeout: Duration,
) -> eyre::Result<()> {
    let path = socket_path(override_path);
    let deadline = std::time::Instant::now() + timeout;
    while path.exists() {
        if std::time::Instant::now() >= deadline {
            return Err(eyre::eyre!(
                "timed out after {timeout:?} waiting for {} to be released by the old daemon",
                path.display()
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flintshot_core::ipc::{Event, Request};
    use flintshot_ipc::send_request;

    #[tokio::test]
    async fn broadcast_fans_out_to_all_subscribers() {
        let bus = SubscriberBus::new();
        let mut r1 = bus.subscribe();
        let mut r2 = bus.subscribe();
        let ev = Event::CaptureStarted;
        bus.broadcast(ev.clone());

        assert_eq!(r1.recv().await.unwrap(), ev);
        assert_eq!(r2.recv().await.unwrap(), ev);
    }

    #[tokio::test]
    async fn dropped_subscriber_does_not_break_others() {
        let bus = SubscriberBus::new();
        let mut alive = bus.subscribe();
        {
            let _doomed = bus.subscribe();
            // `_doomed` drops here.
        }
        let ev = Event::EditorOpened;
        bus.broadcast(ev.clone());
        // The alive receiver still gets the event; broadcast to a
        // dropped receiver didn't error (broadcast::send ignores it).
        assert_eq!(alive.recv().await.unwrap(), ev);
    }

    /// Real-socket end-to-end: server binds a temp socket, a client
    /// connects, sends `Subscribe`, the daemon broadcasts an `Event`,
    /// and the client decodes it. `#[ignore]` because it needs a real
    /// local socket (works headless; mirrored after the Phase 1 capture
    /// smoke-test convention). Run with:
    /// `cargo test -p flintshot-app -- --ignored ipc_server`
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "needs a real local socket (no display); run explicitly"]
    async fn integration_subscribe_receives_broadcast_event() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static UNIQUE: AtomicU64 = AtomicU64::new(0);

        let dir = tempfile::tempdir().unwrap();
        let n = UNIQUE.fetch_add(1, Ordering::Relaxed);
        let sock = dir.path().join(format!("flintshot-ipc-{n}.sock"));

        let server = IpcServer::bind(Some(&sock)).unwrap();
        let bus = server.bus();
        let server_task = tokio::spawn(async move { server.serve().await });

        // Connect a client and send Subscribe.
        let name = socket_name(&sock).unwrap();
        let mut client =
            <Stream as interprocess::local_socket::traits::tokio::Stream>::connect(name)
                .await
                .unwrap();
        let sub = Request::Subscribe;
        write_frame(&mut client, &encode_msg(&sub).unwrap())
            .await
            .unwrap();

        // The server must read Subscribe + register the subscription
        // before a broadcast will reach it. Retry the broadcast (with a
        // read timeout) until the event arrives — this sidesteps the
        // inherent subscribe/broadcast race without adding an ACK to
        // the protocol.
        let event = Event::CaptureStarted;
        let payload = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                bus.broadcast(event.clone());
                if let Ok(Ok(payload)) = tokio::time::timeout(
                    std::time::Duration::from_millis(80),
                    read_frame(&mut client),
                )
                .await
                {
                    return payload;
                }
            }
        })
        .await
        .expect("timed out waiting for the broadcast event");

        let decoded: Event = postcard::from_bytes(&payload).unwrap();
        assert_eq!(decoded, event);

        drop(client);
        server_task.abort();
    }

    /// Worker → daemon feedback over a real socket: a subscriber
    /// connects, then a worker sends `Request::ReportSaved`, and the
    /// subscriber must receive `Event::Saved { path }` (and the daemon
    /// must record the path in `last_saved`). `#[ignore]` (needs a real
    /// local socket). Run with:
    /// `cargo test -p flintshot-app -- --ignored ipc_server`
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "needs a real local socket (no display); run explicitly"]
    async fn integration_report_saved_broadcasts_event_and_records_path() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static UNIQUE: AtomicU64 = AtomicU64::new(0);

        let dir = tempfile::tempdir().unwrap();
        let n = UNIQUE.fetch_add(1, Ordering::Relaxed);
        let sock = dir.path().join(format!("flintshot-report-{n}.sock"));

        let server = IpcServer::bind(Some(&sock)).unwrap();
        let state = server.state();
        let server_task = tokio::spawn(async move { server.serve().await });

        // 1. Subscribe first so the later broadcast reaches us.
        let name = socket_name(&sock).unwrap();
        let mut subscriber =
            <Stream as interprocess::local_socket::traits::tokio::Stream>::connect(name)
                .await
                .unwrap();
        let sub = Request::Subscribe;
        write_frame(&mut subscriber, &encode_msg(&sub).unwrap())
            .await
            .unwrap();

        // Retry until the subscribe has registered (mirrors the
        // subscribe integration test's race-avoidance).
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if read_frame(&mut subscriber).await.is_ok() {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .ok();

        // 2. Open a second connection (the "worker") and send
        //    ReportSaved. Fire-and-forget: close immediately after.
        let report = Request::ReportSaved {
            path: std::path::PathBuf::from("/tmp/flintshot-report.png"),
        };
        send_request(Some(&sock), &report)
            .await
            .expect("sending ReportSaved");

        // 3. The subscriber must receive Event::Saved with the same path.
        let payload = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            read_frame(&mut subscriber),
        )
        .await
        .expect("timed out waiting for Event::Saved")
        .expect("read failed");
        let decoded: Event = postcard::from_bytes(&payload).unwrap();
        assert_eq!(
            decoded,
            Event::Saved {
                path: std::path::PathBuf::from("/tmp/flintshot-report.png"),
            }
        );

        // 4. The daemon state must have recorded the path too.
        //    Retry briefly — the broadcast and the store happen in the
        //    same handler call, but the test observer races the spawn.
        let recorded = tokio::time::timeout(std::time::Duration::from_secs(1), async {
            loop {
                if state.last_saved().is_some() {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .is_ok();
        assert!(recorded, "last_saved was never recorded");
        assert_eq!(
            state.last_saved(),
            Some(std::path::PathBuf::from("/tmp/flintshot-report.png"))
        );

        drop(subscriber);
        server_task.abort();
    }

    #[test]
    fn wait_for_socket_to_free_returns_immediately_when_absent() {
        // A path that doesn't exist is already "free" — no waiting.
        let bogus = std::path::Path::new("/this/does/not/exist/flintshot-test.sock");
        wait_for_socket_to_free(Some(bogus), std::time::Duration::from_secs(1))
            .expect("absent path is immediately free");
    }

    /// The `--replace` core mechanism on a real socket: a client sends
    /// `Request::Quit`, the server's shutdown signal fires, and the
    /// socket file is reclaimed. `#[ignore]` (needs a real local
    /// socket). Run with:
    /// `cargo test -p flintshot-app -- --ignored ipc_server`
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "needs a real local socket (no display); run explicitly"]
    async fn integration_quit_triggers_shutdown_and_frees_socket() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static UNIQUE: AtomicU64 = AtomicU64::new(0);

        let dir = tempfile::tempdir().unwrap();
        let n = UNIQUE.fetch_add(1, Ordering::Relaxed);
        let sock = dir.path().join(format!("flintshot-quit-{n}.sock"));

        let server = IpcServer::bind(Some(&sock)).unwrap();
        let shutdown = server.shutdown_signal();
        let server_task = tokio::spawn(async move { server.serve().await });

        // Send Quit. A failure to connect (e.g. the listener not yet
        // ready) is retried briefly.
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                match send_request(Some(&sock), &Request::Quit).await {
                    Ok(()) => return,
                    Err(_) => tokio::time::sleep(std::time::Duration::from_millis(20)).await,
                }
            }
        })
        .await
        .expect("timed out sending Quit");

        // The shutdown signal must fire.
        tokio::time::timeout(std::time::Duration::from_secs(2), shutdown.notified())
            .await
            .expect("shutdown signal did not fire after Quit");

        // Wait for the serve loop to end and reclaim the socket file.
        let freed = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while sock.exists() {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .is_ok();
        assert!(freed, "socket file was not reclaimed after Quit");

        server_task.abort();
    }
}
