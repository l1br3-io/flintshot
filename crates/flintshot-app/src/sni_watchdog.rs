//! SNI watcher watchdog: keep the session's `StatusNotifierWatcher`
//! alive (or get it activated) so the tray icon always has a host.
//!
//! Why this exists: observed live on L5 (COSMIC, 2026-08-30) — the
//! session's watcher service had exited while the daemon kept
//! running, and Flintshot waited passively forever: nothing on our
//! side ever triggers D-Bus activation, so the icon had nowhere to
//! appear and even a daemon restart didn't help. The only
//! client-side activation trigger is a method call against an
//! activatable well-known name (watching `NameOwnerChanged`, what
//! libappindicator does, never activates anything), so this module
//! *pokes* the watcher names whenever `NameHasOwner` reports the
//! watcher unowned. libappindicator handles registration itself the
//! moment a watcher exists — we only own watcher liveness.
//!
//! Decision logic lives in [`watchdog_tick`] behind the [`SniBus`]
//! seam so it is unit-testable with fakes; [`SessionBus`] is the
//! real zbus-backed bus, driven by a dedicated thread whose first
//! probe doubles as the daemon's startup poke. Full rationale:
//! ADR-0032.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use color_eyre::eyre::{self, Result};

/// The SNI rendezvous name. Every live watcher owns it — KDE/GNOME
/// shells under the standard name, COSMIC's status area alongside
/// its vendor name (it's how libappindicator finds the watcher).
/// Not D-Bus-activatable on COSMIC; owned-by-shell on KDE.
const STANDARD_WATCHER_NAME: &str = "org.kde.StatusNotifierWatcher";

/// COSMIC's activatable watcher name. The standard name has no
/// service file there, so activation on COSMIC goes through this
/// vendor name (verified live: a `Peer Ping` against it D-Bus
/// -activated the watcher).
const COSMIC_WATCHER_NAME: &str = "com.system76.CosmicStatusNotifierWatcher";

/// Object path pokes are sent to. Matches the invocation verified
/// live on COSMIC (the Peer interface answers on any path; this is
/// the watcher's real one).
const WATCHER_PATH: &str = "/StatusNotifierWatcher";

/// Probe cadence. Fixed by decision — no config surface; polling
/// faster than 10 s is forbidden by the spec's boundaries anyway.
const PROBE_INTERVAL: Duration = Duration::from_secs(30);

/// Bound on any single poke. Generous enough for D-Bus activation
/// (service spawn) in the common case; a poke that outlives it is
/// not wasted — the bus keeps activating the service and the next
/// probe reaps the result.
const POKE_TIMEOUT: Duration = Duration::from_secs(2);

/// How long [`ensure_watcher`] may wait, after a poke, for the
/// activated watcher to take the name. A successful poke usually
/// implies the name is already up (the ping reply requires the
/// service), so this bound mostly covers the slow-activation edge.
const ACTIVATION_WAIT: Duration = Duration::from_secs(2);

/// Poll step inside [`ensure_watcher`]'s activation wait.
const OWNED_POLL_STEP: Duration = Duration::from_millis(50);

/// The D-Bus surface the watchdog needs, as a sync seam so the
/// decision logic runs against fakes in unit tests.
///
/// Implementations must be cheap to call wrong: every method may
/// fail on a bus-less or tray-less session, and the watchdog treats
/// those as normal steady states (logged, never propagated).
trait SniBus {
    /// `NameHasOwner(org.kde.StatusNotifierWatcher)`, answered by
    /// the bus daemon itself — never triggers activation, which is
    /// why it is safe as the poll gate.
    fn watcher_owned(&mut self) -> color_eyre::eyre::Result<bool>;

    /// One bounded method call against `name` at the SNI watcher
    /// object path — the activation trigger when `name` has a
    /// service file, a harmless round-trip otherwise.
    fn poke(&mut self, name: &str) -> color_eyre::eyre::Result<()>;
}

/// One watchdog policy tick: probe the standard name, and only when
/// unowned poke the standard then the COSMIC name, stopping at the
/// first success. Every failure is a normal steady state on some
/// desktop (tray-less sessions, no session bus) and is logged at
/// `debug` — the watchdog must never be louder than the problem.
fn watchdog_tick(bus: &mut dyn SniBus) {
    match bus.watcher_owned() {
        Ok(true) => {}
        Ok(false) => {
            for name in [STANDARD_WATCHER_NAME, COSMIC_WATCHER_NAME] {
                match bus.poke(name) {
                    Ok(()) => {
                        tracing::debug!(watcher = name, "poke reached the SNI watcher");
                        break;
                    }
                    Err(e) => {
                        tracing::debug!(watcher = name, error = %e, "SNI watcher poke failed");
                    }
                }
            }
        }
        Err(e) => {
            tracing::debug!(error = %e, "SNI watcher probe failed; skipping this tick");
        }
    }
}

/// Real session-bus [`SniBus`] backed by zbus 5.
///
/// Holds one current-thread runtime for its lifetime (the
/// `send_request_blocking` precedent) and opens a fresh connection
/// per probe/poke — nothing stale can outlive a bus restart, and at
/// a 30 s cadence the connect cost is noise. Not unit-tested: real
/// activation is bus-daemon behavior CI cannot simulate; the live
/// path is covered by the manual test matrix instead.
struct SessionBus {
    runtime: tokio::runtime::Runtime,
}

impl SessionBus {
    fn new() -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| eyre::eyre!("building SNI watchdog runtime: {e}"))?;
        Ok(Self { runtime })
    }
}

/// Collapse a bus-layer error into an `eyre::Report`, message-only:
/// the watchdog logs the display form and moves on.
fn bus_err<E: std::fmt::Display>(e: E) -> eyre::Report {
    eyre::eyre!("{e}")
}

impl SniBus for SessionBus {
    fn watcher_owned(&mut self) -> Result<bool> {
        self.runtime.block_on(async {
            let conn = zbus::Connection::session().await.map_err(bus_err)?;
            let dbus = zbus::fdo::DBusProxy::new(&conn).await.map_err(bus_err)?;
            dbus.name_has_owner(
                STANDARD_WATCHER_NAME
                    .try_into()
                    .map_err(|e| eyre::eyre!("well-known name rejected: {e:?}"))?,
            )
            .await
            .map_err(bus_err)
        })
    }

    fn poke(&mut self, name: &str) -> Result<()> {
        self.runtime.block_on(async {
            let conn = zbus::Connection::session().await.map_err(bus_err)?;
            let peer = zbus::fdo::PeerProxy::builder(&conn)
                .destination(name.to_owned())
                .map_err(bus_err)?
                .path(WATCHER_PATH)
                .map_err(bus_err)?
                .build()
                .await
                .map_err(bus_err)?;
            tokio::time::timeout(POKE_TIMEOUT, peer.ping())
                .await
                .map_err(|_| eyre::eyre!("ping of {name} timed out"))
                .and_then(|result| result.map_err(bus_err))
        })
    }
}

/// The watchdog thread body: tick immediately, then once per
/// `interval`, exiting the moment the shutdown sender drops. The
/// daemon's GLib-pumped main loop must never block on a bus call —
/// that constraint is why this is a dedicated thread.
fn watchdog_loop(bus: &mut dyn SniBus, rx: &mpsc::Receiver<()>, interval: Duration) {
    loop {
        watchdog_tick(bus);
        match rx.recv_timeout(interval) {
            Ok(()) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

/// Running watchdog: owns the shutdown sender and the thread's join
/// handle. Dropping without [`Watchdog::shutdown`] also stops the
/// thread (the dropped sender reads as `Disconnected`), just
/// without waiting for it.
pub(crate) struct Watchdog {
    tx: mpsc::Sender<()>,
    join: JoinHandle<()>,
}

impl Watchdog {
    /// Stop the watchdog and wait for the thread to finish. Bounded
    /// by at most one in-flight poke (`POKE_TIMEOUT`).
    pub(crate) fn shutdown(self) {
        // Drop the sender first: joining while holding it would
        // wait for the interval to elapse, not for the thread.
        drop(self.tx);
        if self.join.join().is_err() {
            tracing::warn!("SNI watchdog thread did not shut down cleanly");
        }
    }
}

/// Spawn the SNI watchdog: probes every [`PROBE_INTERVAL`] to
/// re-poke a watcher that dies mid-session. Best-effort by
/// construction: any failure is logged inside the loop and a
/// spawn/builder failure returns `None`, never blocking daemon
/// startup. Startup liveness is *not* this thread's job —
/// [`ensure_watcher`] handles that before the tray is built.
pub(crate) fn spawn() -> Option<Watchdog> {
    spawn_with(PROBE_INTERVAL, SessionBus::new)
}

/// Test seam over [`spawn`]: inject the interval and the bus
/// factory. Production fixes both via consts and [`SessionBus`].
fn spawn_with<B, F>(interval: Duration, make_bus: F) -> Option<Watchdog>
where
    B: SniBus + Send + 'static,
    F: Fn() -> Result<B> + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<()>();
    let builder = thread::Builder::new().name("flintshot-sni-watchdog".into());
    let join = builder
        .spawn(move || {
            let mut bus = match make_bus() {
                Ok(bus) => bus,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "SNI watchdog bus unavailable; tray watcher self-heal disabled"
                    );
                    return;
                }
            };
            watchdog_loop(&mut bus, &rx, interval);
        })
        .map_err(|e| {
            tracing::warn!(error = %e, "spawning the SNI watchdog thread");
            e
        })
        .ok()?;
    Some(Watchdog { tx, join })
}

/// Startup barrier: make sure the SNI watcher is up — activating it
/// if dead — **before** the tray is built, so libappindicator's
/// first registration attempt finds a host. The fire-and-forget
/// variant lost that race live (L5 2026-08-30: the poke-activated
/// watcher appeared *during* appindicator's init; the
/// `NameOwnerChanged` event was missed and the icon never
/// registered until a daemon restart). Bounded and
/// failure-tolerant: a healthy session pays one probe round trip,
/// a tray-less desktop returns as soon as both pokes error, and
/// nothing ever blocks past [`ACTIVATION_WAIT`].
pub(crate) fn ensure_watcher() {
    let mut bus = match SessionBus::new() {
        Ok(bus) => bus,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "SNI startup barrier unavailable; the watchdog thread still covers mid-session"
            );
            return;
        }
    };
    ensure_watcher_with(&mut bus, ACTIVATION_WAIT);
}

/// Test seam over [`ensure_watcher`]. Every branch returns; the
/// tray build proceeds regardless — this barrier may improve the
/// odds, never break the daemon.
fn ensure_watcher_with(bus: &mut dyn SniBus, wait: Duration) {
    if matches!(bus.watcher_owned(), Ok(true)) {
        return;
    }
    let activated = [STANDARD_WATCHER_NAME, COSMIC_WATCHER_NAME]
        .iter()
        .any(|name| bus.poke(name).is_ok());
    if !activated {
        // No activatable service answered — nothing is coming.
        return;
    }
    let deadline = std::time::Instant::now() + wait;
    while std::time::Instant::now() < deadline {
        if matches!(bus.watcher_owned(), Ok(true)) {
            tracing::debug!("SNI watcher confirmed up before tray build");
            return;
        }
        thread::sleep(OWNED_POLL_STEP);
    }
    tracing::debug!(
        "SNI watcher not owned after the activation wait; \
         libappindicator's own name watch covers the rest"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Programmable fake: `owned` is `None` to make the probe fail;
    /// `poke_results` are consumed in order, one per poke (`Ok` by
    /// default once exhausted). `flip_after` makes the probe report
    /// unowned until the Nth call, then owned (activation landing
    /// mid-sequence).
    #[derive(Default)]
    struct FakeBus {
        owned: Option<bool>,
        flip_after: Option<usize>,
        poke_results: std::collections::VecDeque<std::result::Result<(), String>>,
        calls: Vec<String>,
    }

    impl FakeBus {
        fn owned(mut self, owned: bool) -> Self {
            self.owned = Some(owned);
            self
        }

        fn becomes_owned_after(mut self, probes: usize) -> Self {
            self.owned = Some(false);
            self.flip_after = Some(probes);
            self
        }

        fn with_probe_error(mut self) -> Self {
            self.owned = None;
            self
        }

        fn pokes(mut self, results: Vec<std::result::Result<(), String>>) -> Self {
            self.poke_results = results.into();
            self
        }
    }

    impl SniBus for FakeBus {
        fn watcher_owned(&mut self) -> color_eyre::eyre::Result<bool> {
            self.calls.push("probe".into());
            if let Some(remaining) = self.flip_after {
                if remaining == 0 {
                    return Ok(true);
                }
                self.flip_after = Some(remaining - 1);
                return Ok(false);
            }
            self.owned
                .ok_or_else(|| color_eyre::eyre::eyre!("no session bus"))
        }

        fn poke(&mut self, name: &str) -> color_eyre::eyre::Result<()> {
            self.calls.push(format!("poke:{name}"));
            match self.poke_results.pop_front() {
                Some(std::result::Result::Err(e)) => Err(color_eyre::eyre::eyre!(e)),
                Some(std::result::Result::Ok(())) | None => Ok(()),
            }
        }
    }

    fn run(mut fake: FakeBus) -> Vec<String> {
        watchdog_tick(&mut fake);
        fake.calls
    }

    #[test]
    fn owned_watcher_means_no_poke() {
        assert_eq!(run(FakeBus::default().owned(true)), ["probe"]);
    }

    #[test]
    fn unowned_pokes_standard_then_cosmic() {
        let calls = run(FakeBus::default().owned(false).pokes(vec![
            Err("no kde service".into()),
            Err("no cosmic service".into()),
        ]));
        assert_eq!(
            calls,
            [
                "probe",
                "poke:org.kde.StatusNotifierWatcher",
                "poke:com.system76.CosmicStatusNotifierWatcher",
            ]
        );
    }

    #[test]
    fn standard_poke_success_skips_cosmic() {
        // A reachable standard name answered — no reason to try the
        // vendor name too.
        let calls = run(FakeBus::default()
            .owned(false)
            .pokes(vec![Ok(()), Err("must not be reached".into())]));
        assert_eq!(calls, ["probe", "poke:org.kde.StatusNotifierWatcher"]);
    }

    #[test]
    fn standard_poke_failure_falls_through_to_cosmic() {
        let calls = run(FakeBus::default()
            .owned(false)
            .pokes(vec![Err("not activatable".into()), Ok(())]));
        assert_eq!(
            calls,
            [
                "probe",
                "poke:org.kde.StatusNotifierWatcher",
                "poke:com.system76.CosmicStatusNotifierWatcher",
            ]
        );
    }

    #[test]
    fn probe_error_means_no_poke_and_no_panic() {
        assert_eq!(run(FakeBus::default().with_probe_error()), ["probe"]);
    }

    // ── watchdog loop + thread (task 002) ───────────────────────

    use std::sync::Arc;

    #[test]
    fn loop_ticks_immediately_continues_on_heartbeat_exits_on_disconnect() {
        let (tx, rx) = mpsc::channel::<()>();
        tx.send(()).expect("heartbeat");
        tx.send(()).expect("heartbeat");
        drop(tx);

        // Every poke succeeds by default, so one tick = probe + a
        // single standard-name poke.
        let mut fake = FakeBus::default().owned(false);
        watchdog_loop(&mut fake, &rx, Duration::from_millis(1));

        assert_eq!(
            fake.calls,
            [
                "probe",
                "poke:org.kde.StatusNotifierWatcher",
                "probe",
                "poke:org.kde.StatusNotifierWatcher",
                "probe",
                "poke:org.kde.StatusNotifierWatcher",
            ],
            "1 immediate tick + one per heartbeat, then exit on disconnect"
        );
    }

    /// Thread-shared fake: forwards to an inner [`FakeBus`] behind a
    /// mutex so the spawned watchdog thread and the test can both
    /// reach it.
    #[derive(Clone, Default)]
    struct SharedBus(Arc<std::sync::Mutex<FakeBus>>);

    impl SniBus for SharedBus {
        fn watcher_owned(&mut self) -> Result<bool> {
            self.0.lock().expect("fake bus mutex").watcher_owned()
        }

        fn poke(&mut self, name: &str) -> Result<()> {
            self.0.lock().expect("fake bus mutex").poke(name)
        }
    }

    #[test]
    fn spawned_watchdog_probes_before_first_interval_and_stops() {
        let bus = SharedBus::default();
        *bus.0.lock().expect("fake bus mutex") = FakeBus::default().owned(true);

        // A 60 s interval: any probe observed within seconds can
        // only be the immediate first tick, never interval pacing.
        let factory_bus = bus.clone();
        let watchdog =
            spawn_with(Duration::from_mins(1), move || Ok(factory_bus.clone())).expect("spawn");

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let probed = !bus.0.lock().expect("fake bus mutex").calls.is_empty();
            if probed || std::time::Instant::now() > deadline {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(
            !bus.0.lock().expect("fake bus mutex").calls.is_empty(),
            "first tick must run at thread start, before any interval sleep"
        );

        // Shutdown drops the sender — the thread must notice without
        // waiting out the 60 s interval, so this returns promptly.
        let started = std::time::Instant::now();
        watchdog.shutdown();
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "shutdown must wake on disconnect, not wait out the interval"
        );
        assert_eq!(
            bus.0.lock().expect("fake bus mutex").calls,
            ["probe"],
            "owned watcher means the thread probed and never poked"
        );
    }

    // ── startup barrier (task 006) ──────────────────────────────

    #[test]
    fn barrier_returns_immediately_when_watcher_owned() {
        let mut fake = FakeBus::default().owned(true);
        ensure_watcher_with(&mut fake, Duration::from_secs(5));
        assert_eq!(fake.calls, ["probe"]);
    }

    #[test]
    fn barrier_pokes_then_confirms_once_owned() {
        // Activation lands between the first probe and the first
        // confirmation poll.
        let mut fake = FakeBus::default()
            .becomes_owned_after(1)
            .pokes(vec![Ok(())]);
        ensure_watcher_with(&mut fake, Duration::from_secs(5));
        assert_eq!(
            fake.calls,
            ["probe", "poke:org.kde.StatusNotifierWatcher", "probe",]
        );
    }

    #[test]
    fn barrier_returns_without_waiting_when_nothing_is_activatable() {
        // Both pokes fail (tray-less desktop): waiting out the bound
        // would pointlessly delay every daemon start there.
        let mut fake = FakeBus::default()
            .owned(false)
            .pokes(vec![Err("no service".into()), Err("no service".into())]);
        let started = std::time::Instant::now();
        ensure_watcher_with(&mut fake, Duration::from_secs(5));
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "no activatable service must not wait out the bound"
        );
        assert_eq!(fake.calls.len(), 3, "probe + both pokes, nothing more");
    }

    #[test]
    fn barrier_gives_up_at_the_bound_when_never_owned() {
        let mut fake = FakeBus::default().owned(false).pokes(vec![Ok(())]);
        let started = std::time::Instant::now();
        ensure_watcher_with(&mut fake, Duration::from_millis(120));
        assert!(
            started.elapsed() >= Duration::from_millis(120),
            "a successful poke earns the full activation wait"
        );
    }

    #[test]
    fn barrier_tolerates_probe_error_without_panicking() {
        let mut fake = FakeBus::default()
            .with_probe_error()
            .pokes(vec![Err("no bus".into()), Err("no bus".into())]);
        ensure_watcher_with(&mut fake, Duration::from_secs(5));
        assert_eq!(fake.calls.len(), 3);
    }
}
