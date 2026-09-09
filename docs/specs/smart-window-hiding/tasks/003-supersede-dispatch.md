# Task 003: Supersede-before-spawn at dispatch entry points

**Description:** Wire the registry (Task 002) into the daemon so
every capture trigger — IPC (`ipc_server.rs` `handle_connection`),
X11 hotkeys (`daemon.rs` `drain_hotkeys`), portal hotkeys
(`hotkeys_portal.rs` `dispatch_activation`) — runs **supersede →
spawn** under a capture mutex, implementing the Replace policy with
the kill path first (graceful close lands in Task 004).

Changes:
- `CommandSpawner::spawn`: register capture/pin children in the
  shared `CaptureSessions` (classify via existing `is_capture_argv`
  + a pin-argv twin; pin argv is captured verbatim into the
  registry). The reaper thread switches from blocking
  `Child::wait()` to a `try_wait` poll loop (short lock holds) and
  calls `sessions.exited(pid)` — the kill handle must stay
  reachable (see Task 002).
- New `worker.rs` helper `dispatch_capture(request, spawner, state,
  sessions, sink)` used by every entry site: broadcast
  `Event::CaptureSuperseded`, `sessions.supersede_step` /
  `wait_no_captures` (bounded), kill stragglers, then the existing
  dispatch arm (spawn). Sink ordering: `CaptureSuperseded` before
  `CaptureStarted`.
- Serialization: one capture mutex in the daemon state; the IPC
  (async) path performs the bounded wait via
  `tokio::task::spawn_blocking` so the tokio side thread never
  blocks; sync loop sites call the blocking form directly.
- `hotkeys_portal.rs` / `daemon.rs` dispatch sites route capture
  requests through the helper (reuse `spawns_capture_worker` to
  branch).

**Acceptance criteria:**
- [ ] Unit tests (RecordingSpawner + registry with fake children):
      second capture while one is live → old session superseded
      (killed after grace in tests) before the new spawn; exactly
      one live capture afterwards
- [ ] Simultaneous double-trigger test (two sequential
      `dispatch_capture` calls) → serialized, latest wins, no
      interleaved spawn-before-supersede
- [ ] Kill -9 resilience: a fake straggler that outlives both
      graces is killed and reaped; `SessionEnded` still broadcast
      (reaper path unchanged in behavior)
- [ ] Non-capture requests (OpenApp, PinLast, UploadLast, OpenLast)
      do not supersede anything (dispatch unchanged for them)
- [ ] Existing worker/daemon/ipc_server tests updated and green

**Verification:**
- [ ] `cargo test -p flintshot-app`
- [ ] Standing gates
- [ ] Manual (Checkpoint A): editor open → trigger tray/hotkey
      capture → old worker killed within ~2 s, exactly one editor
      remains, no fork loop

**Dependencies:** 001 (CaptureSuperseded variant), 002 (registry)

**Files likely touched:**
- `crates/flintshot-app/src/worker.rs`
- `crates/flintshot-app/src/ipc_server.rs`
- `crates/flintshot-app/src/daemon.rs`
- `crates/flintshot-app/src/hotkeys_portal.rs`
- `crates/flintshot-app/src/sessions.rs` (minor)

**Docs:** None yet (ADR-0048 in Task 009).

**Estimated scope:** Medium-Large: 5 files
