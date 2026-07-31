//! CLI → daemon delegation. `flintshot region` / `fullscreen` try to
//! hand the capture to a running daemon (centralising the `Event`
//! stream for subscribers); if no daemon answers, they fall back to the
//! Phase 1 in-process capture+editor path unchanged.
//!
//! Transport reuses [`crate::ipc_server`]'s framing + socket-name
//! helpers so the wire format stays identical.

use std::path::Path;

use color_eyre::eyre;
use flintshot_core::ipc::{socket_path, Request};

use crate::ipc_server::{encode_msg, socket_name, write_frame, Stream};

/// The result of trying to delegate a capture to the daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DelegateOutcome {
    /// The daemon accepted the request; it spawns the worker and emits
    /// the `Event` stream. The CLI's job is done.
    Delegated,
    /// No daemon is reachable (no socket / connection refused) — the
    /// caller should run the standalone capture path.
    NoDaemon,
}

/// Try to delegate `request` to a running daemon at the default socket.
pub(crate) fn try_delegate(request: &Request) -> eyre::Result<DelegateOutcome> {
    delegate_to(None, request)
}

/// Testable seam: [`try_delegate`] against an explicit socket path.
///
/// `NoDaemon` is returned when the socket file is absent or the connect
/// is refused / not-found (a stale socket from a crashed daemon). Other
/// errors propagate.
pub(crate) fn delegate_to(
    override_path: Option<&Path>,
    request: &Request,
) -> eyre::Result<DelegateOutcome> {
    let path = socket_path(override_path);
    if !path.exists() {
        return Ok(DelegateOutcome::NoDaemon);
    }

    // A one-shot current-thread runtime — the CLI isn't otherwise in a
    // tokio context.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| eyre::eyre!("building delegation runtime: {e}"))?;

    runtime.block_on(async move {
        let name = socket_name(&path)?;
        match <Stream as interprocess::local_socket::traits::tokio::Stream>::connect(name).await {
            Ok(mut stream) => {
                let bytes = encode_msg(request)?;
                write_frame(&mut stream, &bytes).await?;
                Ok(DelegateOutcome::Delegated)
            }
            Err(e) if is_no_daemon(&e) => Ok(DelegateOutcome::NoDaemon),
            Err(e) => Err(eyre::eyre!(
                "connecting to daemon at {}: {e}",
                path.display()
            )),
        }
    })
}

/// Does this connect error mean "no daemon" (fall back) vs a real fault?
fn is_no_daemon(error: &std::io::Error) -> bool {
    use std::io::ErrorKind;
    matches!(
        error.kind(),
        ErrorKind::ConnectionRefused | ErrorKind::NotFound | ErrorKind::PermissionDenied
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use flintshot_core::ipc::Request;

    #[test]
    fn absent_socket_is_no_daemon() {
        // A path that doesn't exist → immediate NoDaemon, no network.
        let outcome = delegate_to(
            Some(Path::new("/this/does/not/exist/flintshot-test.sock")),
            &Request::CaptureFullscreen { monitor: None },
        )
        .expect("absent path should not error");
        assert_eq!(outcome, DelegateOutcome::NoDaemon);
    }
}
