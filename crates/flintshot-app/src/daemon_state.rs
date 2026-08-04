//! Daemon-wide shared state, accessible from the IPC server's
//! per-connection handler and (in Task 004) the worker dispatch path.
//!
//! [`DaemonState`] is the container the daemon constructs once at startup
//! and shares — by cloning, cheaply, since every field is behind an
//! `Arc` — between the tokio-side IPC server and the main-thread
//! tray/hotkey dispatch loop. It holds the [`SubscriberBus`] (fan-out for
//! daemon → subscriber [`Event`]s) and the `last_saved` path (worker →
//! daemon feedback via [`Request::ReportSaved`], read later by
//! [`Request::PinLast`]).
//!
//! New cross-thread state that both halves need should land here rather
//! than as ad-hoc parameters threaded through `IpcServer::serve` /
//! `handle_connection`, so Task 004's pin-id counter and any future
//! shared field have an obvious home.
//!
//! [`Event`]: flintshot_core::ipc::Event
//! [`Request::ReportSaved`]: flintshot_core::ipc::Request::ReportSaved
//! [`Request::PinLast`]: flintshot_core::ipc::Request::PinLast

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use flintshot_core::ipc::Event;

use crate::ipc_server::SubscriberBus;

/// Shared daemon state, cloneable for cheap handoff between the IPC
/// server's per-connection tasks and the main-thread dispatch loop.
#[derive(Clone)]
pub struct DaemonState {
    /// Fan-out for daemon → subscriber [`Event`]s. Shared between the
    /// IPC server (which fans events to IPC subscribers) and the
    /// tray/hotkey dispatch path, so a capture started from any source
    /// reaches every subscriber.
    ///
    /// [`Event`]: flintshot_core::ipc::Event
    pub bus: SubscriberBus,
    /// The most recently worker-reported save path, or `None` until the
    /// first save arrives. Written by the IPC server's
    /// `Request::ReportSaved` handler; read by `Request::PinLast`
    /// `Request::PinLast` to re-pin the last capture.
    ///
    /// `Mutex` over `RwLock`: the write is a single `replace` and reads
    /// are rare (only on `PinLast`); the lock is held for microseconds.
    last_saved: Arc<Mutex<Option<PathBuf>>>,
    /// Per-daemon monotonic pin id counter, read by `PinLast` to tag
    /// [`Event::Pinned { id }`]. Relaxed ordering is sufficient — the id
    /// only needs to be unique within this daemon run (it resets on
    /// restart; it's a handle for a future "close pin N", not a globally
    /// unique key).
    ///
    /// [`Event::Pinned { id }`]: flintshot_core::ipc::Event::Pinned
    next_pin_id: Arc<AtomicU64>,
}

impl DaemonState {
    /// Fresh state: empty subscriber bus, no last-saved path, pin ids
    /// start at 0.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bus: SubscriberBus::new(),
            last_saved: Arc::new(Mutex::new(None)),
            next_pin_id: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Build state from an existing bus (the daemon shares one bus between
    /// the IPC server and the tray/hotkey path) plus a fresh
    /// `last_saved`.
    //
    // Used by tests that share a bus handle; production code uses
    // `DaemonState::new` (the daemon owns the state end-to-end).
    #[allow(dead_code)]
    #[must_use]
    pub fn with_bus(bus: SubscriberBus) -> Self {
        Self {
            bus,
            last_saved: Arc::new(Mutex::new(None)),
            next_pin_id: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Record a worker-reported save: replace `last_saved` and broadcast
    /// [`Event::Saved`] to subscribers. Called from the IPC server's
    /// `Request::ReportSaved` handler. Idempotent in effect aside from
    /// replacing the prior path.
    ///
    /// # Panics
    /// Only on a poisoned mutex (a panic in a concurrent holder), which
    /// indicates a bug, not a normal runtime condition.
    ///
    /// [`Event::Saved`]: flintshot_core::ipc::Event::Saved
    pub fn record_save(&self, path: PathBuf) {
        let event = Event::Saved { path: path.clone() };
        *self.last_saved.lock().expect("last_saved mutex poisoned") = Some(path);
        self.bus.broadcast(event);
    }

    /// The most recently worker-reported save path, or `None` if no save
    /// has arrived yet. Clones the path (callers are infrequent —
    /// `PinLast` is the only current reader). The lock is released
    /// before return, so the caller never holds it across a spawn.
    ///
    /// # Panics
    /// Only on a poisoned mutex.
    #[must_use]
    pub fn last_saved(&self) -> Option<PathBuf> {
        self.last_saved
            .lock()
            .expect("last_saved mutex poisoned")
            .clone()
    }

    /// Allocate the next monotonic pin id. Used by `PinLast` to tag
    /// [`Event::Pinned { id }`]. Ids start at 0 and increment for each
    /// pin the daemon spawns; they reset on daemon restart and are not
    /// globally unique — they're a per-daemon handle for a future "close
    /// pin N" feature.
    ///
    /// [`Event::Pinned { id }`]: flintshot_core::ipc::Event::Pinned
    #[must_use]
    pub fn next_pin_id(&self) -> u64 {
        self.next_pin_id.fetch_add(1, Ordering::Relaxed)
    }
}

impl Default for DaemonState {
    fn default() -> Self {
        Self::new()
    }
}

/// Read the `path` out of a `Request::ReportSaved`, returning `None` for
/// any other variant. Keeps the pattern-match over [`Request`] in one
/// place (`handle_connection`) and gives the unit tests a pure seam.
///
/// [`Request`]: flintshot_core::ipc::Request
#[must_use]
pub fn extract_report_saved_path(request: &flintshot_core::ipc::Request) -> Option<&Path> {
    match request {
        flintshot_core::ipc::Request::ReportSaved { path } => Some(path),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flintshot_core::ipc::{Event, Request};

    /// `record_save` replaces `last_saved` (even when called more than
    /// once) and broadcasts `Event::Saved` with the same path to every
    /// subscriber on the bus.
    #[tokio::test]
    async fn record_save_stores_path_and_broadcasts_event() {
        let state = DaemonState::new();
        let mut rx = state.bus.subscribe();

        let path = PathBuf::from("/tmp/flintshot-test.png");
        state.record_save(path.clone());

        assert_eq!(state.last_saved(), Some(path.clone()));
        let event = rx.recv().await.expect("subscriber should receive event");
        assert_eq!(event, Event::Saved { path });
    }

    /// A second `record_save` replaces the prior path — `last_saved`
    /// tracks the most recent save, not an append-only history.
    #[tokio::test]
    async fn record_save_replaces_prior_path() {
        let state = DaemonState::new();
        let _ = state.bus.subscribe(); // keep the broadcast non-empty

        state.record_save(PathBuf::from("/tmp/first.png"));
        state.record_save(PathBuf::from("/tmp/second.png"));

        assert_eq!(
            state.last_saved(),
            Some(PathBuf::from("/tmp/second.png")),
            "second save must replace the first"
        );
    }

    /// Fresh state has no last-saved path.
    #[test]
    fn fresh_state_has_no_last_saved() {
        let state = DaemonState::new();
        assert_eq!(state.last_saved(), None);
    }

    /// `extract_report_saved_path` returns the path for `ReportSaved`
    /// and `None` for every other variant (proves the handler's guard
    /// won't mis-route sibling requests).
    #[test]
    fn extract_report_saved_path_only_matches_report_saved() {
        let saved = Request::ReportSaved {
            path: PathBuf::from("/tmp/x.png"),
        };
        assert_eq!(
            extract_report_saved_path(&saved),
            Some(Path::new("/tmp/x.png"))
        );

        let others = [
            Request::CaptureRegion {
                monitor: None,
                delay_ms: 0,
            },
            Request::CaptureFullscreen { monitor: None },
            Request::Quit,
            Request::Subscribe,
            Request::OpenLast,
            Request::PinLast,
            Request::UploadLast {
                backend: "imgur".into(),
            },
        ];
        for req in others {
            assert!(
                extract_report_saved_path(&req).is_none(),
                "`{req:?}` must not look like a ReportSaved"
            );
        }
    }

    /// A bus built via `with_bus` shares its broadcasts with the
    /// caller's existing handle (the daemon relies on this so tray /
    /// hotkey-initiated captures reach IPC subscribers).
    #[tokio::test]
    async fn with_bus_shares_the_subscriber_bus() {
        let bus = SubscriberBus::new();
        let mut rx = bus.subscribe();
        let state = DaemonState::with_bus(bus);

        // record_save on the state must reach the receiver that
        // subscribed through the original bus handle.
        state.record_save(PathBuf::from("/tmp/shared.png"));
        let event = rx.recv().await.expect("original bus handle should receive");
        assert_eq!(
            event,
            Event::Saved {
                path: PathBuf::from("/tmp/shared.png")
            }
        );
    }

    /// Prove the broadcast is a no-op (not an error) when there are no
    /// subscribers — the worker may report before any client subscribes.
    #[test]
    fn record_save_with_no_subscribers_does_not_panic() {
        let state = DaemonState::new();
        state.record_save(PathBuf::from("/tmp/lonely.png"));
        assert_eq!(state.last_saved(), Some(PathBuf::from("/tmp/lonely.png")));
    }

    /// `next_pin_id` returns monotonically increasing ids, starting at 0.
    /// Each call hands out a distinct value — the daemon uses these to
    /// tag `Event::Pinned { id }` so a future "close pin N" can address
    /// a specific pin.
    #[test]
    fn next_pin_id_is_monotonic_from_zero() {
        let state = DaemonState::new();
        assert_eq!(state.next_pin_id(), 0, "first id must be 0");
        assert_eq!(state.next_pin_id(), 1);
        assert_eq!(state.next_pin_id(), 2);
    }

    /// Clones of `DaemonState` share the pin-id counter — the IPC server
    /// thread and the dispatch loop see the same sequence.
    #[test]
    fn next_pin_id_is_shared_across_clones() {
        let state = DaemonState::new();
        let clone = state.clone();
        assert_eq!(state.next_pin_id(), 0);
        assert_eq!(clone.next_pin_id(), 1, "clone must see the increment");
        assert_eq!(state.next_pin_id(), 2);
    }
}
