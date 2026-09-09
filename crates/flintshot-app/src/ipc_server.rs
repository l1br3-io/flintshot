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

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
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
pub(crate) async fn read_frame<R>(r: &mut R) -> eyre::Result<Vec<u8>>
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

/// Postcard-decode an [`Event`] (client side of the bind reply).
pub(crate) fn decode_event(buf: &[u8]) -> eyre::Result<IpcEvent> {
    postcard::from_bytes(buf).map_err(|e| eyre::eyre!("postcard decode event: {e}"))
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

// ── last-saved state ─────────────────────────────────────────────────

/// Daemon-wide "last saved capture" path.
///
/// Workers report saves via [`Request::ReportSaved`]; the IPC connection
/// layer records the path here. [`Request::PinLast`] / [`Request::OpenLast`]
/// (Task 4) and Phase 4 upload-from-last read it back. Cheap shared
/// state — an [`Arc`] around a [`Mutex`] guarding an [`Option<PathBuf>`].
/// The critical section is a single clone/swap (never held across an
/// `.await`), so a `std::sync::Mutex` is correct and avoids a tokio
/// mutex's overhead. See ADR-0023.
///
/// [`Request::ReportSaved`]: flintshot_core::ipc::Request::ReportSaved
/// [`Request::PinLast`]: flintshot_core::ipc::Request::PinLast
/// [`Request::OpenLast`]: flintshot_core::ipc::Request::OpenLast
#[derive(Clone, Default)]
pub struct LastSaved {
    path: Arc<Mutex<Option<PathBuf>>>,
}

impl LastSaved {
    /// Create an empty tracker (no save recorded yet).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a freshly-saved path, overwriting any previous value.
    pub fn set(&self, path: PathBuf) {
        *self.path.lock().expect("last-saved mutex poisoned") = Some(path);
    }

    /// Clone out the current last-saved path, or `None` if no worker has
    /// reported a save yet.
    #[must_use]
    // No reader yet — Task 4 (`PinLast`/`OpenLast`) and the Phase 3
    // integration test consume this. Mirrors `bus()` / `last_saved()`.
    #[allow(dead_code)]
    pub fn get(&self) -> Option<PathBuf> {
        self.path.lock().expect("last-saved mutex poisoned").clone()
    }
}

// ── server ───────────────────────────────────────────────────────────

/// The tokio local-socket listener type, aliased to keep the cfg noise
/// out of the struct body.
type Listener = interprocess::local_socket::tokio::Listener;
/// The tokio local-socket stream type.
pub(crate) type Stream = interprocess::local_socket::tokio::Stream;

/// The IPC server: owns the listener, the subscriber bus, the
/// last-saved + pin-id dispatch state, the portal-bind handle, the
/// tray gate (capture handshake, ADR-0036), the capture dispatch
/// context (supersede-before-spawn, smart-window-hiding), and a
/// shutdown signal.
pub struct IpcServer {
    listener: Listener,
    bus: SubscriberBus,
    state: crate::worker::DispatchState,
    portal: crate::hotkeys_portal::PortalHandle,
    gate: Arc<crate::tray_gate::TrayGate>,
    capture: crate::worker::CaptureDispatch,
    prepare: Arc<crate::prepare_gate::PrepareGate>,
    shutdown: Arc<Notify>,
}

impl IpcServer {
    /// Bind a listener on [`socket_path`] (`override_path` honoured),
    /// reclaiming any stale socket file on Unix via `try_overwrite`,
    /// with a fresh subscriber bus and a permanently-unavailable
    /// portal handle (integration tests don't drive the portal).
    ///
    /// # Errors
    /// - If the socket name can't be resolved.
    /// - If binding the listener fails (e.g. a live socket already
    ///   holds the path and can't be replaced).
    //
    // The daemon uses `bind_with_bus` (it shares a bus with the tray
    // path and owns the live portal session); this convenience is used
    // by the integration tests.
    #[allow(dead_code)]
    pub fn bind(override_path: Option<&Path>) -> eyre::Result<Self> {
        Self::bind_with_bus(
            override_path,
            SubscriberBus::new(),
            crate::worker::DispatchState::new(),
            crate::hotkeys_portal::PortalHandle::unavailable(
                "portal bind is not wired in this IPC server instance",
            ),
            Arc::new(crate::tray_gate::TrayGate::new()),
            crate::worker::CaptureDispatch::new(),
            Arc::new(crate::prepare_gate::PrepareGate::new()),
        )
    }

    /// Like [`bind`](Self::bind) but uses an externally-provided `bus`
    /// and `portal` handle. The daemon shares one bus between the IPC
    /// server (which fans `Event`s to IPC subscribers) and its own
    /// tray/hotkey dispatch path, so a capture started from the tray
    /// reaches IPC subscribers and vice versa; the portal handle routes
    /// `Request::BindShortcuts` into the daemon's portal session; the
    /// tray gate carries the capture handshake (`HideTrayForCapture`
    /// → ack, `ReportCaptured` → restore) between this tokio-side
    /// handler and the daemon's main loop, which alone may touch the
    /// tray icon; the capture dispatch context serializes the
    /// supersede-before-spawn Replace policy across every entry point.
    pub fn bind_with_bus(
        override_path: Option<&Path>,
        bus: SubscriberBus,
        state: crate::worker::DispatchState,
        portal: crate::hotkeys_portal::PortalHandle,
        gate: Arc<crate::tray_gate::TrayGate>,
        capture: crate::worker::CaptureDispatch,
        prepare: Arc<crate::prepare_gate::PrepareGate>,
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
            bus,
            state,
            portal,
            gate,
            capture,
            prepare,
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

    /// A cloneable handle to the daemon's last-saved path. The daemon
    /// keeps one; Task 4 (`PinLast`/`OpenLast`) reads it to act on the
    /// most recent worker save.
    //
    // Only the integration tests use this externally; the daemon's
    // `handle_connection` updates via its own state clone.
    #[allow(dead_code)]
    pub fn last_saved(&self) -> LastSaved {
        self.state.last_saved.clone()
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
        let state = self.state.clone();
        let portal = self.portal.clone();
        let gate = Arc::clone(&self.gate);
        let capture = self.capture.clone();
        let prepare = Arc::clone(&self.prepare);
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
                    let state = state.clone();
                    let portal = portal.clone();
                    let gate = Arc::clone(&gate);
                    let capture = capture.clone();
                    let prepare = Arc::clone(&prepare);
                    let shutdown = Arc::clone(&shutdown);
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, bus, state, shutdown, portal, gate, capture, prepare).await {
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
/// [`Event`] sink; `ReportSaved` records the worker's path in the
/// daemon's last-saved state (and broadcasts [`Event::Saved`]);
/// `BindShortcuts` drives the daemon's portal session and writes the
/// outcome back on this connection; `HideTrayForCapture` /
/// `ReportCaptured` drive the tray gate (capture handshake,
/// ADR-0036); `Quit` signals daemon shutdown; capture requests route
/// through the supersede-then-spawn helper (Replace policy,
/// `worker::dispatch_capture`) while everything else uses the worker
/// dispatch table directly.
#[allow(clippy::too_many_arguments)]
// One arm per intercepted request kind plus the dispatch tail; the
// routing table reads best as a single body.
#[allow(clippy::too_many_lines)]
async fn handle_connection<S>(
    mut stream: S,
    bus: SubscriberBus,
    state: crate::worker::DispatchState,
    shutdown: Arc<Notify>,
    portal: crate::hotkeys_portal::PortalHandle,
    gate: Arc<crate::tray_gate::TrayGate>,
    capture: crate::worker::CaptureDispatch,
    prepare: Arc<crate::prepare_gate::PrepareGate>,
) -> eyre::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
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

        // ReportSaved: a worker reports the path it just saved. Record
        // it as the daemon's last-saved (for PinLast/OpenLast and
        // upload-from-last) and broadcast Event::Saved so subscribers
        // learn about the save. This is a state update, not a spawn —
        // it never reaches the worker dispatch table. See ADR-0023.
        if let IpcRequest::ReportSaved { path } = &request {
            tracing::info!(path = %path.display(), "worker reported save");
            state.last_saved.set(path.clone());
            bus.broadcast(IpcEvent::Saved { path: path.clone() });
            continue;
        }

        // ReportUploaded: a worker reports the URL + delete-handle of a
        // completed upload. Broadcast Event::Uploaded to subscribers so
        // the tray / COSMIC applet / IPC scripts learn the result. This
        // is a relay, not a spawn — it never reaches the worker dispatch
        // table. Parallels ReportSaved above (extends ADR-0023).
        if let IpcRequest::ReportUploaded { url, delete_handle } = &request {
            tracing::info!(%url, "worker reported upload");
            bus.broadcast(IpcEvent::Uploaded {
                url: url.clone(),
                delete_handle: delete_handle.clone(),
            });
            continue;
        }

        // ReportEditorWindowOpened (smart-window-hiding): a worker's
        // truthful "an annotation window is about to run" — one
        // report per actual window (fresh capture, recapture reopen,
        // `open`), replacing the old spawn-time proxy broadcast.
        // Fire-and-forget relay, mirroring ReportSaved.
        if matches!(request, IpcRequest::ReportEditorWindowOpened) {
            tracing::info!("worker reported an editor/overlay window opening");
            bus.broadcast(IpcEvent::EditorOpened);
            continue;
        }

        // BindShortcuts (Phase 6): binding belongs to the daemon's
        // portal session, so the connection layer drives it — the
        // worker dispatch table never sees it. Awaiting the user's
        // dialog blocks only this connection; the reply is one Event
        // frame (ShortcutsBound / Error) written straight back.
        if matches!(request, IpcRequest::BindShortcuts) {
            tracing::info!("IPC BindShortcuts received; opening portal bind dialog");
            let event = match portal.bind().await {
                Ok(bindings) => IpcEvent::ShortcutsBound { bindings },
                Err(failure) => IpcEvent::Error {
                    code: failure.code,
                    message: failure.message,
                },
            };
            let bytes = encode_msg(&event)?;
            write_frame(&mut stream, &bytes).await?;
            continue;
        }

        // HideTrayForCapture (ADR-0036): a capture worker is about to
        // grab the screen. Register the in-flight capture, wait —
        // bounded — for the daemon's main loop to hide the icon and
        // let the panel re-render, then ack on this connection. Like
        // BindShortcuts, this is connection-layer interception: the
        // worker dispatch table never sees it. The wait blocks only
        // this connection; the worker's own reply wait is the
        // authoritative bound.
        if matches!(request, IpcRequest::HideTrayForCapture) {
            let baseline = gate.begin_capture();
            if !gate.wait_confirmed(baseline).await {
                tracing::warn!(
                    "tray-hide confirmation timed out; acking anyway (the capture proceeds)"
                );
            }
            let bytes = encode_msg(&IpcEvent::TrayHidden)?;
            write_frame(&mut stream, &bytes).await?;
            continue;
        }

        // PrepareCapture (smart-window-hiding): the generalized
        // pre-grab handshake. The connection layer broadcasts the
        // hide, closes the pins, arms both gates (prepare + tray),
        // and awaits both — every wait bounded, the reply going out
        // even on timeout (a capture must never fail because a
        // window was slow — ADR-0036, extended). Supersedes and
        // internally includes HideTrayForCapture; the old request
        // keeps working below (rolling-upgrade skew).
        if matches!(request, IpcRequest::PrepareCapture) {
            tracing::info!("PrepareCapture received; clearing flintshot surfaces");
            bus.broadcast(IpcEvent::HideForCapture);
            capture.sessions().close_all_pins();
            let tray_baseline = gate.begin_capture();
            prepare.begin(std::time::Instant::now());
            let (prepared, tray_confirmed) = tokio::join!(prepare.wait_ready(), async {
                gate.wait_confirmed(tray_baseline).await
            });
            if !matches!(
                prepared,
                Some(crate::prepare_gate::PrepareResolution::Ready)
            ) {
                tracing::warn!(
                    "prepare handshake timed out (prepared={prepared:?}, \
                     tray_confirmed={tray_confirmed}); replying anyway"
                );
            }
            let bytes = encode_msg(&IpcEvent::CaptureReady)?;
            write_frame(&mut stream, &bytes).await?;
            continue;
        }

        // ReportAppHidden (smart-window-hiding): the app window's
        // "window destroyed" ack for the handshake above.
        // Fire-and-forget.
        if matches!(request, IpcRequest::ReportAppHidden) {
            tracing::debug!("app window reported hidden");
            prepare.report_app_hidden();
            continue;
        }

        // ReportCaptured (ADR-0036): the grab finished (success or
        // failure), so this in-flight capture releases its hold — the
        // icon returns when the last one reports — and any pins the
        // prepare handshake closed come back at their exact
        // position/size/opacity (best-effort respawns). Fire-and-forget,
        // never broadcast: only the daemon's tray cares.
        if matches!(request, IpcRequest::ReportCaptured) {
            gate.end_capture();
            crate::worker::worker_respawn_queued_pins(
                &capture.sessions(),
                &crate::worker::CommandSpawner::with_bus_and_sessions(
                    bus.clone(),
                    capture.sessions(),
                ),
            );
            continue;
        }

        // Everything else routes through the worker dispatch table.
        // Capture requests take the supersede-then-spawn path first
        // (Replace policy: the new capture replaces any live session,
        // bounded waits on the blocking pool so this connection — and
        // only this connection — is held); PinLast/OpenLast read the
        // last-saved state; Quit signals shutdown; unbuilt phases
        // broadcast an `unsupported` Error.
        let outcome = if crate::worker::spawns_capture_worker(&request) {
            crate::worker::dispatch_capture(
                capture.clone(),
                request.clone(),
                state.clone(),
                bus.clone(),
            )
            .await?
        } else {
            crate::worker::dispatch(
                &request,
                &crate::worker::CommandSpawner::with_bus_and_sessions(
                    bus.clone(),
                    capture.sessions(),
                ),
                &state,
                &mut |event| {
                    bus.broadcast(event);
                },
            )?
        };
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

    // ── BindShortcuts interception (Phase 6, Task 003) ─────────────
    //
    // Duplex pairs exercise `handle_connection` directly — no real
    // socket needed, and the portal side is a stub the test drives.

    use crate::hotkeys_portal::{BindFailure, PortalCmd, PortalHandle};
    use flintshot_core::ipc::ShortcutBinding;

    // ── Tray-hide handshake interception (ADR-0036) ────────────────

    use crate::tray_gate::TrayGate;

    /// A gate with a 30 ms grace so a background stepper confirms in
    /// tens of milliseconds, and a 2 s confirm bound.
    fn handshake_gate() -> Arc<TrayGate> {
        Arc::new(TrayGate::with_durations(
            std::time::Duration::from_millis(30),
            std::time::Duration::from_millis(30),
            std::time::Duration::from_millis(30),
            std::time::Duration::from_secs(1),
            std::time::Duration::from_secs(2),
        ))
    }

    /// Step a shared gate as if a real tray were present, for `millis`
    /// — the main loop's job, stood in for the test.
    fn step_in_background(gate: &Arc<TrayGate>, millis: u64) -> tokio::task::JoinHandle<()> {
        let gate = Arc::clone(gate);
        tokio::spawn(async move {
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(millis);
            while std::time::Instant::now() < deadline {
                gate.step(std::time::Instant::now(), true);
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
    }

    #[tokio::test]
    async fn hide_tray_for_capture_replies_tray_hidden_once_confirmed() {
        let gate = handshake_gate();
        let stepper = step_in_background(&gate, 2_000);
        let (mut client, task) = bind_test_connection(
            crate::hotkeys_portal::PortalHandle::unavailable("unused in this test"),
            Arc::clone(&gate),
            None,
            None,
        );
        write_frame(
            &mut client,
            &encode_msg(&Request::HideTrayForCapture).unwrap(),
        )
        .await
        .unwrap();

        let payload =
            tokio::time::timeout(std::time::Duration::from_secs(2), read_frame(&mut client))
                .await
                .expect("the ack arrives once the main loop confirms")
                .unwrap();
        let decoded: Event = postcard::from_bytes(&payload).unwrap();
        assert!(
            matches!(decoded, Event::TrayHidden),
            "expected TrayHidden, got {decoded:?}"
        );
        assert_eq!(gate.hide_depth(), 1, "the handshake holds until the report");
        task.abort();
        stepper.abort();
    }

    #[tokio::test]
    async fn hide_tray_for_capture_acks_after_the_bound_when_unconfirmed() {
        // A gate nothing steps and a 20 ms confirm bound: the reply
        // must still arrive (the worker proceeds regardless; worst
        // case the icon made it into the shot — pre-fix behaviour).
        let gate = Arc::new(TrayGate::with_durations(
            std::time::Duration::from_secs(1),
            std::time::Duration::from_secs(1),
            std::time::Duration::from_secs(1),
            std::time::Duration::from_secs(1),
            std::time::Duration::from_millis(20),
        ));
        let (mut client, task) = bind_test_connection(
            crate::hotkeys_portal::PortalHandle::unavailable("unused in this test"),
            Arc::clone(&gate),
            None,
            None,
        );
        write_frame(
            &mut client,
            &encode_msg(&Request::HideTrayForCapture).unwrap(),
        )
        .await
        .unwrap();

        let payload =
            tokio::time::timeout(std::time::Duration::from_secs(2), read_frame(&mut client))
                .await
                .expect("the bounded wait still acks")
                .unwrap();
        let decoded: Event = postcard::from_bytes(&payload).unwrap();
        assert!(matches!(decoded, Event::TrayHidden));
        task.abort();
    }

    #[tokio::test]
    async fn report_captured_releases_the_handshake_hold() {
        let gate = handshake_gate();
        let stepper = step_in_background(&gate, 2_000);
        let (mut client, task) = bind_test_connection(
            crate::hotkeys_portal::PortalHandle::unavailable("unused in this test"),
            Arc::clone(&gate),
            None,
            None,
        );
        write_frame(
            &mut client,
            &encode_msg(&Request::HideTrayForCapture).unwrap(),
        )
        .await
        .unwrap();
        let payload =
            tokio::time::timeout(std::time::Duration::from_secs(2), read_frame(&mut client))
                .await
                .expect("ack arrives")
                .unwrap();
        assert!(matches!(
            postcard::from_bytes::<Event>(&payload).unwrap(),
            Event::TrayHidden
        ));

        // The grab finished: fire-and-forget report, then the hold
        // must drain back to zero (no reply is expected — poll).
        write_frame(&mut client, &encode_msg(&Request::ReportCaptured).unwrap())
            .await
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while gate.hide_depth() != 0 && std::time::Instant::now() < deadline {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        assert_eq!(
            gate.hide_depth(),
            0,
            "the report releases the in-flight capture"
        );
        task.abort();
        stepper.abort();
    }

    // ── PrepareCapture coordination (smart-window-hiding Task 005) ──

    /// A prepare gate with a short bound and tiny grace, plus a
    /// background "main loop" stepping it to clear conditions (the
    /// stepper ends with the test runtime).
    fn stepped_prepare_gate() -> Arc<crate::prepare_gate::PrepareGate> {
        let gate = Arc::new(crate::prepare_gate::PrepareGate::with_durations(
            std::time::Duration::from_secs(2),
            std::time::Duration::from_millis(10),
        ));
        let stepper = {
            let gate = Arc::clone(&gate);
            tokio::spawn(async move {
                for _ in 0..600 {
                    gate.step(std::time::Instant::now(), false, false, 0);
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }
            })
        };
        tokio::spawn(async move {
            let _ = stepper.await;
        });
        Arc::clone(&gate)
    }

    #[tokio::test]
    async fn prepare_capture_replies_capture_ready_when_clear() {
        let prepare = stepped_prepare_gate();
        let tray = handshake_gate();
        let tray_stepper = step_in_background(&tray, 2_000);
        let (mut client, task) = bind_test_connection(
            crate::hotkeys_portal::PortalHandle::unavailable("unused in this test"),
            tray,
            None,
            Some(Arc::clone(&prepare)),
        );
        write_frame(&mut client, &encode_msg(&Request::PrepareCapture).unwrap())
            .await
            .unwrap();

        let payload =
            tokio::time::timeout(std::time::Duration::from_secs(2), read_frame(&mut client))
                .await
                .expect("the reply arrives once the gate is clear")
                .unwrap();
        let decoded: Event = postcard::from_bytes(&payload).unwrap();
        assert!(
            matches!(decoded, Event::CaptureReady),
            "expected CaptureReady, got {decoded:?}"
        );
        task.abort();
        tray_stepper.abort();
    }

    #[tokio::test]
    async fn report_app_hidden_unblocks_the_prepare_wait() {
        // A live app session means the gate waits for the ack: no
        // reply until ReportAppHidden arrives on any connection.
        let gate = Arc::new(crate::prepare_gate::PrepareGate::with_durations(
            std::time::Duration::from_secs(2),
            std::time::Duration::from_millis(10),
        ));
        let capture = crate::worker::CaptureDispatch::new();
        capture
            .sessions()
            .register_app(4242, std::sync::Arc::new(std::sync::Mutex::new(None)));
        let tray = handshake_gate();
        let tray_stepper = step_in_background(&tray, 4_000);
        let (mut client, task) = bind_test_connection(
            crate::hotkeys_portal::PortalHandle::unavailable("unused in this test"),
            tray,
            Some(capture),
            Some(Arc::clone(&gate)),
        );
        write_frame(&mut client, &encode_msg(&Request::PrepareCapture).unwrap())
            .await
            .unwrap();

        // No ack yet: the reply must not arrive inside a short
        // window (well before the 2 s bound would bail us out).
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(300),
                read_frame(&mut client)
            )
            .await
            .is_err(),
            "alive app without an ack holds the reply"
        );

        // The main loop steps the gate; the ack unblocks it.
        let acker = {
            let gate = Arc::clone(&gate);
            tokio::spawn(async move {
                for _ in 0..600 {
                    gate.step(std::time::Instant::now(), true, false, 0);
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }
            })
        };
        gate.report_app_hidden();
        let payload =
            tokio::time::timeout(std::time::Duration::from_secs(2), read_frame(&mut client))
                .await
                .expect("the ack unblocks the reply")
                .unwrap();
        let decoded: Event = postcard::from_bytes(&payload).unwrap();
        assert!(matches!(decoded, Event::CaptureReady));
        task.abort();
        acker.abort();
        tray_stepper.abort();
    }

    #[tokio::test]
    async fn prepare_capture_replies_after_the_bound_when_never_clear() {
        // An app that never acks: the bounded handshake still replies
        // (a capture must never fail because a window was slow).
        let gate = Arc::new(crate::prepare_gate::PrepareGate::with_durations(
            std::time::Duration::from_millis(300),
            std::time::Duration::from_millis(10),
        ));
        let capture = crate::worker::CaptureDispatch::new();
        capture
            .sessions()
            .register_app(4243, std::sync::Arc::new(std::sync::Mutex::new(None)));
        let tray = handshake_gate();
        let tray_stepper = step_in_background(&tray, 4_000);
        let (mut client, task) = bind_test_connection(
            crate::hotkeys_portal::PortalHandle::unavailable("unused in this test"),
            tray,
            Some(capture),
            Some(Arc::clone(&gate)),
        );
        write_frame(&mut client, &encode_msg(&Request::PrepareCapture).unwrap())
            .await
            .unwrap();

        // The main loop steps but the app never acks.
        let stepper = {
            let gate = Arc::clone(&gate);
            tokio::spawn(async move {
                for _ in 0..600 {
                    gate.step(std::time::Instant::now(), true, false, 0);
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }
            })
        };
        let payload =
            tokio::time::timeout(std::time::Duration::from_secs(2), read_frame(&mut client))
                .await
                .expect("the bound still replies")
                .unwrap();
        let decoded: Event = postcard::from_bytes(&payload).unwrap();
        assert!(
            matches!(decoded, Event::CaptureReady),
            "timeout still replies CaptureReady (warned), got {decoded:?}"
        );
        task.abort();
        stepper.abort();
        tray_stepper.abort();
    }

    // ── Real EditorOpened (smart-window-hiding Task 008) ─────────

    #[tokio::test]
    async fn report_editor_window_opened_broadcasts_editor_opened() {
        // The worker's truthful window report replaces the spawn-time
        // proxy: each report fans out exactly one EditorOpened.
        let bus = SubscriberBus::new();
        let mut subscriber = bus.subscribe();
        let (client, server) = tokio::io::duplex(8 * 1024);
        let state = crate::worker::DispatchState::new();
        let shutdown = Arc::new(Notify::new());
        let capture = crate::worker::CaptureDispatch::new();
        let prepare = Arc::new(crate::prepare_gate::PrepareGate::new());
        let task = tokio::spawn(async move {
            let _ = handle_connection(
                server,
                bus,
                state,
                shutdown,
                crate::hotkeys_portal::PortalHandle::unavailable("unused"),
                Arc::new(crate::tray_gate::TrayGate::new()),
                capture,
                prepare,
            )
            .await;
        });

        let mut client = client;
        write_frame(
            &mut client,
            &encode_msg(&Request::ReportEditorWindowOpened).unwrap(),
        )
        .await
        .unwrap();
        write_frame(
            &mut client,
            &encode_msg(&Request::ReportEditorWindowOpened).unwrap(),
        )
        .await
        .unwrap();

        let first =
            tokio::time::timeout(std::time::Duration::from_secs(2), subscriber.recv()).await;
        assert_eq!(first, Ok(Ok(Event::EditorOpened)), "one per report");
        let second =
            tokio::time::timeout(std::time::Duration::from_secs(2), subscriber.recv()).await;
        assert_eq!(second, Ok(Ok(Event::EditorOpened)));
        task.abort();
    }

    /// Spawn `handle_connection` on the server half of a duplex pair
    /// with a fresh bus/state/shutdown, the given portal handle, tray
    /// gate, capture dispatch context, and prepare gate. Returns the
    /// client half + the task handle.
    #[allow(clippy::too_many_arguments)]
    fn bind_test_connection(
        portal: PortalHandle,
        gate: Arc<crate::tray_gate::TrayGate>,
        capture: Option<crate::worker::CaptureDispatch>,
        prepare: Option<Arc<crate::prepare_gate::PrepareGate>>,
    ) -> (tokio::io::DuplexStream, tokio::task::JoinHandle<()>) {
        let (client, server) = tokio::io::duplex(8 * 1024);
        let bus = SubscriberBus::new();
        let state = crate::worker::DispatchState::new();
        let shutdown = Arc::new(Notify::new());
        let capture = capture.unwrap_or_default();
        let prepare = prepare.unwrap_or_else(|| Arc::new(crate::prepare_gate::PrepareGate::new()));
        let task = tokio::spawn(async move {
            let _ = handle_connection(server, bus, state, shutdown, portal, gate, capture, prepare)
                .await;
        });
        (client, task)
    }

    #[tokio::test]
    async fn bind_shortcuts_replies_with_bound_event() {
        let (portal, mut cmds) = PortalHandle::stub();
        // Drive the stub: answer the bind with a successful binding.
        tokio::spawn(async move {
            if let Some(PortalCmd::Bind { reply }) = cmds.recv().await {
                let binding = ShortcutBinding {
                    id: "region".into(),
                    trigger: "Print".into(),
                };
                reply.send(Ok(vec![binding])).expect("reply sends");
            }
        });

        let (mut client, task) = bind_test_connection(
            portal,
            Arc::new(crate::tray_gate::TrayGate::new()),
            None,
            None,
        );
        write_frame(&mut client, &encode_msg(&Request::BindShortcuts).unwrap())
            .await
            .unwrap();

        let payload =
            tokio::time::timeout(std::time::Duration::from_secs(2), read_frame(&mut client))
                .await
                .expect("reply arrives")
                .unwrap();
        let decoded: Event = postcard::from_bytes(&payload).unwrap();
        assert!(matches!(
            &decoded,
            Event::ShortcutsBound { bindings }
                if bindings.len() == 1
                    && bindings[0].id == "region"
                    && bindings[0].trigger == "Print"
        ));
        task.abort();
    }

    #[tokio::test]
    async fn bind_shortcuts_cancelled_replies_with_error_event() {
        let (portal, mut cmds) = PortalHandle::stub();
        tokio::spawn(async move {
            if let Some(PortalCmd::Bind { reply }) = cmds.recv().await {
                reply
                    .send(Err(BindFailure {
                        code: "bind-cancelled".into(),
                        message: "dismissed".into(),
                    }))
                    .expect("reply sends");
            }
        });

        let (mut client, task) = bind_test_connection(
            portal,
            Arc::new(crate::tray_gate::TrayGate::new()),
            None,
            None,
        );
        write_frame(&mut client, &encode_msg(&Request::BindShortcuts).unwrap())
            .await
            .unwrap();

        let payload =
            tokio::time::timeout(std::time::Duration::from_secs(2), read_frame(&mut client))
                .await
                .expect("reply arrives")
                .unwrap();
        let decoded: Event = postcard::from_bytes(&payload).unwrap();
        assert!(
            matches!(&decoded, Event::Error { code, .. } if code == "bind-cancelled"),
            "expected bind-cancelled error, got {decoded:?}"
        );
        task.abort();
    }

    #[tokio::test]
    async fn bind_shortcuts_with_unavailable_portal_replies_error() {
        let portal = PortalHandle::unavailable("no portal frontend");
        let (mut client, task) = bind_test_connection(
            portal,
            Arc::new(crate::tray_gate::TrayGate::new()),
            None,
            None,
        );
        write_frame(&mut client, &encode_msg(&Request::BindShortcuts).unwrap())
            .await
            .unwrap();

        let payload =
            tokio::time::timeout(std::time::Duration::from_secs(2), read_frame(&mut client))
                .await
                .expect("reply arrives")
                .unwrap();
        let decoded: Event = postcard::from_bytes(&payload).unwrap();
        assert!(
            matches!(
                &decoded,
                Event::Error { code, message }
                    if code == "bind-failed" && message.contains("no portal frontend")
            ),
            "expected classified bind-failed error, got {decoded:?}"
        );
        task.abort();
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

    #[test]
    fn last_saved_starts_empty() {
        let ls = LastSaved::new();
        assert_eq!(ls.get(), None, "a fresh tracker has no save recorded");
    }

    #[test]
    fn last_saved_set_then_get_round_trips_the_path() {
        let ls = LastSaved::new();
        ls.set(PathBuf::from("/tmp/flintshot/shot.png"));
        assert_eq!(
            ls.get(),
            Some(PathBuf::from("/tmp/flintshot/shot.png")),
            "set must record the path for read-back"
        );
    }

    #[test]
    fn last_saved_set_overwrites_previous() {
        let ls = LastSaved::new();
        ls.set(PathBuf::from("/tmp/a.png"));
        ls.set(PathBuf::from("/tmp/b.png"));
        assert_eq!(
            ls.get(),
            Some(PathBuf::from("/tmp/b.png")),
            "the most recent save wins"
        );
    }

    #[test]
    fn last_saved_clones_share_state() {
        // Cloning the handle (one per IPC connection) shares the
        // underlying Arc<Mutex<…>>; a write on one clone is visible to
        // every other.
        let a = LastSaved::new();
        let b = a.clone();
        a.set(PathBuf::from("/tmp/shared.png"));
        assert_eq!(
            b.get(),
            Some(PathBuf::from("/tmp/shared.png")),
            "clones must observe writes from each other"
        );
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

    /// A worker sends `Request::ReportSaved`; the daemon's last-saved
    /// state must hold the reported path. `#[ignore]` (needs a real
    /// local socket). Run with:
    /// `cargo test -p flintshot-app -- --ignored ipc_server`
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "needs a real local socket (no display); run explicitly"]
    async fn integration_report_saved_updates_last_saved() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static UNIQUE: AtomicU64 = AtomicU64::new(0);

        let dir = tempfile::tempdir().unwrap();
        let n = UNIQUE.fetch_add(1, Ordering::Relaxed);
        let sock = dir.path().join(format!("flintshot-report-{n}.sock"));

        let server = IpcServer::bind(Some(&sock)).unwrap();
        let last_saved = server.last_saved();
        let server_task = tokio::spawn(async move { server.serve().await });

        // Send ReportSaved. The listener may not be ready immediately, so
        // retry the connect/send briefly.
        let path = std::path::PathBuf::from("/tmp/flintshot-reported.png");
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                match send_request(Some(&sock), &Request::ReportSaved { path: path.clone() }).await
                {
                    Ok(()) => return,
                    Err(_) => tokio::time::sleep(std::time::Duration::from_millis(20)).await,
                }
            }
        })
        .await
        .expect("timed out sending ReportSaved");

        // The connection layer updates last_saved asynchronously; poll
        // until it lands (bounded).
        let observed = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if let Some(p) = last_saved.get() {
                    if p == path {
                        return true;
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .is_ok();
        assert!(
            observed,
            "last-saved state was not updated to the reported path"
        );

        server_task.abort();
    }

    /// A worker sends `Request::ReportUploaded`; a subscriber that
    /// connected earlier must receive `Event::Uploaded` with the same
    /// URL and delete-handle. `#[ignore]` (needs a real local socket).
    /// Run with: `cargo test -p flintshot-app -- --ignored report_uploaded`
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "needs a real local socket (no display); run explicitly"]
    async fn integration_report_uploaded_broadcasts_to_subscribers() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static UNIQUE: AtomicU64 = AtomicU64::new(0);

        let dir = tempfile::tempdir().unwrap();
        let n = UNIQUE.fetch_add(1, Ordering::Relaxed);
        let sock = dir.path().join(format!("flintshot-uploaded-{n}.sock"));

        let server = IpcServer::bind(Some(&sock)).unwrap();
        let server_task = tokio::spawn(async move { server.serve().await });

        // Connect a subscriber and send Subscribe.
        let name = socket_name(&sock).unwrap();
        let mut sub = <Stream as interprocess::local_socket::traits::tokio::Stream>::connect(name)
            .await
            .unwrap();
        write_frame(&mut sub, &encode_msg(&Request::Subscribe).unwrap())
            .await
            .unwrap();

        let url = "https://imgur.com/abc123".to_string();
        let delete_handle = Some("delete-hash-xyz".to_string());

        // The server must register the subscriber before the report
        // arrives. Retry send_request + read until the event lands —
        // sidesteps the inherent subscribe/report race.
        let payload = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                let _ = send_request(
                    Some(&sock),
                    &Request::ReportUploaded {
                        url: url.clone(),
                        delete_handle: delete_handle.clone(),
                    },
                )
                .await;
                if let Ok(Ok(payload)) =
                    tokio::time::timeout(std::time::Duration::from_millis(80), read_frame(&mut sub))
                        .await
                {
                    return payload;
                }
            }
        })
        .await
        .expect("timed out waiting for Event::Uploaded");

        let decoded: Event = postcard::from_bytes(&payload).unwrap();
        assert_eq!(decoded, Event::Uploaded { url, delete_handle });

        drop(sub);
        server_task.abort();
    }
}
