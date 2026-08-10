//! COSMIC applet → daemon IPC client.
//!
//! Pure transport layer for `flintshot-cosmic`. Tasks 7–8 build the
//! panel UI on top of this module; it is testable headless (no
//! `libcosmic`) against a hand-rolled in-process listener.
//!
//! # Wire format
//!
//! Mirrors `flintshot-app::ipc_server` exactly: 4-byte little-endian
//! length prefix, postcard payload, 1 MiB cap. The message schema
//! ([`Request`]/[`Event`]) is the single source of truth in
//! `flintshot_core::ipc`.
//!
//! # Why duplicate the framing?
//!
//! `flintshot-app::ipc_server` is `pub(crate)`, and depending on
//! `flintshot-app` from the applet would pull in `egui`/`eframe`/the
//! whole CLI — violating ADR-0010 ("schema in core, transport in
//! bins") and the applet's pure-client role. So the ~30-line framing
//! is duplicated here.
//!
//! # GOTCHA: keep the two copies in sync
//!
//! If the wire format in `flintshot-app::ipc_server` changes (length
//! prefix width, endianness, frame cap, postcard variant), update this
//! copy too — they must agree byte-for-byte. Task 9 (hardening)
//! considers a cross-crate framing conformance test to catch drift.
//!
//! [`Request`]: flintshot_core::ipc::Request
//! [`Event`]: flintshot_core::ipc::Event

use std::path::Path;

use color_eyre::eyre;
use flintshot_core::ipc::{socket_path, Event as IpcEvent, Request as IpcRequest};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Maximum frame size on the wire (1 MiB). Guards against a peer that
/// claims a huge length prefix. Matches
/// `flintshot_app::ipc_server::MAX_FRAME_LEN`.
pub const MAX_FRAME_LEN: usize = 1 << 20;

/// Tokio local-socket stream, aliased to keep cfg noise out of signatures.
type Stream = interprocess::local_socket::tokio::Stream;

// ── framing (mirror of flintshot-app/src/ipc_server.rs) ──────────────

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
async fn write_frame<W>(w: &mut W, payload: &[u8]) -> eyre::Result<()>
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
fn encode_msg<T>(msg: &T) -> eyre::Result<Vec<u8>>
where
    T: serde::Serialize + ?Sized,
{
    postcard::to_allocvec(msg).map_err(|e| eyre::eyre!("postcard encode: {e}"))
}

/// Postcard-decode an [`Event`].
///
/// [`Event`]: flintshot_core::ipc::Event
fn decode_event(buf: &[u8]) -> eyre::Result<IpcEvent> {
    postcard::from_bytes(buf).map_err(|e| eyre::eyre!("postcard decode event: {e}"))
}

/// Resolve a socket path to an interprocess `Name`. Mirrors
/// `flintshot_app::ipc_server::socket_name`: Unix → filesystem socket
/// name; Windows → namespaced pipe name.
fn socket_name(path: &Path) -> eyre::Result<interprocess::local_socket::Name<'_>> {
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

/// Connect to the daemon socket at `override_path` (or the platform
/// default when `None`).
async fn connect(override_path: Option<&Path>) -> eyre::Result<Stream> {
    let path = socket_path(override_path);
    let name = socket_name(&path)?;
    <Stream as interprocess::local_socket::traits::tokio::Stream>::connect(name)
        .await
        .map_err(|e| eyre::eyre!("connecting to IPC socket {}: {e}", path.display()))
}

/// Does this connect error mean "no daemon" (absent / refused / stale
/// socket from a crashed daemon) vs a real fault? Mirrors
/// `flintshot_app::ipc_client::is_no_daemon`.
fn is_no_daemon(error: &std::io::Error) -> bool {
    use std::io::ErrorKind;
    matches!(
        error.kind(),
        ErrorKind::ConnectionRefused | ErrorKind::NotFound | ErrorKind::PermissionDenied
    )
}

// ── public client API ────────────────────────────────────────────────

/// Daemon reachability, as seen by a one-shot probe.
///
/// Returned by [`probe_daemon`]; the popup (Task 7) shows
/// [`DaemonState::NotRunning`] as a "daemon not running" status line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DaemonState {
    /// The socket accepts connections — a daemon is running.
    Running,
    /// The socket is missing or refused the connection — no daemon.
    NotRunning,
}

/// Connect, send one framed `request`, disconnect. For one-shot
/// actions (`CaptureRegion`, `CaptureFullscreen`, `Quit`, `PinLast`, …).
/// Fire-and-forget: no response is read; the daemon's reaction arrives
/// via side effects (spawned worker, event stream, shutdown).
///
/// Callers that want to degrade gracefully when no daemon is running
/// should call [`probe_daemon`] first (or mirror its `is_no_daemon`
/// logic on the returned error).
///
/// # Errors
/// - Socket-name resolution failure.
/// - Connect failure (the daemon isn't running).
/// - Postcard encode / framed write failure.
//
// Only the unit tests and Task 7's popup actions consume this; the
// smoke probe in `main` exercises `subscribe`/`probe_daemon` instead.
// Allow is per-item (not module-wide) so genuinely dead code still
// trips the warning once Task 7 lands.
#[allow(dead_code)]
pub async fn send_request(override_path: Option<&Path>, request: &IpcRequest) -> eyre::Result<()> {
    let mut stream = connect(override_path).await?;
    let bytes = encode_msg(request)?;
    write_frame(&mut stream, &bytes).await?;
    Ok(())
}

/// A long-lived subscription to the daemon's [`Event`] stream, created
/// by [`subscribe`]. Read with [`next_event`](Self::next_event) until
/// it returns `Ok(None)` (stream lost), then reconnect.
///
/// [`Event`]: flintshot_core::ipc::Event
pub struct EventStream {
    stream: Stream,
}

impl EventStream {
    /// Read the next event from the daemon.
    ///
    /// - `Ok(Some(event))` — got one.
    /// - `Ok(None)` — the stream ended (clean EOF or a read fault; the
    ///   fault is logged at `DEBUG`). The applet treats this as the
    ///   "stream lost" signal and reconnects. Read errors and EOF are
    ///   collapsed here because both mean "the subscriber can't make
    ///   progress"; the server papers over its own `Lagged` skew
    ///   server-side (`ipc_server.rs:296`-area), so a client never
    ///   sees a `Lagged` frame.
    /// - `Err(_)` — a decode fault (bytes arrived but postcard
    ///   couldn't decode them); the stream is corrupt and should be
    ///   dropped rather than reconnected.
    ///
    /// # Errors
    /// Postcard decode failure.
    pub async fn next_event(&mut self) -> eyre::Result<Option<IpcEvent>> {
        let payload = match read_frame(&mut self.stream).await {
            Ok(p) => p,
            Err(e) => {
                tracing::debug!(error = %e, "IPC event stream ended");
                return Ok(None);
            }
        };
        let event = decode_event(&payload)?;
        Ok(Some(event))
    }
}

/// Connect, send `Subscribe`, return an [`EventStream`] that reads
/// `Event` frames until the daemon disconnects or the stream is
/// dropped.
///
/// # Errors
/// - Socket-name resolution / connect failure (no daemon).
/// - `Subscribe` encode / framed write failure.
pub async fn subscribe(override_path: Option<&Path>) -> eyre::Result<EventStream> {
    let mut stream = connect(override_path).await?;
    let bytes = encode_msg(&IpcRequest::Subscribe)?;
    write_frame(&mut stream, &bytes).await?;
    Ok(EventStream { stream })
}

/// Probe whether a daemon is reachable at `override_path` (or the
/// platform default socket when `None`).
///
/// Checks socket-file presence first (cheap), then attempts a connect.
/// A missing file or a refused / stale connection returns
/// [`DaemonState::NotRunning`]; a successful connect returns
/// [`DaemonState::Running`] (the probe connection is dropped
/// immediately, which is harmless to the daemon).
///
/// This mirrors [`flintshot_app::ipc_client`]`::is_no_daemon`'s
/// absent/refused/stale distinction: any connect error that isn't a
/// hard fault also reports `NotRunning`, so a stale socket left behind
/// by a crashed daemon degrades as "no daemon" rather than surfacing
/// an error to the UI.
#[must_use]
pub async fn probe_daemon(override_path: Option<&Path>) -> DaemonState {
    let path = socket_path(override_path);
    if !path.exists() {
        return DaemonState::NotRunning;
    }
    let name = match socket_name(&path) {
        Ok(n) => n,
        Err(e) => {
            tracing::debug!(error = %e, "probe_daemon: socket_name failed");
            return DaemonState::NotRunning;
        }
    };
    match <Stream as interprocess::local_socket::traits::tokio::Stream>::connect(name).await {
        Ok(_) => DaemonState::Running,
        Err(e) if is_no_daemon(&e) => DaemonState::NotRunning,
        Err(e) => {
            tracing::debug!(error = %e, "probe_daemon: connect fault (not no-daemon)");
            DaemonState::NotRunning
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flintshot_core::ipc::{Event, Request};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use tokio::io::{duplex, AsyncWriteExt};

    /// Unique counter so parallel tests never collide on a socket path.
    static UNIQUE: AtomicU64 = AtomicU64::new(0);

    /// Fresh temp socket path plus the `TempDir` that owns it (kept
    /// alive for the test's duration so cleanup is automatic).
    fn fresh_socket() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let n = UNIQUE.fetch_add(1, Ordering::Relaxed);
        let sock = dir.path().join(format!("flintshot-cosmic-test-{n}.sock"));
        (dir, sock)
    }

    /// Bind a real local-socket listener on `sock`. Speaks the same
    /// framing as the daemon; ~3 LOC of glue the applet can't share
    /// with `flintshot-app` (crate boundary).
    #[cfg(unix)]
    fn bind_listener(sock: &Path) -> interprocess::local_socket::tokio::Listener {
        let name = socket_name(sock).expect("socket_name");
        interprocess::local_socket::ListenerOptions::new()
            .name(name)
            .try_overwrite(true)
            .create_tokio()
            .expect("bind listener")
    }

    // ── pure framing (no socket; runs on all platforms) ─────────────

    #[tokio::test]
    async fn framing_roundtrips_a_request() {
        let (mut a, mut b) = duplex(8 * 1024);
        let original = Request::CaptureRegion {
            monitor: Some(2),
            delay_ms: 500,
        };
        let bytes = encode_msg(&original).unwrap();
        write_frame(&mut a, &bytes).await.unwrap();
        let payload = read_frame(&mut b).await.unwrap();
        let decoded: Request = postcard::from_bytes(&payload).unwrap();
        assert_eq!(decoded, original);
    }

    #[tokio::test]
    async fn framing_roundtrips_an_event() {
        let (mut a, mut b) = duplex(8 * 1024);
        let original = Event::Pinned { id: 42 };
        let bytes = encode_msg(&original).unwrap();
        write_frame(&mut a, &bytes).await.unwrap();
        let payload = read_frame(&mut b).await.unwrap();
        let decoded: Event = postcard::from_bytes(&payload).unwrap();
        assert_eq!(decoded, original);
    }

    #[tokio::test]
    async fn truncated_frame_returns_error_not_panic() {
        let (mut a, mut b) = duplex(16);
        a.write_all(&[0x01, 0x00]).await.unwrap();
        a.shutdown().await.unwrap();
        let result = read_frame(&mut b).await;
        assert!(result.is_err(), "truncated length must error, not panic");
    }

    #[tokio::test]
    async fn oversized_length_is_rejected() {
        let (mut a, mut b) = duplex(64);
        let huge = u32::try_from(MAX_FRAME_LEN + 1).unwrap();
        a.write_all(&huge.to_le_bytes()).await.unwrap();
        a.write_all(&[0u8, 0u8]).await.unwrap();
        let err = read_frame(&mut b).await.unwrap_err();
        assert!(
            format!("{err}").contains("too large"),
            "expected 'too large' in error, got: {err}"
        );
    }

    #[tokio::test]
    async fn write_frame_rejects_oversized_payload() {
        let (_a, mut b) = duplex(64);
        let huge = vec![0u8; MAX_FRAME_LEN + 1];
        let err = write_frame(&mut b, &huge).await.unwrap_err();
        assert!(format!("{err}").contains("too large"));
    }

    // ── real socket (Unix local socket; no display needed) ──────────

    /// `send_request` connects, writes one frame, disconnects. A
    /// hand-rolled listener accepts, reads the frame, decodes —
    /// proving the client's bytes match the schema. This is the
    /// substantive proof of the framing contract without depending on
    /// `flintshot-app`.
    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn send_request_round_trips_against_test_listener() {
        let (_dir, sock) = fresh_socket();
        let listener = bind_listener(&sock);

        let original = Request::CaptureRegion {
            monitor: None,
            delay_ms: 0,
        };
        let server = tokio::spawn(async move {
            use interprocess::local_socket::traits::tokio::Listener as _;
            let mut conn = listener.accept().await.expect("accept");
            let payload = read_frame(&mut conn).await.expect("read_frame");
            postcard::from_bytes::<Request>(&payload).expect("decode")
        });

        // The listener bind is synchronous, but retry the connect
        // briefly to absorb any OS-level scheduling lag (mirror of the
        // app crate's integration tests).
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                match send_request(Some(&sock), &original).await {
                    Ok(()) => return,
                    Err(_) => tokio::time::sleep(std::time::Duration::from_millis(10)).await,
                }
            }
        })
        .await
        .expect("timed out sending request");

        let got = server.await.expect("server task");
        assert_eq!(got, original);
    }

    /// `subscribe` sends `Subscribe` then reads `Event` frames in the
    /// order the server writes them. Exercises the full `EventStream`
    /// path including `decode_event` and the stream-lost handling.
    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn subscribe_receives_events_in_order() {
        let (_dir, sock) = fresh_socket();
        let listener = bind_listener(&sock);

        let to_send = vec![
            Event::CaptureStarted,
            Event::Saved {
                path: PathBuf::from("/tmp/a.png"),
            },
            Event::Pinned { id: 7 },
        ];
        // Clone for the server closure so the original stays available
        // for the final assertion.
        let server_events = to_send.clone();

        let server = tokio::spawn(async move {
            use interprocess::local_socket::traits::tokio::Listener as _;
            let mut conn = listener.accept().await.expect("accept");
            let payload = read_frame(&mut conn).await.expect("read subscribe");
            let req: Request = postcard::from_bytes(&payload).expect("decode subscribe");
            assert_eq!(req, Request::Subscribe);
            for ev in &server_events {
                let bytes = postcard::to_allocvec(ev).expect("encode");
                write_frame(&mut conn, &bytes).await.expect("write_frame");
            }
        });

        let mut stream = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                match subscribe(Some(&sock)).await {
                    Ok(s) => return s,
                    Err(_) => tokio::time::sleep(std::time::Duration::from_millis(10)).await,
                }
            }
        })
        .await
        .expect("timed out subscribing");

        let mut got = Vec::new();
        while let Some(ev) = stream.next_event().await.expect("next_event") {
            got.push(ev);
            if got.len() == to_send.len() {
                break;
            }
        }
        assert_eq!(got, to_send);

        server.await.expect("server join");
    }

    /// A subscriber that lags on the server's broadcast channel is
    /// skipped server-side (logged, not delivered) — the client only
    /// sees well-formed `Event` frames or a clean stream-lost. Here
    /// the server closes mid-stream; the client observes that as
    /// `Ok(None)` (no panic).
    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn next_event_returns_none_on_disconnect_without_panicking() {
        let (_dir, sock) = fresh_socket();
        let listener = bind_listener(&sock);

        let server = tokio::spawn(async move {
            use interprocess::local_socket::traits::tokio::Listener as _;
            let mut conn = listener.accept().await.expect("accept");
            // Drain Subscribe, send one event, then drop the connection
            // (simulates the daemon's broadcast channel closing).
            let _ = read_frame(&mut conn).await.expect("read subscribe");
            let bytes = postcard::to_allocvec(&Event::CaptureStarted).expect("encode");
            write_frame(&mut conn, &bytes)
                .await
                .expect("write one event");
            drop(conn);
        });

        let mut stream = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                match subscribe(Some(&sock)).await {
                    Ok(s) => return s,
                    Err(_) => tokio::time::sleep(std::time::Duration::from_millis(10)).await,
                }
            }
        })
        .await
        .expect("timed out subscribing");

        let first = stream.next_event().await.expect("first event");
        assert_eq!(first, Some(Event::CaptureStarted));
        // Server dropped the connection; the next read is EOF, surfaced
        // as Ok(None) — not an error, not a panic.
        let second = stream.next_event().await.expect("eof");
        assert_eq!(second, None, "disconnect must surface as Ok(None)");

        server.await.expect("server join");
    }

    /// A bound listener that accepts connections reports `Running`.
    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn probe_daemon_running_when_listener_accepts() {
        let (_dir, sock) = fresh_socket();
        let _listener = bind_listener(&sock);

        let state = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                match probe_daemon(Some(&sock)).await {
                    DaemonState::Running => return DaemonState::Running,
                    DaemonState::NotRunning => {
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }
                }
            }
        })
        .await
        .expect("timed out probing");
        assert_eq!(state, DaemonState::Running);
    }

    /// A missing socket file reports `NotRunning` without touching the
    /// network (the cheap `path.exists()` early-return).
    #[tokio::test]
    async fn probe_daemon_not_running_when_socket_absent() {
        let state = probe_daemon(Some(Path::new(
            "/this/does/not/exist/flintshot-cosmic.sock",
        )))
        .await;
        assert_eq!(state, DaemonState::NotRunning);
    }

    /// A stale socket file left by a crashed daemon (the path exists
    /// but nothing accepts) reports `NotRunning` rather than erroring.
    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn probe_daemon_not_running_for_stale_socket() {
        let (_dir, sock) = fresh_socket();
        std::fs::write(&sock, b"stale").expect("create stale file");
        let state = probe_daemon(Some(&sock)).await;
        assert_eq!(state, DaemonState::NotRunning);
    }
}
