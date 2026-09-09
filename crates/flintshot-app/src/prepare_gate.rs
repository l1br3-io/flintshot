//! Daemon-side coordination for `Request::PrepareCapture`
//! (smart-window-hiding): the generalized pre-grab handshake that
//! clears **every** flintshot surface from the screen before a
//! worker's grab — the app window (hide + ack), pin windows (kill +
//! respawn queue), and the tray icon (the existing `TrayGate`, driven
//! separately by the same interception).
//!
//! Split like `tray_gate`: [`PrepareMachine`] is pure (injected
//! clock, unit-tested); [`PrepareGate`] is the thread-safe shell —
//! the IPC side calls [`PrepareGate::begin`] +
//! [`PrepareGate::wait_ready`], the daemon's main loop ticks
//! [`PrepareGate::step`] every poll, and the
//! `Request::ReportAppHidden` interception flips the ack.
//!
//! The **ack-then-grace** rule (ADR-0047's lesson: close ≠ pixels
//! gone): once every wait-condition holds, the machine still holds
//! the reply for [`UNMAP_GRACE`] — the compositor destroys surfaces
//! asynchronously, and an ack that races the repaint would let the
//! worker grab its window's ghost.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tokio::sync::Notify;

/// Compositor unmap grace shared with the app window's close-for-
/// capture path (`app_window::HIDE_GRACE_MS` aliases this). After
/// every condition holds (app acked/vacuous, pins closed), the reply
/// still waits this out: surface destruction is compositor-side and
/// asynchronous.
pub(crate) const UNMAP_GRACE_MS: u64 = 250;

/// Overall bound for one handshake, mirroring the tray gate's server
/// confirm bound (ADR-0036's "never fail because a window was slow"
/// extended): past it the reply goes out anyway, flagged
/// [`PrepareResolution::TimedOut`] and logged.
const PREPARE_BOUND: Duration = Duration::from_secs(3);

/// IPC-side wait slack over [`PREPARE_BOUND`] (the machine resolves
/// on the main loop's poll cadence, up to 100 ms late).
const WAIT_SLACK: Duration = Duration::from_millis(500);

/// One step's verdict from the pure machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrepareAction {
    /// Conditions unmet and time left: keep waiting.
    Wait,
    /// All clear plus the unmap grace: reply `CaptureReady`.
    Ready,
    /// Bound expired with conditions unmet: reply anyway (warned).
    ReadyTimedOut,
}

/// How a handshake resolved (IPC side returns this from its wait).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrepareResolution {
    /// Screen clear (or vacuous) — grab now.
    Ready,
    /// The bound expired; reply anyway with a warning.
    TimedOut,
}

/// Pure handshake state machine. The only stateful part is the
/// all-clear anchor for the unmap grace; every condition is a fresh
/// input each step, so a regressing condition (a pin respawned
/// mid-handshake) re-arms the grace.
#[derive(Debug)]
struct PrepareMachine {
    deadline: Instant,
    unmap_grace: Duration,
    /// When every wait-condition first held; `None` until then.
    all_clear_at: Option<Instant>,
}

impl PrepareMachine {
    fn new(now: Instant, bound: Duration, unmap_grace: Duration) -> Self {
        Self {
            deadline: now + bound,
            unmap_grace,
            all_clear_at: None,
        }
    }

    /// `live_captures` counts the preparing worker itself (it is a
    /// registered capture session by the time it asks), so ≤ 1 is
    /// clear — dispatch's supersede already made it the only one.
    fn step(
        &mut self,
        now: Instant,
        ack: bool,
        app_alive: bool,
        pins_alive: bool,
        live_captures: usize,
    ) -> PrepareAction {
        let clear = (ack || !app_alive) && !pins_alive && live_captures <= 1;
        if !clear {
            self.all_clear_at = None;
            return if now >= self.deadline {
                PrepareAction::ReadyTimedOut
            } else {
                PrepareAction::Wait
            };
        }
        let anchor = *self.all_clear_at.get_or_insert(now);
        if now >= anchor + self.unmap_grace {
            PrepareAction::Ready
        } else if now >= self.deadline {
            PrepareAction::ReadyTimedOut
        } else {
            PrepareAction::Wait
        }
    }
}

/// Thread-safe shell around [`PrepareMachine`]: begin + wait on the
/// IPC (tokio) thread, ack from the `ReportAppHidden` interception,
/// tick from the daemon's main loop.
#[derive(Debug)]
pub(crate) struct PrepareGate {
    inner: Mutex<PrepareInner>,
    resolved: Notify,
    bound: Duration,
    grace: Duration,
}

#[derive(Debug)]
struct PrepareInner {
    /// A handshake is in flight (begun, not yet resolved).
    armed: bool,
    machine: PrepareMachine,
    /// `ReportAppHidden` seen for the armed handshake.
    ack: bool,
    /// Set once, when the machine resolves.
    outcome: Option<PrepareResolution>,
}

impl PrepareGate {
    /// Production timings (see the module consts).
    pub(crate) fn new() -> Self {
        Self::with_durations(PREPARE_BOUND, Duration::from_millis(UNMAP_GRACE_MS))
    }

    /// Testable seam: explicit handshake bound and unmap grace.
    pub(crate) fn with_durations(bound: Duration, grace: Duration) -> Self {
        Self {
            inner: Mutex::new(PrepareInner {
                armed: false,
                machine: PrepareMachine::new(Instant::now(), bound, grace),
                ack: false,
                outcome: None,
            }),
            resolved: Notify::new(),
            bound,
            grace,
        }
    }

    /// Arm a handshake (IPC side, on `PrepareCapture` receipt).
    /// Re-arming while one is in flight resets it — captures are
    /// serialized, so overlap means a straggler's late ask.
    pub(crate) fn begin(&self, now: Instant) {
        let mut inner = self.inner.lock().expect("prepare gate mutex poisoned");
        inner.armed = true;
        inner.ack = false;
        inner.outcome = None;
        inner.machine = PrepareMachine::new(now, self.bound, self.grace);
    }

    /// `Request::ReportAppHidden` interception: the app window's
    /// "window destroyed" ack.
    pub(crate) fn report_app_hidden(&self) {
        self.inner.lock().expect("prepare gate mutex poisoned").ack = true;
    }

    /// Main-loop tick. Inputs are read fresh each time: an app window
    /// that died without acking (process exit reaped) stops being
    /// waited on, exactly like one that acked.
    pub(crate) fn step(
        &self,
        now: Instant,
        app_alive: bool,
        pins_alive: bool,
        live_captures: usize,
    ) {
        let mut inner = self.inner.lock().expect("prepare gate mutex poisoned");
        if !inner.armed || inner.outcome.is_some() {
            return;
        }
        let ack = inner.ack;
        let action = inner
            .machine
            .step(now, ack, app_alive, pins_alive, live_captures);
        match action {
            PrepareAction::Wait => {}
            PrepareAction::Ready => {
                inner.outcome = Some(PrepareResolution::Ready);
                drop(inner);
                self.resolved.notify_waiters();
            }
            PrepareAction::ReadyTimedOut => {
                inner.outcome = Some(PrepareResolution::TimedOut);
                drop(inner);
                self.resolved.notify_waiters();
            }
        }
    }

    /// The armed handshake's resolution, if the main loop stepped one.
    pub(crate) fn resolution(&self) -> Option<PrepareResolution> {
        self.inner
            .lock()
            .expect("prepare gate mutex poisoned")
            .outcome
    }

    /// Bounded wait for the main loop's resolution. `None` means the
    /// caller's own bound expired (reply anyway, warn) — the machine's
    /// deadline normally resolves first. Poll cadence mirrors the tray
    /// gate's `wait_confirmed`.
    pub(crate) async fn wait_ready(&self) -> Option<PrepareResolution> {
        let deadline = Instant::now() + self.bound + WAIT_SLACK;
        loop {
            if let Some(resolution) = self.resolution() {
                return Some(resolution);
            }
            if Instant::now() >= deadline {
                return None;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(anchor: Instant, ms: u64) -> Instant {
        anchor + Duration::from_millis(ms)
    }

    // ── pure machine ─────────────────────────────────────────────

    fn machine(unmap_grace: Duration) -> PrepareMachine {
        PrepareMachine::new(Instant::now(), Duration::from_secs(3), unmap_grace)
    }

    #[test]
    fn all_clear_reads_ready_after_the_unmap_grace() {
        let t0 = Instant::now();
        let mut m = machine(Duration::from_millis(250));
        assert_eq!(
            m.step(t0, false, false, false, 1),
            PrepareAction::Wait,
            "clear, but the compositor grace has not elapsed"
        );
        assert_eq!(
            m.step(at(t0, 250), false, false, false, 1),
            PrepareAction::Ready
        );
    }

    #[test]
    fn ack_missing_with_no_app_window_is_vacuously_clear() {
        // No app window owes an ack: clear immediately (after the
        // grace).
        let t0 = Instant::now();
        let mut m = machine(Duration::from_millis(10));
        assert_eq!(m.step(t0, false, false, false, 0), PrepareAction::Wait);
        assert_eq!(
            m.step(at(t0, 10), false, false, false, 0),
            PrepareAction::Ready
        );
    }

    #[test]
    fn live_app_window_without_ack_waits_then_times_out() {
        let t0 = Instant::now();
        let mut m = PrepareMachine::new(t0, Duration::from_millis(300), Duration::from_millis(10));
        for t in [0, 100, 299] {
            assert_eq!(
                m.step(at(t0, t), false, true, false, 1),
                PrepareAction::Wait,
                "the app is alive and has not acked"
            );
        }
        assert_eq!(
            m.step(at(t0, 300), false, true, false, 1),
            PrepareAction::ReadyTimedOut,
            "bound expired: reply anyway"
        );
    }

    #[test]
    fn ack_releases_the_app_wait() {
        let t0 = Instant::now();
        let mut m = machine(Duration::from_millis(10));
        assert_eq!(m.step(t0, false, true, false, 1), PrepareAction::Wait);
        // The ack lands mid-grace-window; the grace anchors here.
        assert_eq!(m.step(at(t0, 5), true, true, false, 1), PrepareAction::Wait);
        assert_eq!(
            m.step(at(t0, 16), true, true, false, 1),
            PrepareAction::Ready
        );
    }

    #[test]
    fn dead_app_window_needs_no_ack() {
        // The app process exited without acking (crash / killed): its
        // reaped registry entry stops the wait.
        let t0 = Instant::now();
        let mut m = machine(Duration::from_millis(10));
        assert_eq!(m.step(t0, false, false, false, 1), PrepareAction::Wait);
        assert_eq!(
            m.step(at(t0, 10), false, false, false, 1),
            PrepareAction::Ready
        );
    }

    #[test]
    fn pins_alive_hold_the_reply() {
        let t0 = Instant::now();
        let mut m = machine(Duration::from_millis(10));
        assert_eq!(
            m.step(at(t0, 10), false, false, true, 1),
            PrepareAction::Wait,
            "a live pin would land in the grab"
        );
        assert_eq!(
            m.step(at(t0, 20), false, false, false, 1),
            PrepareAction::Wait,
            "pins clear: the grace re-anchors at the new clear"
        );
        assert_eq!(
            m.step(at(t0, 31), false, false, false, 1),
            PrepareAction::Ready
        );
    }

    #[test]
    fn a_second_live_capture_holds_then_times_out() {
        // Belt-and-braces re-assert: dispatch serialized the supersede,
        // so > 1 live capture means a straggler that ignored even the
        // kill — wait it out, then reply anyway.
        let t0 = Instant::now();
        let mut m = PrepareMachine::new(t0, Duration::from_millis(200), Duration::from_millis(10));
        assert_eq!(m.step(t0, false, false, false, 2), PrepareAction::Wait);
        assert_eq!(
            m.step(at(t0, 200), false, false, false, 2),
            PrepareAction::ReadyTimedOut
        );
    }

    #[test]
    fn regression_re_arms_the_grace() {
        // A condition that clears then regresses (say a pin respawned
        // mid-handshake) must not inherit the old anchor.
        let t0 = Instant::now();
        let mut m = machine(Duration::from_millis(100));
        m.step(t0, false, false, false, 1); // clear at t0
                                            // Pin appears at t0+50, gone again at t0+60: the grace counts
                                            // from the NEW all-clear, not the stale one.
        m.step(at(t0, 50), false, false, true, 1);
        assert_eq!(
            m.step(at(t0, 60), false, false, false, 1),
            PrepareAction::Wait,
            "re-armed: 100 ms grace from the new clear"
        );
        assert_eq!(
            m.step(at(t0, 160), false, false, false, 1),
            PrepareAction::Ready
        );
    }

    // ── shell ────────────────────────────────────────────────────

    fn gate() -> PrepareGate {
        PrepareGate::with_durations(Duration::from_millis(500), Duration::from_millis(10))
    }

    #[test]
    fn begin_resolves_via_main_loop_steps() {
        let g = gate();
        let t0 = Instant::now();
        g.begin(t0);
        g.step(t0, false, false, 0);
        assert_eq!(g.resolution(), None, "grace not yet elapsed");
        g.step(at(t0, 11), false, false, 0);
        assert_eq!(g.resolution(), Some(PrepareResolution::Ready));
    }

    #[test]
    fn report_app_hidden_flips_the_ack() {
        let g = gate();
        let t0 = Instant::now();
        g.begin(t0);
        g.step(at(t0, 20), true, false, 0);
        assert_eq!(g.resolution(), None, "alive app, no ack: waiting");
        g.report_app_hidden();
        g.step(at(t0, 21), true, false, 0);
        assert_eq!(g.resolution(), None, "acked but the grace re-armed");
        g.step(at(t0, 32), true, false, 0);
        assert_eq!(g.resolution(), Some(PrepareResolution::Ready));
    }

    #[test]
    fn timeout_resolution_flows_through() {
        let g = gate();
        let t0 = Instant::now();
        g.begin(t0);
        g.step(at(t0, 500), true, false, 0);
        assert_eq!(g.resolution(), Some(PrepareResolution::TimedOut));
    }

    #[test]
    fn unarmed_gate_never_resolves() {
        let g = gate();
        g.step(Instant::now(), false, false, 0);
        assert_eq!(g.resolution(), None);
    }

    #[tokio::test]
    async fn wait_ready_releases_on_resolution() {
        let g = std::sync::Arc::new(gate());
        let t0 = Instant::now();
        g.begin(t0);
        // Background "main loop" stepping the gate to clear.
        let stepper = {
            let g = std::sync::Arc::clone(&g);
            tokio::spawn(async move {
                for _ in 0..100 {
                    g.step(Instant::now(), false, false, 0);
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
        };
        let resolved = g.wait_ready().await;
        assert_eq!(resolved, Some(PrepareResolution::Ready));
        stepper.abort();
    }

    #[tokio::test]
    async fn wait_ready_is_bounded_when_never_resolved() {
        // A 20 ms machine bound nothing steps: the shell's own bound
        // (machine bound + slack) must still release the caller.
        let g = PrepareGate::with_durations(Duration::from_millis(20), Duration::from_millis(10));
        let started = Instant::now();
        g.begin(started);
        let resolved = g.wait_ready().await;
        assert_eq!(resolved, None, "never resolved: the wait bound fired");
        assert!(
            started.elapsed() >= Duration::from_millis(20),
            "the bound is actually waited out"
        );
    }
}
