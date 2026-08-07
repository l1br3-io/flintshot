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
// On disconnect the stream yields [`Message::DaemonGone`] and then —
// unlike a typical iced subscription — *retries* with a bounded backoff
// rather than ending. This is deliberate: iced's [`Subscription`] tracker
// does NOT auto-restart a recipe whose stream has ended (same recipe id
// ⇒ `contains_key` short-circuits respawn — see iced's
// `subscription/tracker.rs` at the locked revision), so a stream that
// ended on `DaemonGone` would silence the applet forever. Looping
// internally with backoff makes the subscription self-healing: when the
// daemon comes back (launched by Task 005's *Start daemon* button, or
// restarted externally), the next connect succeeds, the stream yields
// [`Message::DaemonUp`], and event delivery resumes — no applet restart
// needed. The backoff (250 ms → 1 s) bounds the retry rate so a dead
// socket isn't hot-looped.

use std::time::Duration;

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

/// Initial backoff after a failed/rejected connect attempt.
const INITIAL_BACKOFF: Duration = Duration::from_millis(250);

/// Cap for the exponential backoff. One retry per second against a
/// genuinely-absent daemon is gentle on the socket layer while keeping
/// *Start daemon* recovery latency near the spec's ~1 s manual target
/// (worst case: one backoff tick + the daemon's own bind time).
const MAX_BACKOFF: Duration = Duration::from_secs(1);

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
/// The stream runs for the applet's lifetime. Each iteration runs one
/// [`run_session`] (a single connect → subscribe → read-until-error
/// pass) and, on failure, backs off and retries:
///
/// 1. [`run_session`] resolves [`socket_path`]`(None)`, connects, writes
///    a postcard-encoded `Request::Subscribe`, yields
///    [`Message::DaemonUp`], then loops [`read_frame`] → `Event` →
///    [`Message::IpcEvent`] until the socket errors / hits clean EOF.
/// 2. On any failure (no socket file, connect refused, write/read
///    error) the session returns without having emitted `DaemonUp`
///    (or having emitted it then lost the socket).
/// 3. The outer loop emits [`Message::DaemonGone`] **once per
///    gone-episode** (a flag dedups repeat emissions across backoff
///    retries so the UI/log isn't spammed), sleeps `backoff`, and
///    retries. Backoff resets to [`INITIAL_BACKOFF`] once a session
///    connects, and doubles up to [`MAX_BACKOFF`] on each consecutive
///    failure.
#[must_use]
pub fn subscriber_stream() -> cosmic::iced::futures::stream::BoxStream<'static, Message> {
    Box::pin(stream::channel(8, |mut sender: mpsc::Sender<Message>| {
        async move {
            let mut backoff = INITIAL_BACKOFF;
            // Emit DaemonGone once per gone-episode: true while we've
            // already reported the loss and are backing off. Reset to
            // false when a session connects, so a *later* loss emits a
            // fresh DaemonGone.
            let mut reported_gone = false;

            loop {
                let connected = run_session(&mut sender).await;

                match connected {
                    SessionOutcome::Connected => {
                        // The daemon answered at some point (DaemonUp was
                        // emitted). Reset so the next loss reports again,
                        // and restart backoff from the floor.
                        reported_gone = false;
                        backoff = INITIAL_BACKOFF;
                    }
                    SessionOutcome::NeverConnected => {
                        // Never reached DaemonUp this attempt; keep the
                        // existing suppression flag and grow the backoff.
                    }
                }

                if !reported_gone {
                    let _ = sender.send(Message::DaemonGone).await;
                    reported_gone = true;
                }

                tokio::time::sleep(backoff).await;
                backoff = backoff.saturating_mul(2).min(MAX_BACKOFF);
            }
        }
    }))
}

/// Outcome of a single connect → subscribe → read pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SessionOutcome {
    /// The session connected and emitted [`Message::DaemonUp`] (it then
    /// streamed events until the socket was lost). The outer loop should
    /// emit a fresh [`Message::DaemonGone`] for the loss.
    Connected,
    /// The session never reached `DaemonUp` (no socket file, connect
    /// refused, or the `Subscribe` write failed). The outer loop should
    /// treat this as a continued absence and back off.
    NeverConnected,
}

/// Run one connect → subscribe → read-until-error pass against the
/// daemon, forwarding messages through `sender`.
///
/// Emits [`Message::DaemonUp`] right after the `Subscribe` frame is
/// written (before the first read blocks) so the UI flips to `Connected`
/// immediately, then streams [`Message::IpcEvent`]s. On any failure
/// returns without logging a per-attempt warning for the routine
/// no-daemon case (`is_no_daemon`); non-no-daemon errors are logged.
async fn run_session(sender: &mut mpsc::Sender<Message>) -> SessionOutcome {
    let path = socket_path(None);

    // Fast path: no socket file → no daemon. On Unix, connecting to a
    // missing socket path would give ENOENT (also caught by
    // is_no_daemon), but the pre-check avoids the syscall.
    #[cfg(unix)]
    if !path.exists() {
        return SessionOutcome::NeverConnected;
    }

    let name = match socket_name(&path) {
        Ok(n) => n,
        Err(e) => {
            tracing::warn!(error = %e, "IPC socket name resolution failed");
            return SessionOutcome::NeverConnected;
        }
    };

    let mut conn = match Stream::connect(name).await {
        Ok(c) => c,
        Err(e) => {
            if !is_no_daemon(&e) {
                tracing::warn!(error = %e, "IPC connect (non-no-daemon error)");
            }
            return SessionOutcome::NeverConnected;
        }
    };

    let bytes = match encode_msg(&Request::Subscribe) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "encoding Subscribe request");
            return SessionOutcome::NeverConnected;
        }
    };
    if let Err(e) = write_frame(&mut conn, &bytes).await {
        tracing::warn!(error = %e, "writing Subscribe frame");
        return SessionOutcome::NeverConnected;
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
                tracing::debug!(error = %e, "IPC subscriber session ended");
                return SessionOutcome::Connected;
            }
        }
    }
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

    #[test]
    fn backoff_constants_are_ordered_and_bounded() {
        // Guards the recovery-latency claim in the module docs: the cap
        // must exceed the floor and stay within the ~1 s target band.
        assert!(INITIAL_BACKOFF < MAX_BACKOFF);
        assert!(MAX_BACKOFF <= Duration::from_secs(1));
    }
}
