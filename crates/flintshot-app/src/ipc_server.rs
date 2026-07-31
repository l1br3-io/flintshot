//! IPC server transport: `interprocess` local socket + length-prefixed
//! postcard framing.
//!
//! Schema ([`Request`]/[`Event`]) comes from `flintshot-core::ipc`
//! (Task 1); everything transport/encoding stays here in
//! `flintshot-app` so no `interprocess`/`postcard` types leak into
//! core. The server runs on the daemon's tokio side thread (ADR-0018):
//! it accepts connections, reads u32-little-endian-length-prefixed
//! postcard frames, dispatches [`Request`]s, and fans [`Event`]s out to
//! subscribers via [`SubscriberBus`].
//!
//! Framing detail (u32 LE length, 1 MiB cap) extends ADR-0011; not
//! divergent enough to warrant its own ADR.
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
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{broadcast, Notify};

/// Maximum payload size accepted on the wire (1 MiB). Guards against a
/// peer that claims a huge length prefix.
pub const MAX_FRAME_LEN: usize = 1 << 20;
/// Per-subscriber broadcast buffer. A subscriber slower than this gets a
/// `Lagged` error (logged + skipped) instead of unbounded buffering.
const EVENT_BUS_CAPACITY: usize = 64;

// ── framing ──────────────────────────────────────────────────────────

/// Read one length-prefixed frame: 4-byte little-endian length, then
/// exactly that many payload bytes. Rejects lengths over
/// [`MAX_FRAME_LEN`]. A short read (truncation / clean disconnect)
/// surfaces as an error, never a panic.
async fn read_frame<R>(r: &mut R) -> eyre::Result<Vec<u8>>
where
    R: AsyncRead + Unpin,
{
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf)
        .await
        .map_err(|e| eyre::eyre!("reading frame length: {e}"))?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_FRAME_LEN {
        return Err(eyre::eyre!(
            "frame too large: {len} bytes (max {MAX_FRAME_LEN})"
        ));
    }
    let mut payload = vec![0u8; len];
    r.read_exact(&mut payload)
        .await
        .map_err(|e| eyre::eyre!("reading frame payload: {e}"))?;
    Ok(payload)
}

/// Write one length-prefixed frame. Rejects payloads over
/// [`MAX_FRAME_LEN`] before touching the socket.
pub(crate) async fn write_frame<W>(w: &mut W, payload: &[u8]) -> eyre::Result<()>
where
    W: AsyncWrite + Unpin,
{
    if payload.len() > MAX_FRAME_LEN {
        return Err(eyre::eyre!(
            "payload too large: {} bytes (max {MAX_FRAME_LEN})",
            payload.len()
        ));
    }
    let len = u32::try_from(payload.len())
        .map_err(|_| eyre::eyre!("payload length {} overflows u32", payload.len()))?;
    w.write_all(&len.to_le_bytes())
        .await
        .map_err(|e| eyre::eyre!("writing frame length: {e}"))?;
    w.write_all(payload)
        .await
        .map_err(|e| eyre::eyre!("writing frame payload: {e}"))?;
    w.flush()
        .await
        .map_err(|e| eyre::eyre!("flushing frame: {e}"))?;
    Ok(())
}

/// Postcard-encode `msg` to owned bytes.
pub(crate) fn encode_msg<T>(msg: &T) -> eyre::Result<Vec<u8>>
where
    T: serde::Serialize + ?Sized,
{
    postcard::to_allocvec(msg).map_err(|e| eyre::eyre!("postcard encode: {e}"))
}

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
    fn subscribe(&self) -> broadcast::Receiver<IpcEvent> {
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
/// The tokio local-socket stream type.
pub(crate) type Stream = interprocess::local_socket::tokio::Stream;

/// The IPC server: owns the listener, the subscriber bus, and a
/// shutdown signal.
pub struct IpcServer {
    listener: Listener,
    bus: SubscriberBus,
    shutdown: Arc<Notify>,
}

impl IpcServer {
    /// Bind a listener on [`socket_path`] (`override_path` honoured),
    /// reclaiming any stale socket file on Unix via `try_overwrite`,
    /// with a fresh subscriber bus.
    ///
    /// # Errors
    /// - If the socket name can't be resolved.
    /// - If binding the listener fails (e.g. a live socket already
    ///   holds the path and can't be replaced).
    //
    // The daemon uses `bind_with_bus` (it shares a bus with the tray
    // path); this convenience is used by the integration tests.
    #[allow(dead_code)]
    pub fn bind(override_path: Option<&Path>) -> eyre::Result<Self> {
        Self::bind_with_bus(override_path, SubscriberBus::new())
    }

    /// Like [`bind`](Self::bind) but uses an externally-provided
    /// `bus`. The daemon shares one bus between the IPC server (which
    /// fans `Event`s to IPC subscribers) and its own tray/hotkey
    /// dispatch path, so a capture started from the tray reaches IPC
    /// subscribers and vice versa.
    pub fn bind_with_bus(override_path: Option<&Path>, bus: SubscriberBus) -> eyre::Result<Self> {
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
            bus,
            shutdown: Arc::new(Notify::new()),
        })
    }

    /// A cloneable handle to broadcast [`Event`]s to subscribers. The
    /// daemon keeps one and calls [`SubscriberBus::broadcast`] as
    /// capture outcomes arrive (Task 006).
    //
    // Only the integration test uses this externally; the daemon's
    // `handle_connection` broadcasts via its own bus clone.
    #[allow(dead_code)]
    pub fn bus(&self) -> SubscriberBus {
        self.bus.clone()
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
        let bus = self.bus.clone();
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
                    let bus = bus.clone();
                    let shutdown = Arc::clone(&shutdown);
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, bus, shutdown).await {
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
/// [`Event`] sink; `Quit` signals daemon shutdown; other requests are
/// handled in Task 006.
async fn handle_connection(
    mut stream: Stream,
    bus: SubscriberBus,
    shutdown: Arc<Notify>,
) -> eyre::Result<()> {
    loop {
        let payload = read_frame(&mut stream).await?;
        let request = decode_request(&payload)?;
        if matches!(request, IpcRequest::Subscribe) {
            tracing::debug!("IPC subscriber registered");
            // Subscribe synchronously — the receiver is registered
            // the moment Subscribe is read, before the forwarder
            // task is scheduled, so a client that sends Subscribe
            // then waits for events doesn't race the task spawn.
            let rx = bus.subscribe();
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
        // CaptureStarted; Quit signals shutdown; unbuilt phases
        // broadcast an `unsupported` Error.
        let outcome =
            crate::worker::dispatch(&request, &crate::worker::CommandSpawner, &mut |event| {
                bus.broadcast(event);
            })?;
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

/// Resolve the OS socket/pipe name for `path`. Unix → a filesystem
/// socket name; Windows → a namespaced pipe name.
pub(crate) fn socket_name(path: &Path) -> eyre::Result<interprocess::local_socket::Name<'_>> {
    // The prelude pulls the ToFsName / ToNsName traits into scope (as `_`)
    // so `.to_fs_name()` / `.to_ns_name()` resolve.
    use interprocess::local_socket::tokio::prelude::*;
    #[cfg(unix)]
    {
        use interprocess::local_socket::GenericFilePath;
        path.to_fs_name::<GenericFilePath>()
            .map_err(|e| eyre::eyre!("resolving filesystem socket name: {e}"))
    }
    #[cfg(not(unix))]
    {
        use interprocess::local_socket::GenericNamespaced;
        path.to_string_lossy()
            .into_owned()
            .to_ns_name::<GenericNamespaced>()
            .map_err(|e| eyre::eyre!("resolving pipe name: {e}"))
    }
}

// ── client helpers (used by `--replace` now, detect-and-delegate in 011) ──

/// Connect to the daemon at [`socket_path`](`override_path`) and send
/// one framed `request`. No response is read (fire-and-forget); the
/// daemon's reaction arrives via its side effects (shutdown for `Quit`,
/// a spawned worker + event stream for captures).
pub(crate) async fn send_request(
    override_path: Option<&Path>,
    request: &IpcRequest,
) -> eyre::Result<()> {
    let path = socket_path(override_path);
    let name = socket_name(&path)?;
    let mut conn = <Stream as interprocess::local_socket::traits::tokio::Stream>::connect(name)
        .await
        .map_err(|e| eyre::eyre!("connecting to IPC socket {}: {e}", path.display()))?;
    let bytes = encode_msg(request)?;
    write_frame(&mut conn, &bytes).await?;
    Ok(())
}

/// Sync wrapper around [`send_request`] for callers not already in a
/// tokio runtime (the `--replace` handshake runs on the daemon's main
/// thread, before the tokio side starts).
pub(crate) fn send_request_blocking(
    override_path: Option<&Path>,
    request: &IpcRequest,
) -> eyre::Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| eyre::eyre!("building IPC client runtime: {e}"))?;
    runtime.block_on(send_request(override_path, request))
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
    use tokio::io::{duplex, AsyncWriteExt};

    #[tokio::test]
    async fn framing_roundtrips_a_request() {
        // A duplex pair gives us connected in-memory async buffers — no
        // real socket needed to exercise the framing path.
        let (mut client, mut server) = duplex(8 * 1024);
        let original = Request::CaptureRegion {
            monitor: Some(2),
            delay_ms: 500,
        };

        let bytes = encode_msg(&original).unwrap();
        write_frame(&mut client, &bytes).await.unwrap();

        let payload = read_frame(&mut server).await.unwrap();
        let decoded: Request = postcard::from_bytes(&payload).unwrap();
        assert_eq!(decoded, original);
    }

    #[tokio::test]
    async fn framing_roundtrips_an_event() {
        let (mut a, mut b) = duplex(8 * 1024);
        let original = Event::Saved {
            path: std::path::PathBuf::from("/tmp/x.png"),
        };
        write_frame(&mut a, &encode_msg(&original).unwrap())
            .await
            .unwrap();
        let payload = read_frame(&mut b).await.unwrap();
        let decoded: Event = postcard::from_bytes(&payload).unwrap();
        assert_eq!(decoded, original);
    }

    #[tokio::test]
    async fn truncated_frame_returns_error_not_panic() {
        // Write only 2 of the 4 length bytes, then close the writer.
        let (mut client, mut server) = duplex(8);
        client.write_all(&[0x01, 0x00]).await.unwrap();
        client.shutdown().await.unwrap();
        let result = read_frame(&mut server).await;
        assert!(result.is_err(), "truncated length must error, not panic");
    }

    #[tokio::test]
    async fn oversized_length_is_rejected() {
        // Hand-craft a length prefix claiming MAX_FRAME_LEN + 1 bytes.
        let (mut client, mut server) = duplex(16);
        let huge = u32::try_from(MAX_FRAME_LEN + 1).unwrap();
        client.write_all(&huge.to_le_bytes()).await.unwrap();
        // Send a couple of payload bytes so read_exact on the length
        // already happens before the payload read.
        client.write_all(&[0u8, 0u8]).await.unwrap();
        let err = read_frame(&mut server).await.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("too large"),
            "expected 'too large' in error, got: {msg}"
        );
    }

    #[tokio::test]
    async fn write_frame_rejects_oversized_payload() {
        let (mut _client, mut server) = duplex(16);
        let huge = vec![0u8; MAX_FRAME_LEN + 1];
        let err = write_frame(&mut server, &huge).await.unwrap_err();
        assert!(format!("{err}").contains("too large"));
    }

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
