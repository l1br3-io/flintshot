//! Daemon-side tray gating for captures (ADR-0036/0037/0042).
//!
//! A capture worker hides the daemon's tray for exactly the grab
//! window. Since ADR-0042 the tray is a supervised helper process
//! and "hide" means **stop it** — the process exit is the only
//! causal SNI unregistration, which is what destroys the panel's
//! open menu popup — while "show" respawns it. The handshake is
//! worker-driven — it is the only process that knows when the grab
//! happens:
//!
//! 1. Worker sends `Request::HideTrayForCapture`; the IPC layer
//!    calls [`TrayGate::begin_capture`] and awaits the confirm epoch
//!    ([`TrayGate::wait_confirmed`]).
//! 2. The daemon's main loop steps the gate ([`TrayGate::step`]),
//!    stops the helper, and — once a propagation grace has let the
//!    exit unregister and the panel re-render — bumps the epoch,
//!    releasing the ack.
//! 3. Worker sends `Request::ReportCaptured` after the grab
//!    (success or failure); the last in-flight capture's report
//!    respawns the helper. A worker that dies between the two is
//!    covered by the safety bound: the loop force-restores.
//!
//! The helper's own capture-click exits arrive as an unexpected
//! child exit, which the supervisor feeds back as [`TrayGate::pre_hide`]
//! — side effect only, no depth: the worker's handshake owns the
//! depth, so one capture is never counted twice. The pre-hide holds
//! through a handshake window (spawn + connect latency) before a
//! depth of zero restores it, so a click's exit survives until its
//! worker's handshake arrives.
//!
//! Decisions live in [`TrayVisibility`] — pure, injected-clock,
//! unit-tested. [`TrayGate`] is the thread-safe shell (IPC thread +
//! main loop) around it; process side effects are applied by the
//! daemon from [`TrayAction`]s, never here.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long after hiding before the hide is "confirmed" (the ack may
/// leave): the panel removes the SNI item asynchronously, so an ack
/// that races the repaint would let the worker grab the icon's
/// ghost. Panels re-render within a frame or two; 250 ms is ~an
/// order of magnitude of headroom.
const HIDE_GRACE: Duration = Duration::from_millis(250);

/// Grace for **tray-menu-launched** captures, anchored at the click:
/// the popup's close is panel-owned and unobservable — no API queries
/// another process's popup state — and some panels (COSMIC) never
/// close it on activation at all (an upstream `TODO`), so the ack
/// waits this out blind (ADR-0037/0039). Only the icon-grace applies
/// to hotkey / IPC / applet launches (no menu involved), and only
/// tray clicks pay it.
const MENU_GRACE: Duration = Duration::from_millis(600);

/// How long a pre-hidden tray stays hidden with no handshake
/// arrived: worker spawn + connect + request is ~tens of ms; past
/// half a second the spawn failed and the icon returns.
const HANDSHAKE_WINDOW: Duration = Duration::from_millis(500);

/// Hard bound on the icon staying hidden: a worker killed between
/// its hide request and its capture report must not ghost the tray
/// for the rest of the session. Generous enough to survive a
/// dawdled first-capture portal permission dialog (the common slow
/// path) — a capture that legitimately takes longer re-hides nothing
/// (the pixels are the problem, not the timing).
const SAFETY_RESTORE: Duration = Duration::from_secs(30);

/// How long the IPC layer waits on the main loop's confirmation
/// before acking anyway. The worker's own reply wait is shorter and
/// authoritative; this only keeps the connection tidy.
const CONFIRM_TIMEOUT: Duration = Duration::from_secs(3);

/// Side effect the daemon's main loop applies after a
/// [`TrayGate::step`] transition. The gate decides *when*; the
/// daemon owns *how* — since ADR-0042 that is helper-process
/// lifecycle, not visibility flips: `Hide` stops the `flintshot
/// tray` helper (its exit unregisters the SNI item — the only
/// causal way to close the panel's open popup) and `Show` respawns
/// it (a fresh process = a fresh bus name, so no ADR-0039-style
/// duplicate can accumulate).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrayAction {
    /// Hide the tray: stop the helper process (SNI unregistration —
    /// destroys the open popup and removes the icon).
    Hide,
    /// Set the tray visible again: respawn the helper.
    Show,
    /// Safety-expired restore — like [`TrayAction::Show`], plus the
    /// daemon logs why (a capture worker likely died mid-grab).
    ForceShow,
}

/// Why the daemon is pre-hiding at dispatch time — decides which
/// grace the hide confirms under (see [`TrayVisibility::pre_hide`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreHideOrigin {
    /// A tray-menu click: the helper exited at the click, and the
    /// exit → unregistration → popup-destroy chain (plus any panel
    /// close animation) is unobservable, so the ack waits out
    /// [`MENU_GRACE`] anchored at the observed exit (ADR-0037/0042).
    // No-tray builds never take this arm (there is no helper to
    // exit); tests keep it honest.
    #[cfg_attr(not(feature = "tray"), allow(dead_code))]
    TrayMenu,
    /// Hotkey / anything menu-less: only the icon removal chain
    /// matters, so the icon grace suffices.
    Other,
}

/// Pure visibility state machine. Owned by the main loop (via
/// [`TrayGate`]'s mutex); `now` is always injected so every
/// transition is unit-testable without sleeping.
#[derive(Debug)]
struct TrayVisibility {
    state: Visibility,
    /// In-flight capture handshakes (workers that requested a hide
    /// and have not yet reported the grab done).
    hide_depth: usize,
    /// Monotonic count of completed (grace-elapsed) hides. Each
    /// handshake's ack waits for this to exceed its baseline.
    confirmed_epoch: u64,
    icon_grace: Duration,
    menu_grace: Duration,
    handshake_window: Duration,
    safety: Duration,
}

/// The icon's visibility. `Hidden` carries the grace anchor
/// (`since`), the grace this hide confirms under (`grace` — icon
/// or menu), whether the grace has elapsed and been acknowledged
/// (`confirmed`), and the earliest a depth-of-zero restore may run
/// (`hold_until` — the pre-hide window that bridges click →
/// handshake).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Visibility {
    Shown,
    Hidden {
        since: Instant,
        grace: Duration,
        confirmed: bool,
        hold_until: Instant,
    },
}

impl TrayVisibility {
    fn new(
        icon_grace: Duration,
        menu_grace: Duration,
        handshake_window: Duration,
        safety: Duration,
    ) -> Self {
        Self {
            state: Visibility::Shown,
            hide_depth: 0,
            confirmed_epoch: 0,
            icon_grace,
            menu_grace,
            handshake_window,
            safety,
        }
    }

    /// Advance one tick. Returns the side effect to apply, if any.
    fn step(&mut self, now: Instant, tray_present: bool) -> Option<TrayAction> {
        match self.state {
            Visibility::Shown => {
                if self.hide_depth == 0 {
                    return None;
                }
                if tray_present {
                    self.state = Visibility::Hidden {
                        since: now,
                        // No pre-hide anchored a click, so no menu is
                        // known to be closing — the icon grace alone.
                        grace: self.icon_grace,
                        confirmed: false,
                        hold_until: now,
                    };
                    Some(TrayAction::Hide)
                } else {
                    // No icon exists: the hide is vacuous, so confirm
                    // immediately — the handshake must not wait on a
                    // grace for pixels that were never there.
                    self.confirmed_epoch += 1;
                    None
                }
            }
            Visibility::Hidden {
                since,
                grace,
                confirmed,
                hold_until,
            } => {
                if now.duration_since(since) > self.safety {
                    // A capture never reported (dead worker). Reset
                    // everything: the icon returns and any handshake
                    // that arrives next starts fresh.
                    self.hide_depth = 0;
                    self.state = Visibility::Shown;
                    return Some(TrayAction::ForceShow);
                }
                if self.hide_depth == 0 && now >= hold_until {
                    // Every in-flight capture reported (or a pre-hide's
                    // handshake window expired: the spawn failed).
                    self.state = Visibility::Shown;
                    return Some(TrayAction::Show);
                }
                if !confirmed && now.duration_since(since) >= grace {
                    // The panel has had the grace to re-render — ack.
                    if let Visibility::Hidden { confirmed, .. } = &mut self.state {
                        *confirmed = true;
                    }
                    self.confirmed_epoch += 1;
                }
                None
            }
        }
    }

    /// Register an in-flight capture handshake (worker side). Returns
    /// the confirm-epoch baseline its ack waits to see exceeded.
    /// Arriving while hidden: the confirmation re-arms — each new
    /// waiter needs its own epoch bump past its baseline — and the
    /// pre-hide hold ends (a real handshake now owns the depth, so
    /// depth-zero restores are meaningful again).
    fn begin_capture(&mut self, now: Instant) -> u64 {
        if let Visibility::Hidden {
            confirmed,
            hold_until,
            ..
        } = &mut self.state
        {
            *confirmed = false;
            *hold_until = now;
        }
        self.hide_depth += 1;
        self.confirmed_epoch
    }

    /// A capture's grab finished; restore when the last one reports.
    fn end_capture(&mut self) {
        self.hide_depth = self.hide_depth.saturating_sub(1);
    }

    /// Pre-hide at dispatch time (tray click / hotkey fire): hide
    /// now, without any depth — the worker's handshake owns it.
    /// `origin` picks the grace this hide confirms under: a tray
    /// click's popup is still mid-close (a `DBusMenu` round trip + spring
    /// animation) and unobservable, so [`PreHideOrigin::TrayMenu`]
    /// waits out the menu grace too, anchored here at the click
    /// (ADR-0037). Returns whether the caller must apply the hide
    /// side effect (false when already hidden or there is no tray).
    fn pre_hide(&mut self, now: Instant, tray_present: bool, origin: PreHideOrigin) -> bool {
        if !tray_present || self.state != Visibility::Shown {
            return false;
        }
        let grace = match origin {
            PreHideOrigin::TrayMenu => self.menu_grace,
            PreHideOrigin::Other => self.icon_grace,
        };
        self.state = Visibility::Hidden {
            since: now,
            grace,
            confirmed: false,
            hold_until: now + self.handshake_window,
        };
        true
    }
}

/// Thread-safe shell around [`TrayVisibility`]: the IPC (tokio) side
/// calls `begin_capture` / `wait_confirmed` / `end_capture`; the main
/// loop calls `pre_hide` / `step` and applies the returned actions.
#[derive(Debug)]
pub(crate) struct TrayGate {
    inner: Mutex<TrayVisibility>,
    confirmed_epoch: AtomicU64,
    confirm_timeout: Duration,
}

impl TrayGate {
    /// Production timings (see the module consts).
    pub(crate) fn new() -> Self {
        Self::with_durations(
            HIDE_GRACE,
            MENU_GRACE,
            HANDSHAKE_WINDOW,
            SAFETY_RESTORE,
            CONFIRM_TIMEOUT,
        )
    }

    /// Testable seam: explicit timings (icon grace, menu grace,
    /// handshake window, safety, confirm timeout).
    pub(crate) fn with_durations(
        icon_grace: Duration,
        menu_grace: Duration,
        handshake_window: Duration,
        safety: Duration,
        confirm_timeout: Duration,
    ) -> Self {
        Self {
            inner: Mutex::new(TrayVisibility::new(
                icon_grace,
                menu_grace,
                handshake_window,
                safety,
            )),
            confirmed_epoch: AtomicU64::new(0),
            confirm_timeout,
        }
    }

    /// Register an in-flight capture handshake; returns the baseline
    /// to pass to [`Self::wait_confirmed`].
    pub(crate) fn begin_capture(&self) -> u64 {
        let now = Instant::now();
        self.inner
            .lock()
            .expect("tray visibility mutex poisoned")
            .begin_capture(now)
    }

    /// The grab finished for one in-flight capture.
    pub(crate) fn end_capture(&self) {
        self.inner
            .lock()
            .expect("tray visibility mutex poisoned")
            .end_capture();
    }

    /// Main-loop pre-hide at dispatch (tray click / hotkey). Returns
    /// whether the caller must hide the icon right now. `origin`
    /// selects the grace (see [`PreHideOrigin`]).
    pub(crate) fn pre_hide(&self, now: Instant, tray_present: bool, origin: PreHideOrigin) -> bool {
        self.inner
            .lock()
            .expect("tray visibility mutex poisoned")
            .pre_hide(now, tray_present, origin)
    }

    /// Main-loop tick: advance the state machine, mirror the confirm
    /// epoch for the IPC waiters, and hand back the side effect to
    /// apply (if any).
    pub(crate) fn step(&self, now: Instant, tray_present: bool) -> Option<TrayAction> {
        let mut inner = self.inner.lock().expect("tray visibility mutex poisoned");
        let action = inner.step(now, tray_present);
        self.confirmed_epoch
            .store(inner.confirmed_epoch, Ordering::Release);
        action
    }

    /// Wait until a hide begun at `baseline` is confirmed (grace
    /// elapsed). Bounded by the gate's confirm timeout; `false` means
    /// the main loop did not confirm in time (the caller acks anyway
    /// — the worker proceeds regardless, worst case the icon made it
    /// into the shot, exactly the pre-fix behaviour).
    pub(crate) async fn wait_confirmed(&self, baseline: u64) -> bool {
        let deadline = Instant::now() + self.confirm_timeout;
        while self.confirmed_epoch.load(Ordering::Acquire) <= baseline {
            if Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        true
    }

    /// Current confirm epoch (assertions in tests).
    #[cfg(test)]
    pub(crate) fn confirmed_epoch(&self) -> u64 {
        self.confirmed_epoch.load(Ordering::Acquire)
    }

    /// In-flight capture count (assertions in tests).
    #[cfg(test)]
    pub(crate) fn hide_depth(&self) -> usize {
        self.inner
            .lock()
            .expect("tray visibility mutex poisoned")
            .hide_depth
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A gate with timings small enough to advance by hand: icon grace
    /// 100 ms, menu grace 300 ms, handshake window 200 ms, safety 1 s.
    fn gate() -> TrayGate {
        TrayGate::with_durations(
            Duration::from_millis(100),
            Duration::from_millis(300),
            Duration::from_millis(200),
            Duration::from_secs(1),
            Duration::from_millis(50),
        )
    }

    /// A fixed anchor each test advances by whole milliseconds —
    /// deterministic arithmetic on one `Instant`, nothing sleeps.
    fn at(anchor: Instant, n: u64) -> Instant {
        anchor + Duration::from_millis(n)
    }

    #[test]
    fn capture_begins_then_step_hides() {
        let g = gate();
        let t0 = Instant::now();
        g.begin_capture();
        assert_eq!(g.step(t0, true), Some(TrayAction::Hide));
    }

    #[test]
    fn hide_is_not_confirmed_before_the_grace() {
        let g = gate();
        let t0 = Instant::now();
        g.begin_capture();
        assert_eq!(g.step(t0, true), Some(TrayAction::Hide));
        assert_eq!(g.step(at(t0, 99), true), None, "grace not yet elapsed");
        assert_eq!(g.confirmed_epoch(), 0, "no ack before the grace");
        g.step(at(t0, 100), true);
        assert_eq!(
            g.confirmed_epoch(),
            1,
            "grace elapsed: exactly one epoch bump"
        );
    }

    #[test]
    fn last_report_restores_the_icon() {
        let g = gate();
        let t0 = Instant::now();
        g.begin_capture();
        assert_eq!(g.step(t0, true), Some(TrayAction::Hide));
        g.step(at(t0, 100), true); // confirm
        g.end_capture();
        assert_eq!(g.step(at(t0, 150), true), Some(TrayAction::Show));
    }

    #[test]
    fn depth_keeps_the_icon_hidden_until_the_last_report() {
        let g = gate();
        let t0 = Instant::now();
        g.begin_capture();
        assert_eq!(g.step(t0, true), Some(TrayAction::Hide));
        g.begin_capture();
        g.step(at(t0, 100), true);
        g.end_capture();
        assert_eq!(
            g.step(at(t0, 120), true),
            None,
            "one capture still in flight: stay hidden"
        );
        g.end_capture();
        assert_eq!(g.step(at(t0, 140), true), Some(TrayAction::Show));
    }

    #[test]
    fn second_handshake_gets_its_own_confirmation() {
        let g = gate();
        let t0 = Instant::now();
        let first = g.begin_capture();
        assert_eq!(g.step(t0, true), Some(TrayAction::Hide));
        g.step(at(t0, 100), true);
        assert_eq!(g.confirmed_epoch(), 1);
        // A second worker arrives mid-flight: its baseline must also
        // be exceeded, not just the first's.
        let second = g.begin_capture();
        assert_eq!(second, 1);
        g.step(at(t0, 110), true);
        assert_eq!(
            g.confirmed_epoch(),
            2,
            "the re-armed hide re-confirms past the second baseline"
        );
        assert!(g.confirmed_epoch() > second);
        assert!(g.confirmed_epoch() > first);
    }

    #[test]
    fn safety_bound_force_restores_and_resets_everything() {
        let g = gate();
        let t0 = Instant::now();
        g.begin_capture();
        assert_eq!(g.step(t0, true), Some(TrayAction::Hide));
        // No report ever arrives; 1 s safety passes.
        assert_eq!(
            g.step(at(t0, 1_001), true),
            Some(TrayAction::ForceShow),
            "safety expiry force-restores"
        );
        assert_eq!(g.hide_depth(), 0, "the dead capture is forgotten");
        // The gate still works afterwards.
        g.begin_capture();
        assert_eq!(g.step(at(t0, 1_100), true), Some(TrayAction::Hide));
    }

    #[test]
    fn pre_hide_hides_without_depth_and_survives_the_handshake_gap() {
        let g = gate();
        let t0 = Instant::now();
        assert!(
            g.pre_hide(t0, true, PreHideOrigin::Other),
            "first pre-hide acts"
        );
        // The worker's handshake has not arrived yet (200 ms window).
        assert_eq!(
            g.step(at(t0, 50), true),
            None,
            "pre-hide holds through the handshake gap despite depth 0"
        );
        let baseline = g.begin_capture(); // the handshake lands
        assert_eq!(baseline, 0);
        assert!(
            !g.pre_hide(at(t0, 60), true, PreHideOrigin::Other),
            "already hidden: no-op"
        );
        g.step(at(t0, 100), true);
        assert_eq!(g.confirmed_epoch(), 1, "grace counted from the click");
        g.end_capture();
        assert_eq!(g.step(at(t0, 120), true), Some(TrayAction::Show));
    }

    #[test]
    fn pre_hide_expires_when_no_handshake_arrives() {
        let g = gate();
        let t0 = Instant::now();
        assert!(g.pre_hide(t0, true, PreHideOrigin::Other));
        assert_eq!(
            g.step(at(t0, 201), true),
            Some(TrayAction::Show),
            "spawn failed: the icon returns after the window"
        );
    }

    #[test]
    fn without_a_tray_the_handshake_confirms_immediately() {
        let g = gate();
        let t0 = Instant::now();
        let baseline = g.begin_capture();
        assert_eq!(g.step(t0, false), None, "nothing to hide: no side effect");
        assert!(
            g.confirmed_epoch() > baseline,
            "vacuous hide confirms without a grace"
        );
    }

    #[test]
    fn pre_hide_without_a_tray_is_a_no_op() {
        let g = gate();
        let t0 = Instant::now();
        assert!(!g.pre_hide(t0, false, PreHideOrigin::TrayMenu));
        assert_eq!(g.step(t0, false), None);
    }

    #[test]
    fn end_capture_floors_at_zero() {
        let g = gate();
        g.end_capture();
        g.end_capture();
        g.begin_capture();
        assert_eq!(g.hide_depth(), 1, "extra ends never underflow the depth");
    }

    #[test]
    fn tray_menu_pre_hide_waits_out_the_menu_grace() {
        // The panel's popup close (a `DBusMenu` round trip + a spring
        // animation) outlasts the icon repaint — a tray-launched
        // capture must not be confirmed at the icon grace.
        let g = gate();
        let t0 = Instant::now();
        assert!(g.pre_hide(t0, true, PreHideOrigin::TrayMenu));
        g.begin_capture(); // the handshake lands; the pre-hide anchors
        assert_eq!(
            g.step(at(t0, 100), true),
            None,
            "icon grace elapsed: still not confirmable"
        );
        assert_eq!(g.confirmed_epoch(), 0);
        g.step(at(t0, 300), true);
        assert_eq!(
            g.confirmed_epoch(),
            1,
            "menu grace elapsed from the click: confirmed"
        );
    }

    #[test]
    fn hotkey_pre_hide_uses_the_icon_grace() {
        // No menu is involved — only the icon repaint must be waited
        // out, so hotkey captures stay as fast as the icon grace.
        let g = gate();
        let t0 = Instant::now();
        assert!(g.pre_hide(t0, true, PreHideOrigin::Other));
        g.begin_capture();
        g.step(at(t0, 100), true);
        assert_eq!(
            g.confirmed_epoch(),
            1,
            "no menu: the icon grace alone gates the ack"
        );
    }

    #[test]
    fn step_created_hide_uses_the_icon_grace() {
        // Hides begun without a pre-hide (IPC/applet path — the
        // daemon learns from the handshake alone) wait only the icon
        // grace; the origin of the click is unknowable there.
        let g = gate();
        let t0 = Instant::now();
        g.begin_capture();
        assert_eq!(g.step(t0, true), Some(TrayAction::Hide));
        assert_eq!(g.step(at(t0, 100), true), None);
        assert_eq!(g.confirmed_epoch(), 1, "step-created hide: icon grace");
    }

    #[tokio::test]
    async fn wait_confirmed_releases_once_the_epoch_passes_the_baseline() {
        let g = std::sync::Arc::new(gate());
        let baseline = g.begin_capture();
        // No tray → each step confirms, so a stepper task driving the
        // gate releases the waiter without real-time dependence.
        let stepper = {
            let g = std::sync::Arc::clone(&g);
            tokio::spawn(async move {
                for _ in 0..50 {
                    g.step(Instant::now(), false);
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
        };
        assert!(
            g.wait_confirmed(baseline).await,
            "the confirm epoch exceeds the baseline"
        );
        stepper.abort();
    }

    #[tokio::test]
    async fn wait_confirmed_is_bounded_when_never_confirmed() {
        // 20 ms timeout, nothing steps the gate: the wait gives up.
        let g = TrayGate::with_durations(
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_millis(20),
        );
        let started = Instant::now();
        assert!(!g.wait_confirmed(0).await);
        assert!(
            started.elapsed() >= Duration::from_millis(20),
            "the bound is actually waited out"
        );
    }
}
