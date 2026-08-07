//! IPC subscriber bridge: connects to the daemon's socket, sends
//! `Request::Subscribe`, and forwards decoded `Event`s as
//! [`Message`](crate::app::Message)s via an iced [`Subscription`].
//!
//! The subscription is alive from applet startup for its entire lifetime
//! (never gated on popup state).
// RATIONALE: subscribing at startup — not popup-open — closes the
// subscribe/click race (spec Q4): the stream is listening before the
// user can click a capture button, so `CaptureStarted` can't be missed.
// No request/response round-trip was added to the protocol; the
// subscription is a one-way `Event` fan-out.
//
// On disconnect the stream yields [`Message::DaemonGone`] and ends.
// iced's [`Tracker`](cosmic::iced::Subscription) keeps the recipe entry
// alive (same id) so the stream is not auto-restarted on every `update`
// — that would busy-loop against a dead socket. Task 005's *Start
// daemon* button drives the re-probe.

use cosmic::iced::futures::channel::mpsc;
use cosmic::iced::futures::SinkExt;
use cosmic::iced::stream;
use cosmic::iced::Subscription;
use flintshot_core::ipc::{socket_path, Event, Request};
use flintshot_ipc::{encode_msg, is_no_daemon, read_frame, socket_name, write_frame, Stream};
use interprocess::local_socket::traits::tokio::Stream as _;

use crate::app::Message;

/// Stable subscription identity. iced's runtime hashes recipes to dedup
/// across `subscription()` re-calls (the runtime calls `subscription()`
// GOTCHA: on every `update`). A constant `&'static str` id keeps exactly
// one reader stream alive — without it, the runtime would spawn a fresh
// connect attempt on every message, racing the socket.
const SUBSCRIPTION_ID: &str = "flintshot-cosmic-ipc-subscriber";

/// The iced [`Subscription`] wrapping [`subscriber_stream`].
///
/// Returns exactly one recipe (see [`Subscription::units`]) so the
/// runtime keeps a single reader alive.
pub fn subscription() -> Subscription<Message> {
    Subscription::run_with(SUBSCRIPTION_ID, |_| subscriber_stream())
}

/// The raw subscriber stream, boxed so the integration test can name
/// the type and drive it with [`StreamExt::next`] without iced's
/// recipe/event-stream machinery.
///
/// Stream lifecycle:
/// 1. Resolve [`socket_path`]`(None)`. On Unix, an absent socket file
///    yields [`Message::DaemonGone`] immediately (avoids a syscall).
/// 2. Connect to the daemon. On any connect error, yield
///    [`Message::DaemonGone`] and end.
/// 3. Write a postcard-encoded `Request::Subscribe` frame.
/// 4. Yield [`Message::DaemonUp`] *before* the first read blocks — the
///    UI flips to `Connected` immediately.
/// 5. Loop: [`read_frame`] → `postcard::from_bytes::<Event>` → yield
///    [`Message::IpcEvent`]. On read error / clean EOF, yield
///    [`Message::DaemonGone`] and end.
#[must_use]
pub fn subscriber_stream() -> cosmic::iced::futures::stream::BoxStream<'static, Message> {
    Box::pin(stream::channel(8, |mut sender: mpsc::Sender<Message>| {
        async move {
            let path = socket_path(None);

            // Fast path: no socket file → no daemon. On Unix, connecting
            // to a missing socket path would give ENOENT (also caught
            // by is_no_daemon), but the pre-check avoids the syscall.
            #[cfg(unix)]
            if !path.exists() {
                let _ = sender.send(Message::DaemonGone).await;
                return;
            }

            let name = match socket_name(&path) {
                Ok(n) => n,
                Err(e) => {
                    tracing::warn!(error = %e, "IPC socket name resolution failed");
                    let _ = sender.send(Message::DaemonGone).await;
                    return;
                }
            };

            let mut conn = match Stream::connect(name).await {
                Ok(c) => c,
                Err(e) => {
                    if !is_no_daemon(&e) {
                        tracing::warn!(error = %e, "IPC connect (non-no-daemon error)");
                    }
                    let _ = sender.send(Message::DaemonGone).await;
                    return;
                }
            };

            let bytes = match encode_msg(&Request::Subscribe) {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(error = %e, "encoding Subscribe request");
                    let _ = sender.send(Message::DaemonGone).await;
                    return;
                }
            };
            if let Err(e) = write_frame(&mut conn, &bytes).await {
                tracing::warn!(error = %e, "writing Subscribe frame");
                let _ = sender.send(Message::DaemonGone).await;
                return;
            }

            // Daemon is reachable — flip the UI before the first event.
            let _ = sender.send(Message::DaemonUp).await;

            loop {
                match read_frame(&mut conn).await {
                    Ok(payload) => match postcard::from_bytes::<Event>(&payload) {
                        Ok(event) => {
                            let _ = sender.send(Message::IpcEvent(event)).await;
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "decoding Event frame; skipping");
                        }
                    },
                    Err(e) => {
                        tracing::debug!(error = %e, "IPC subscriber stream ended");
                        let _ = sender.send(Message::DaemonGone).await;
                        return;
                    }
                }
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_has_exactly_one_recipe() {
        let sub = subscription();
        assert_eq!(
            sub.units(),
            1,
            "exactly one subscriber recipe — multiple would race the socket"
        );
    }
}
