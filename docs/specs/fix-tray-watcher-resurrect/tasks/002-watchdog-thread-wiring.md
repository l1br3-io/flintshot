# Task 002: Watchdog thread + `daemon.rs` wiring

**Description:** Turn the Task 1 decision logic into a long-running
watchdog thread and wire it into the daemon. This task also lands
the real zbus-backed `SniBus` impl (compiled, not unit-tested —
activation is bus-daemon behavior CI can't simulate; stated
honestly): sync methods that `block_on` on a current-thread runtime
the impl owns (`send_request_blocking` precedent,
`ipc_server.rs:511`), opening a fresh session-bus connection per
probe/poke (nothing stale to outlive a bus restart) and bounding
every poke with `tokio::time::timeout(2 s)`. The thread's loop:
probe immediately on start (this *is* the spec's startup poke —
resolves OQ1 as fire-and-forget), then every 30 s, running
`watchdog_tick` each time; sleep is `mpsc::Receiver::recv_timeout(`
`INTERVAL)` so dropping the sender (daemon exit) wakes the thread
instantly. `daemon.rs` spawns the watchdog before `tray::build`
(best-effort: a spawn failure logs a warning and the daemon
continues without it) and joins it after the event loop, next to the
existing tokio-side join. Interval and probe constructor are
parameters of the internal spawn fn so tests can drive the loop with
a fake bus and a short interval; the public entry point fixes the
30 s const (no config surface — spec out-of-scope).

**Acceptance criteria:**

- [x] zbus-backed `SniBus` impl: `block_on` on an owned
      current-thread runtime, per-call fresh connection, poke bound
      by `tokio::time::timeout(2 s)`; never constructed by tests
      (no bus in CI)
- [x] `sni_watchdog::spawn()`-shaped entry returns a handle owning
      the shutdown sender + `JoinHandle`; `INTERVAL = 30 s` and the
      2 s poke bound are named `const`s
- [x] First tick runs immediately at thread start (before any
      sleep); subsequent ticks every interval
- [x] Shutdown: dropping the sender (or an explicit handle method)
      ends the thread; join is bounded by at most one in-flight poke
- [x] `daemon.rs`: watchdog spawned before `tray::build`, only under
      `#[cfg(all(feature = "tray", target_os = "linux"))]`; spawn
      failure never aborts daemon startup (warn + continue); handle
      joined after `run_event_loop` returns
- [x] Unit tests with a fake `SniBus`: first tick fires without
      waiting an interval; the thread exits promptly (< ~1 s) once
      the sender drops; over a short interval the tick count matches
      probes seen by the fake
- [x] Non-tray and non-Linux builds: no watchdog code compiled;
      `run()` shape unchanged apart from the cfg-gated block

**Verification:**

- [x] `cargo test -p flintshot-app --features tray --quiet` green
      (new loop tests alongside Task 1's seam tests)
- [x] `cargo check -p flintshot-app --no-default-features --quiet`
      clean
- [x] Full standing gates: `cargo fmt --all --check`;
      `cargo clippy --workspace --all-targets -- -D warnings`;
      `cargo test --workspace --all-targets`;
      `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [x] Manual smoke (optional pre-L5): with the daemon running on any
      session, `RUST_LOG=flintshot=debug` shows one probe per 30 s
      and no pokes while a watcher is alive

**Dependencies:** Task 1 (seam + decision logic + dep edge)

**Files likely touched:**

- `crates/flintshot-app/src/sni_watchdog.rs` (thread loop, handle,
  tests)
- `crates/flintshot-app/src/daemon.rs` (spawn before `tray::build`,
  join at exit)

**Docs:**

- GOTCHA candidate for the module doc: the daemon loop itself must
  never call the blocking probe — that's *why* this is a dedicated
  thread (GLib-pumped main loop stays non-blocking, ADR-0031).

**Estimated scope:** Small: 2 files
