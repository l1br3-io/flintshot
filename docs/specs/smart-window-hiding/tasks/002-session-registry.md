# Task 002: Capture session registry (`sessions.rs`)

**Description:** New module `crates/flintshot-app/src/sessions.rs`
holding the daemon's notion of live worker sessions, designed after
`tray_gate.rs`: a pure, unit-testable core plus a thread-safe shell.

Data model:
- `SessionKind { Capture, Pin }`
- Registry entries keyed by child pid:
  - kill handle: `Arc<Mutex<Option<Child>>>` — the reaper never
    *takes* the `Child` out (see Task 003 for why: a blocked
    `wait()` would make the handle unkillable); instead the shell
    exposes `poll_exit(pid)` running `try_wait()` under a short
    lock, and `kill(pid)` locking briefly to call `Child::kill`.
  - for pins: the full respawn argv (`Vec<String>` incl.
    `--at/--size/--opacity`) and the pin id.
- Respawn queue: pin argvs parked when a pin is closed for a grab;
  drained (`take()`) when the daemon restores surfaces.

Shell API (thread-safe, `std::sync`):
- `register_capture(pid, handle)` / `register_pin(id, pid, argv, handle)`
- `exited(pid)` — reaper-observed exit; clears the entry
- `live_captures() -> usize` / `capture_pids() -> Vec<u32>`
- `supersede_step(now) -> SupersedeAction` — pure-machine-driven:
  broadcast-then-wait, then kill, per the graces below
- `wait_no_captures(deadline)` — blocking bounded wait (condvar or
  poll loop; injected clock for tests)
- `kill_all_captures()`, `close_all_pins()` (kills + queues respawn
  argvs), `take_respawn_queue() -> Vec<Vec<String>>`

Graces as named consts (tunable later):
- `SUPERSEDE_GRACE_MS: u64 = 1500` — wait for graceful exit
- `KILL_WAIT_MS: u64 = 500` — wait after hard kill

**Acceptance criteria:**
- [ ] Pure transitions: supersede lifecycle
      (wait → kill → confirmed) covered by unit tests with a fake
      clock, including "straggler ignores grace → killed → reaped"
- [ ] Kill handle stays reachable while a poll-wait is in progress
      (unit test: `poll_exit` loop + concurrent `kill` on a dummy —
      may use a real short-lived child like `worker.rs` reaper tests
      do)
- [ ] Pin queue: closed pins' argvs enqueue exactly once; drain is
      destructive; killed capture sessions never enqueue
- [ ] `wait_no_captures` respects its deadline (test with fake
      clock) and wakes early on `exited`

**Verification:**
- [ ] `cargo test -p flintshot-app sessions`
- [ ] Standing gates (`fmt`, `clippy -D warnings`, workspace tests)

**Dependencies:** None (wired up in Task 003; not yet used by the daemon)

**Files likely touched:**
- `crates/flintshot-app/src/sessions.rs` (new, + `lib`/`mod` wiring)
- `crates/flintshot-app/src/main.rs` or wherever modules are declared

**Docs:** Consts carry the grace rationale as doc comments.

**Estimated scope:** Medium: 2 files
