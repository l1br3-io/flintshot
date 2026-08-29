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
//! seam so it is unit-testable with fakes; the real zbus-backed bus
//! and the watchdog thread arrive with their caller in `daemon.rs`
//! wiring. Full rationale: ADR-0032.

// Interim: the daemon wiring (this effort's task 002) is the only
// caller; this allow keeps `-D warnings` green until it lands and
// is removed with it.
#![allow(dead_code)]

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Programmable fake: `owned` is `None` to make the probe fail;
    /// `poke_results` are consumed in order, one per poke (`Ok` by
    /// default once exhausted).
    #[derive(Default)]
    struct FakeBus {
        owned: Option<bool>,
        poke_results: std::collections::VecDeque<std::result::Result<(), String>>,
        calls: Vec<String>,
    }

    impl FakeBus {
        fn owned(mut self, owned: bool) -> Self {
            self.owned = Some(owned);
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
}
