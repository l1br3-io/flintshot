//! CLI → daemon delegation. `flintshot region` / `fullscreen` try to
//! hand the capture to a running daemon (centralising the `Event`
//! stream for subscribers); if no daemon answers, they fall back to the
//! Phase 1 in-process capture+editor path unchanged.
//!
//! Transport reuses [`crate::ipc_server`]'s framing + socket-name
//! helpers so the wire format stays identical.

use std::path::Path;

use color_eyre::eyre;
use flintshot_core::ipc::{socket_path, Event as IpcEvent, Request};

use crate::ipc_server::{decode_event, encode_msg, socket_name, write_frame, Stream};

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
    // A one-shot current-thread runtime — the CLI isn't otherwise in a
    // tokio context.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| eyre::eyre!("building delegation runtime: {e}"))?;

    runtime.block_on(async move { delegate_async(override_path, request).await })
}

/// Async core of [`delegate_to`], for callers already inside a
/// reactor (the app window's pump thread).
pub(crate) async fn delegate_async(
    override_path: Option<&Path>,
    request: &Request,
) -> eyre::Result<DelegateOutcome> {
    let path = socket_path(override_path);
    if !path.exists() {
        return Ok(DelegateOutcome::NoDaemon);
    }

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
}

/// Connect to the daemon at `override_path` (or the default socket),
/// send `Subscribe`, and return the stream to read [`IpcEvent`]s
/// from. The daemon sends no ack — the connection becomes a pure
/// event sink, so the first `read_frame` blocks until the next
/// broadcast. Returns `Ok(None)` when no daemon answers.
///
/// For the app window's long-lived pump thread; callers drive
/// [`crate::ipc_server::read_frame`] on the returned stream inside
/// their own reactor.
pub(crate) async fn subscribe_to(override_path: Option<&Path>) -> eyre::Result<Option<Stream>> {
    let path = socket_path(override_path);
    if !path.exists() {
        return Ok(None);
    }

    let name = socket_name(&path)?;
    match <Stream as interprocess::local_socket::traits::tokio::Stream>::connect(name).await {
        Ok(mut stream) => {
            let bytes = encode_msg(&Request::Subscribe)?;
            write_frame(&mut stream, &bytes).await?;
            Ok(Some(stream))
        }
        Err(e) if is_no_daemon(&e) => Ok(None),
        Err(e) => Err(eyre::eyre!(
            "connecting to daemon at {}: {e}",
            path.display()
        )),
    }
}

/// Does this connect error mean "no daemon" (fall back) vs a real fault?
fn is_no_daemon(error: &std::io::Error) -> bool {
    use std::io::ErrorKind;
    matches!(
        error.kind(),
        ErrorKind::ConnectionRefused | ErrorKind::NotFound | ErrorKind::PermissionDenied
    )
}

/// Send `Request::BindShortcuts` to the daemon at `override_path` (or
/// the default socket) and await its one-frame reply — the portal bind
/// dialog blocks until the user finishes, so this is a request/reply
/// round-trip, not a fire-and-forget delegate. Returns `Ok(None)` when
/// no daemon answers (the CLI's "start the daemon first" error).
pub(crate) fn bind_shortcuts_to(override_path: Option<&Path>) -> eyre::Result<Option<IpcEvent>> {
    let path = socket_path(override_path);
    if !path.exists() {
        return Ok(None);
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| eyre::eyre!("building bind runtime: {e}"))?;

    runtime.block_on(async move {
        let name = socket_name(&path)?;
        let mut stream =
            match <Stream as interprocess::local_socket::traits::tokio::Stream>::connect(name).await
            {
                Ok(s) => s,
                Err(e) if is_no_daemon(&e) => return Ok(None),
                Err(e) => {
                    return Err(eyre::eyre!(
                        "connecting to daemon at {}: {e}",
                        path.display()
                    ))
                }
            };
        let bytes = encode_msg(&Request::BindShortcuts)?;
        write_frame(&mut stream, &bytes).await?;
        let payload = crate::ipc_server::read_frame(&mut stream).await?;
        Ok(Some(decode_event(&payload)?))
    })
}

/// Outcome of asking the daemon to hide its tray for an imminent
/// capture (see [`hide_tray_for_capture_to`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HideTrayOutcome {
    /// No daemon answered: nothing was registered server-side, so no
    /// `ReportCaptured` is owed — proceed and grab immediately.
    NoDaemon,
    /// The daemon registered the in-flight capture. `acked` says
    /// whether its `Event::TrayHidden` reply arrived inside the
    /// bound — the caller may grab either way (an unconfirmed hide is
    /// the pre-fix behaviour, not a failure); a `ReportCaptured` is
    /// owed the moment the grab finishes.
    Registered { acked: bool },
}

/// How long the worker waits for the daemon's `TrayHidden` ack before
/// capturing anyway. Deliberately shorter than the daemon's own
/// confirm bound so the worker — the thing the user is staring at —
/// is never the one left hanging.
const TRAY_REPLY_BOUND: std::time::Duration = std::time::Duration::from_secs(2);

/// Ask the daemon at `override_path` (or the default socket) to hide
/// its tray icon for an imminent capture, and await the ack —
/// bounded by [`TRAY_REPLY_BOUND`]: a slow daemon must not stall the
/// capture. Mirrors [`bind_shortcuts_to`]'s round-trip shape with a
/// timeout on top; see ADR-0036.
pub(crate) fn hide_tray_for_capture_to(
    override_path: Option<&Path>,
) -> eyre::Result<HideTrayOutcome> {
    let path = socket_path(override_path);
    if !path.exists() {
        return Ok(HideTrayOutcome::NoDaemon);
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| eyre::eyre!("building tray-hide runtime: {e}"))?;

    runtime.block_on(async move {
        let name = socket_name(&path)?;
        let mut stream =
            match <Stream as interprocess::local_socket::traits::tokio::Stream>::connect(name).await
            {
                Ok(s) => s,
                Err(e) if is_no_daemon(&e) => return Ok(HideTrayOutcome::NoDaemon),
                Err(e) => {
                    return Err(eyre::eyre!(
                        "connecting to daemon at {}: {e}",
                        path.display()
                    ))
                }
            };
        let bytes = encode_msg(&Request::HideTrayForCapture)?;
        write_frame(&mut stream, &bytes).await?;
        match tokio::time::timeout(TRAY_REPLY_BOUND, crate::ipc_server::read_frame(&mut stream))
            .await
        {
            Ok(Ok(payload)) => {
                decode_event(&payload)?;
                Ok(HideTrayOutcome::Registered { acked: true })
            }
            Ok(Err(e)) => Err(e),
            Err(_) => {
                tracing::warn!(
                    "daemon did not confirm the tray hide within {TRAY_REPLY_BOUND:?}; \
                     capturing anyway (the icon may appear in the shot)"
                );
                Ok(HideTrayOutcome::Registered { acked: false })
            }
        }
    })
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

    #[test]
    fn bind_shortcuts_absent_socket_reports_no_daemon() {
        let reply = bind_shortcuts_to(Some(Path::new(
            "/this/does/not/exist/flintshot-test-bind.sock",
        )))
        .expect("absent path should not error");
        assert!(reply.is_none(), "no daemon → None reply");
    }

    #[test]
    fn hide_tray_absent_socket_reports_no_daemon() {
        let outcome = hide_tray_for_capture_to(Some(Path::new(
            "/this/does/not/exist/flintshot-test-hide.sock",
        )))
        .expect("absent path should not error");
        assert_eq!(
            outcome,
            HideTrayOutcome::NoDaemon,
            "no daemon → nothing hidden, nothing owed"
        );
    }
}
