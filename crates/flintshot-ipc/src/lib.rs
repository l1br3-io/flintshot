//! IPC transport shared by `flintshot-app` (the daemon) and
//! `flintshot-cosmic` (the panel applet). The schema lives in
//! [`flintshot_core::ipc`]; this crate owns only the `interprocess`
//! socket plumbing and the length-prefixed postcard framing.
//!
//! # Wire-format invariants
//!
//! - **u32 little-endian** length prefix.
//! - **1 MiB cap** ([`MAX_FRAME_LEN`]) on payload size.
//! - **Postcard** encoding of [`Request`] / [`Event`].
//!
//! The framing tests at the bottom of this file moved unchanged from
//! `flintshot-app::ipc_server` — they are the proof of byte-identity
//! with the Phase 2 daemon wire format.
//!
//! [`Request`]: flintshot_core::ipc::Request
//! [`Event`]: flintshot_core::ipc::Event

// RATIONALE: this crate exists so the daemon (`flintshot-app`, a bin)
// and the COSMIC applet (`flintshot-cosmic`, also a bin) share one
// byte-identical wire-format implementation. A bin cannot depend on
// another bin, so the transport must be a lib. See ADR-0025 for the
// full context (why a 5th crate, not a lib+bin split of flintshot-app;
// why not duplicate the framing in the applet).

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]

use std::path::Path;

use color_eyre::eyre;
use flintshot_core::ipc::{socket_path, Request as IpcRequest};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// The tokio local-socket stream type. Re-exported so callers (the
/// daemon's connection handler and the applet's subscriber) never need
/// to name `interprocess` directly.
pub use interprocess::local_socket::tokio::Stream;

/// Maximum payload size accepted on the wire (1 MiB). Guards against a
/// peer that claims a huge length prefix. Moved unchanged from
/// `flintshot-app::ipc_server`.
pub const MAX_FRAME_LEN: usize = 1 << 20;

// ── framing ──────────────────────────────────────────────────────────

/// Read one length-prefixed frame: 4-byte little-endian length, then
/// exactly that many payload bytes. Rejects lengths over
/// [`MAX_FRAME_LEN`]. A short read (truncation / clean disconnect)
/// surfaces as an error, never a panic.
///
/// # Errors
///
/// Returns an error if the length-prefix read is short (truncation or
/// clean disconnect), if the decoded length exceeds [`MAX_FRAME_LEN`],
/// or if the payload read is short.
pub async fn read_frame<R>(r: &mut R) -> eyre::Result<Vec<u8>>
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
///
/// # Errors
///
/// Returns an error if `payload` exceeds [`MAX_FRAME_LEN`], or if any
/// write or flush fails.
pub async fn write_frame<W>(w: &mut W, payload: &[u8]) -> eyre::Result<()>
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
///
/// # Errors
///
/// Returns an error if postcard serialization fails.
pub fn encode_msg<T>(msg: &T) -> eyre::Result<Vec<u8>>
where
    T: serde::Serialize + ?Sized,
{
    postcard::to_allocvec(msg).map_err(|e| eyre::eyre!("postcard encode: {e}"))
}

/// Resolve the OS socket/pipe name for `path`. Unix → a filesystem
/// socket name; Windows → a namespaced pipe name.
///
/// # Errors
///
/// Returns an error if the path can't be converted to a valid socket
/// name (Unix) or pipe name (Windows).
pub fn socket_name(path: &Path) -> eyre::Result<interprocess::local_socket::Name<'_>> {
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

// ── client helpers ───────────────────────────────────────────────────

/// Connect to the daemon at the socket resolved by
/// [`socket_path`](`flintshot_core::ipc::socket_path`) and send one
/// framed `request`. No response is read (fire-and-forget); the
/// daemon's reaction arrives via its side effects (shutdown for `Quit`,
/// a spawned worker + event stream for captures).
///
/// # Errors
///
/// Returns an error if the socket name can't be resolved, the connection
/// can't be established, or the frame can't be encoded or written.
pub async fn send_request(override_path: Option<&Path>, request: &IpcRequest) -> eyre::Result<()> {
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
///
/// # Errors
///
/// Propagates errors from [`send_request`] or a tokio runtime
/// construction failure.
pub fn send_request_blocking(
    override_path: Option<&Path>,
    request: &IpcRequest,
) -> eyre::Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| eyre::eyre!("building IPC client runtime: {e}"))?;
    runtime.block_on(send_request(override_path, request))
}

/// Does this connect error mean "no daemon" (fall back) vs a real fault?
///
/// Used by detect-and-delegate (`flintshot region` / `flintshot
/// fullscreen`) and the worker's report-after-save path to distinguish
/// "no daemon running" (benign — fall back to standalone capture) from a
/// genuine transport failure (propagate).
#[must_use]
pub fn is_no_daemon(error: &std::io::Error) -> bool {
    use std::io::ErrorKind;
    matches!(
        error.kind(),
        ErrorKind::ConnectionRefused | ErrorKind::NotFound | ErrorKind::PermissionDenied
    )
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
}
