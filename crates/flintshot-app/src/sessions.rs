//! Daemon-side registry of live worker sessions (smart-window-hiding).
//!
//! The daemon's notion of an *active capture session*: every capture
//! and pin worker it spawned, keyed by pid, each holding a kill
//! handle (`Arc<Mutex<Option<Child>>>`). Process exit stays the
//! truth source — a reaper observes exits and reports them via
//! [`CaptureSessions::exited`] — so a crashed or killed worker
//! "reports" by dying, exactly like the `SessionEnded` reaper
//! (ADR-0046).
//!
//! The reaper never *takes* the `Child` out of the handle: a blocked
//! `wait()` would own the child exclusively and make the kill handle
//! unreachable. Instead reaping is poll-based
//! ([`CaptureSessions::poll_exit`] runs `try_wait()` under a short
//! lock), so a supersede can still hard-kill a straggler mid-poll.
//!
//! The **Replace** policy's ordering — broadcast superseded, wait
//! bounded for graceful exits, hard-kill stragglers, wait again —
//! lives in [`SupersedeMachine`]: pure, injected-clock, unit-tested.
//! [`CaptureSessions`] is the thread-safe shell around it (IPC
//! thread + main loop), designed after `tray_gate.rs`.
//!
//! Pins are closed (killed) rather than superseded — their argv is
//! parked in a respawn queue the daemon drains after the grab, so a
//! pin returns at its exact position/size/opacity.

// Wired into the daemon's dispatch path in the supersede-dispatch
// task; until then nothing outside this module (and its tests)
// constructs a registry.
#![allow(dead_code)]

use std::collections::HashMap;
use std::process::Child;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// How long a superseded worker gets to close gracefully (its IPC
/// subscription heard `Event::CaptureSuperseded` and it is exiting
/// via the normal path — RAII guards finish, no partial saves). Past
/// this bound the supersede hard-kills: a worker that cannot close
/// within ~1.5 s is hung, and the new capture must not wait forever.
const SUPERSEDE_GRACE: Duration = Duration::from_millis(1500);

/// How long to wait after a hard kill for the reaper to confirm the
/// exits. Short: SIGKILL is immediate, only the poll cadence delays
/// the observation. Past it the supersede proceeds anyway — process
/// exit remains the truth, so a late confirmation still cleans up.
const KILL_WAIT: Duration = Duration::from_millis(500);

/// What kind of worker a session entry tracks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionKind {
    /// A capture worker (`region` / `fullscreen`): superseded on a
    /// new capture, never respawned.
    Capture,
    /// A pin worker: closed for a grab and respawned from its parked
    /// argv afterwards.
    Pin,
}

/// Side effect / phase signal of one [`SupersedeMachine`] step. The
/// machine decides *when*; the caller applies the process/IPC side
/// of `Broadcast` (the subscriber bus is not owned here), while
/// `Kill` is applied by the shell itself on its own stored handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SupersedeAction {
    /// Broadcast [`crate::…Event::CaptureSuperseded`] — not applied
    /// here; the daemon owns the bus. The machine has armed its
    /// grace deadline.
    Broadcast,
    /// Grace expired with live captures: the shell hard-killed them
    /// (the action is returned for logging/tests).
    Kill,
    /// The old session is gone (or every bound expired) — the caller
    /// may spawn the new worker now.
    Proceed,
    /// Within a bound; nothing to do yet.
    Wait,
}

/// Pure supersede state machine: `Idle → AwaitGrace → (AwaitKill) → Idle`.
/// `now` is always injected so every transition is unit-testable
/// without sleeping.
#[derive(Debug)]
struct SupersedeMachine {
    phase: SupersedePhase,
    grace: Duration,
    kill_wait: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SupersedePhase {
    Idle,
    AwaitGrace { until: Instant },
    AwaitKill { until: Instant },
}

impl SupersedeMachine {
    fn new(grace: Duration, kill_wait: Duration) -> Self {
        Self {
            phase: SupersedePhase::Idle,
            grace,
            kill_wait,
        }
    }

    /// Advance one tick. `live` is the caller's live-capture count (a
    /// pure input, like `tray_gate`'s `tray_present`).
    fn step(&mut self, now: Instant, live: usize) -> SupersedeAction {
        match self.phase {
            SupersedePhase::Idle => {
                if live == 0 {
                    SupersedeAction::Proceed
                } else {
                    self.phase = SupersedePhase::AwaitGrace {
                        until: now + self.grace,
                    };
                    SupersedeAction::Broadcast
                }
            }
            SupersedePhase::AwaitGrace { until } => {
                if live == 0 {
                    self.phase = SupersedePhase::Idle;
                    SupersedeAction::Proceed
                } else if now < until {
                    SupersedeAction::Wait
                } else {
                    self.phase = SupersedePhase::AwaitKill {
                        until: now + self.kill_wait,
                    };
                    SupersedeAction::Kill
                }
            }
            SupersedePhase::AwaitKill { until } => {
                if live == 0 || now >= until {
                    self.phase = SupersedePhase::Idle;
                    SupersedeAction::Proceed
                } else {
                    SupersedeAction::Wait
                }
            }
        }
    }
}

/// Pin respawn payload: the exact worker argv to re-run.
#[derive(Debug, Clone)]
struct PinInfo {
    id: u64,
    argv: Vec<String>,
}

/// One tracked worker session.
#[derive(Debug)]
struct SessionEntry {
    kind: SessionKind,
    handle: Arc<Mutex<Option<Child>>>,
    /// Present for pins only.
    pin: Option<PinInfo>,
}

#[derive(Debug)]
struct RegistryInner {
    entries: HashMap<u32, SessionEntry>,
    respawn_queue: Vec<Vec<String>>,
    machine: SupersedeMachine,
}

impl RegistryInner {
    fn new(grace: Duration, kill_wait: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            respawn_queue: Vec::new(),
            machine: SupersedeMachine::new(grace, kill_wait),
        }
    }

    fn live_captures(&self) -> usize {
        self.entries
            .values()
            .filter(|e| e.kind == SessionKind::Capture)
            .count()
    }
}

/// Thread-safe shell: registration and exits from the spawner's
/// reaper threads, supersede driving and pin respawn from the
/// dispatch path.
#[derive(Debug)]
pub(crate) struct CaptureSessions {
    inner: Mutex<RegistryInner>,
    /// Signalled on every [`Self::exited`] so [`Self::wait_no_captures`]
    /// wakes early instead of polling out its whole deadline.
    exited_cv: Condvar,
}

/// Best-effort SIGKILL through a stored handle. Errors are ignored:
/// an already-exited-but-unreaped child is harmless, and a reaped one
/// means the session is already over.
fn kill_handle(handle: &Arc<Mutex<Option<Child>>>) {
    if let Some(child) = handle
        .lock()
        .expect("session handle mutex poisoned")
        .as_mut()
    {
        let _ = child.kill();
    }
}

impl CaptureSessions {
    /// Registry with the production graces (see the module consts).
    pub(crate) fn new() -> Self {
        Self::with_graces(SUPERSEDE_GRACE, KILL_WAIT)
    }

    /// Testable seam: explicit supersede graces (grace, kill wait).
    pub(crate) fn with_graces(grace: Duration, kill_wait: Duration) -> Self {
        Self {
            inner: Mutex::new(RegistryInner::new(grace, kill_wait)),
            exited_cv: Condvar::new(),
        }
    }

    /// Track a spawned capture worker.
    pub(crate) fn register_capture(&self, pid: u32, handle: Arc<Mutex<Option<Child>>>) {
        self.inner
            .lock()
            .expect("session registry mutex poisoned")
            .entries
            .insert(
                pid,
                SessionEntry {
                    kind: SessionKind::Capture,
                    handle,
                    pin: None,
                },
            );
    }

    /// Track a spawned pin worker with its full respawn argv.
    pub(crate) fn register_pin(
        &self,
        id: u64,
        pid: u32,
        argv: Vec<String>,
        handle: Arc<Mutex<Option<Child>>>,
    ) {
        self.inner
            .lock()
            .expect("session registry mutex poisoned")
            .entries
            .insert(
                pid,
                SessionEntry {
                    kind: SessionKind::Pin,
                    handle,
                    pin: Some(PinInfo { id, argv }),
                },
            );
    }

    /// Reaper-observed exit: forget the session and wake any
    /// bounded waiter. Unknown pids are a no-op (the entry may have
    /// been cleared already by a close).
    pub(crate) fn exited(&self, pid: u32) {
        self.inner
            .lock()
            .expect("session registry mutex poisoned")
            .entries
            .remove(&pid);
        self.exited_cv.notify_all();
    }

    /// How many capture workers are live.
    pub(crate) fn live_captures(&self) -> usize {
        self.inner
            .lock()
            .expect("session registry mutex poisoned")
            .live_captures()
    }

    /// Pids of the live capture workers.
    pub(crate) fn capture_pids(&self) -> Vec<u32> {
        let inner = self.inner.lock().expect("session registry mutex poisoned");
        inner
            .entries
            .iter()
            .filter(|(_, e)| e.kind == SessionKind::Capture)
            .map(|(&pid, _)| pid)
            .collect()
    }

    /// Drive the supersede machine one step. When the grace expires
    /// with captures still live, the stragglers are hard-killed here
    /// (the returned [`SupersedeAction::Kill`] is informational).
    pub(crate) fn supersede_step(&self, now: Instant) -> SupersedeAction {
        let mut inner = self.inner.lock().expect("session registry mutex poisoned");
        let live = inner.live_captures();
        let action = inner.machine.step(now, live);
        if action == SupersedeAction::Kill {
            for entry in inner.entries.values() {
                if entry.kind == SessionKind::Capture {
                    kill_handle(&entry.handle);
                }
            }
        }
        action
    }

    /// Block until no captures are live or `deadline` passes.
    /// Returns whether the captures drained. Wakes early on every
    /// [`Self::exited`].
    pub(crate) fn wait_no_captures(&self, deadline: Instant) -> bool {
        let mut guard = self.inner.lock().expect("session registry mutex poisoned");
        loop {
            if guard.live_captures() == 0 {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            let (g, _timeout) = self
                .exited_cv
                .wait_timeout(guard, deadline - now)
                .expect("session registry condvar poisoned");
            guard = g;
        }
    }

    /// Best-effort SIGKILL of one session's worker.
    pub(crate) fn kill(&self, pid: u32) {
        let handle = {
            let inner = self.inner.lock().expect("session registry mutex poisoned");
            inner.entries.get(&pid).map(|e| Arc::clone(&e.handle))
        };
        if let Some(handle) = handle {
            kill_handle(&handle);
        }
    }

    /// Best-effort hard kill of every live capture worker (the
    /// supersede fallback; also applied internally by
    /// [`Self::supersede_step`] at its Kill transition).
    pub(crate) fn kill_all_captures(&self) {
        let inner = self.inner.lock().expect("session registry mutex poisoned");
        for entry in inner.entries.values() {
            if entry.kind == SessionKind::Capture {
                kill_handle(&entry.handle);
            }
        }
    }

    /// Close every live pin for a grab: hard-kill the workers and
    /// park their argvs in the respawn queue (exactly once — the
    /// entries leave the registry as they are queued).
    pub(crate) fn close_all_pins(&self) {
        let mut inner = self.inner.lock().expect("session registry mutex poisoned");
        let pin_pids: Vec<u32> = inner
            .entries
            .iter()
            .filter(|(_, e)| e.kind == SessionKind::Pin)
            .map(|(&pid, _)| pid)
            .collect();
        for pid in pin_pids {
            if let Some(entry) = inner.entries.remove(&pid) {
                kill_handle(&entry.handle);
                if let Some(pin) = entry.pin {
                    inner.respawn_queue.push(pin.argv);
                }
            }
        }
    }

    /// Destructively drain the parked pin argvs to respawn.
    pub(crate) fn take_respawn_queue(&self) -> Vec<Vec<String>> {
        std::mem::take(
            &mut self
                .inner
                .lock()
                .expect("session registry mutex poisoned")
                .respawn_queue,
        )
    }

    /// Poll one worker's exit under a short lock (never a blocking
    /// `wait` — the kill handle must stay reachable). Returns the
    /// status once observed; the registry entry is cleared with
    /// [`Self::exited`]. `None` means still pending (or unknown pid).
    pub(crate) fn poll_exit(&self, pid: u32) -> Option<std::process::ExitStatus> {
        enum Polled {
            Exited(std::process::ExitStatus),
            Gone,
            Pending,
        }
        let handle = {
            let inner = self.inner.lock().expect("session registry mutex poisoned");
            let entry = inner.entries.get(&pid)?;
            Arc::clone(&entry.handle)
        };
        // The handle guard is dropped before `exited` re-enters the
        // registry: lock order stays inner → handle everywhere.
        let polled = {
            let mut guard = handle.lock().expect("session handle mutex poisoned");
            match guard.as_mut() {
                None => Polled::Gone,
                Some(child) => match child.try_wait() {
                    Ok(Some(status)) => Polled::Exited(status),
                    Ok(None) => Polled::Pending,
                    // ECHILD: reaped elsewhere — the session is over.
                    Err(_) => Polled::Gone,
                },
            }
        };
        match polled {
            Polled::Exited(status) => {
                self.exited(pid);
                Some(status)
            }
            Polled::Gone => {
                self.exited(pid);
                None
            }
            Polled::Pending => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Production graces, advanced by hand on one anchor — the
    /// machine takes `now` so nothing sleeps.
    fn at(anchor: Instant, ms: u64) -> Instant {
        anchor + Duration::from_millis(ms)
    }

    /// A session registry with no real child behind the handle
    /// (kills are best-effort no-ops; lifecycle logic is what is
    /// under test).
    fn fake_handle() -> Arc<Mutex<Option<Child>>> {
        Arc::new(Mutex::new(None))
    }

    #[test]
    fn supersede_lifecycle_broadcast_wait_reaped_proceed() {
        let s = CaptureSessions::new();
        s.register_capture(101, fake_handle());
        let t0 = Instant::now();

        assert_eq!(s.supersede_step(t0), SupersedeAction::Broadcast);
        assert_eq!(
            s.supersede_step(at(t0, 100)),
            SupersedeAction::Wait,
            "grace not yet expired"
        );
        s.exited(101);
        assert_eq!(
            s.supersede_step(at(t0, 200)),
            SupersedeAction::Proceed,
            "reaper-confirmed exit releases the supersede"
        );
        assert_eq!(s.live_captures(), 0);
    }

    #[test]
    fn straggler_ignores_grace_is_killed_then_reaped() {
        let s = CaptureSessions::new();
        s.register_capture(202, fake_handle());
        let t0 = Instant::now();

        assert_eq!(s.supersede_step(t0), SupersedeAction::Broadcast);
        assert_eq!(s.supersede_step(at(t0, 1499)), SupersedeAction::Wait);
        assert_eq!(
            s.supersede_step(at(t0, 1500)),
            SupersedeAction::Kill,
            "grace expired with a live straggler: hard kill"
        );
        s.exited(202);
        assert_eq!(s.supersede_step(at(t0, 1501)), SupersedeAction::Proceed);
    }

    #[test]
    fn straggler_surviving_the_kill_still_proceeds_at_the_bound() {
        // A worker that ignores even SIGKILL cannot wedge the
        // supersede: past the kill wait the new capture proceeds and
        // the eventual reaper report cleans the registry.
        let s = CaptureSessions::new();
        s.register_capture(303, fake_handle());
        let t0 = Instant::now();

        assert_eq!(s.supersede_step(t0), SupersedeAction::Broadcast);
        assert_eq!(s.supersede_step(at(t0, 1500)), SupersedeAction::Kill);
        assert_eq!(
            s.supersede_step(at(t0, 1999)),
            SupersedeAction::Wait,
            "kill wait not yet expired"
        );
        assert_eq!(
            s.supersede_step(at(t0, 2000)),
            SupersedeAction::Proceed,
            "bounded: proceeds without the straggler's exit"
        );
        assert_eq!(s.live_captures(), 1, "entry lingers until the reaper");
        s.exited(303);
        assert_eq!(s.live_captures(), 0);
    }

    #[test]
    fn supersede_with_no_live_captures_is_vacuous() {
        let s = CaptureSessions::new();
        assert_eq!(
            s.supersede_step(Instant::now()),
            SupersedeAction::Proceed,
            "nothing to supersede: no broadcast, proceed immediately"
        );
    }

    #[test]
    fn pins_are_not_captures_for_the_supersede_machine() {
        let s = CaptureSessions::new();
        s.register_pin(7, 404, vec!["pin".into()], fake_handle());
        assert_eq!(s.live_captures(), 0);
        assert_eq!(s.capture_pids(), Vec::<u32>::new());
        assert_eq!(
            s.supersede_step(Instant::now()),
            SupersedeAction::Proceed,
            "a live pin alone is not superseded"
        );
    }

    #[test]
    fn closing_pins_queues_argv_exactly_once_and_drain_is_destructive() {
        let s = CaptureSessions::new();
        s.register_pin(
            7,
            404,
            vec![
                "pin".into(),
                "/tmp/x.png".into(),
                "--at".into(),
                "10,20".into(),
            ],
            fake_handle(),
        );
        s.register_pin(
            8,
            405,
            vec!["pin".into(), "/tmp/y.png".into()],
            fake_handle(),
        );
        s.close_all_pins();
        s.close_all_pins(); // second close must not double-queue

        let mut queued = s.take_respawn_queue();
        queued.sort();
        assert_eq!(
            queued,
            vec![
                vec![
                    "pin".to_string(),
                    "/tmp/x.png".into(),
                    "--at".into(),
                    "10,20".into()
                ],
                vec!["pin".to_string(), "/tmp/y.png".into()],
            ],
            "each pin's full argv parked exactly once"
        );
        assert!(s.take_respawn_queue().is_empty(), "drain is destructive");
    }

    #[test]
    fn killed_capture_sessions_never_enqueue_for_respawn() {
        let s = CaptureSessions::new();
        s.register_capture(505, fake_handle());
        s.kill_all_captures();
        s.close_all_pins();
        assert!(
            s.take_respawn_queue().is_empty(),
            "the respawn queue is a pin-only concept"
        );
    }

    #[test]
    fn wait_no_captures_true_immediately_when_none_are_live() {
        let s = CaptureSessions::new();
        assert!(s.wait_no_captures(Instant::now() + Duration::from_secs(5)));
    }

    #[test]
    fn wait_no_captures_respects_its_deadline() {
        let s = CaptureSessions::new();
        s.register_capture(606, fake_handle());
        let started = Instant::now();
        assert!(
            !s.wait_no_captures(started + Duration::from_millis(50)),
            "deadline passes with the capture live"
        );
        assert!(
            started.elapsed() >= Duration::from_millis(50),
            "the bound is actually waited out"
        );
    }

    #[test]
    fn wait_no_captures_wakes_early_on_exited() {
        let s = CaptureSessions::new();
        s.register_capture(707, fake_handle());
        let s2 = Arc::new(s);
        let notifier = {
            let s = Arc::clone(&s2);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(30));
                s.exited(707);
            })
        };
        let started = Instant::now();
        assert!(
            s2.wait_no_captures(started + Duration::from_secs(10)),
            "the exit releases the waiter"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "woke early, not by the 10 s deadline"
        );
        notifier.join().expect("notifier thread");
    }

    /// A real short-lived-ish child (`sleep 30`) so `try_wait` /
    /// `kill` exercise real OS semantics: a concurrent kill must
    /// stay possible while another thread polls the exit — the
    /// poll-wait must never own the child exclusively.
    #[test]
    fn kill_handle_stays_reachable_during_poll_wait() {
        let s = Arc::new(CaptureSessions::new());
        let child = std::process::Command::new("sleep")
            .arg("30")
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("spawning sleep");
        let pid = child.id();
        s.register_capture(pid, Arc::new(Mutex::new(Some(child))));

        let poller = {
            let s = Arc::clone(&s);
            std::thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(5);
                while Instant::now() < deadline {
                    if s.poll_exit(pid).is_some() {
                        return true;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                false
            })
        };

        std::thread::sleep(Duration::from_millis(20));
        s.kill(pid); // must not deadlock or miss the poller's lock

        assert!(
            poller.join().expect("poller thread"),
            "the poll observed the killed child's exit"
        );
        assert_eq!(s.live_captures(), 0, "poll_exit cleared the entry");
    }
}
