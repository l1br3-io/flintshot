# Task 004: Tray helper daemon-watch bounded connect retry

**Description:** The helper's one-shot daemon watch marks *any*
connect failure as "daemon gone" and the helper exits — during the
daemon's own startup (socket race) or against an IPC-less daemon this
produces respawn churn (spec §Diagnosis A3). Give the watch a bounded
(~2 s) connect-retry window: retry `Stream::connect` at a short
interval until the window expires, and only then mark gone. A
late-appearing listener is accepted within the window. The stream-EOF
→ gone semantics are unchanged (daemon death still ends the helper
promptly).

**Acceptance criteria:**
- [ ] RED first: seam-level test — a watch pointed at a socket path whose listener appears ~after the first connect attempt does NOT mark gone within the window (current one-shot code marks gone immediately → RED)
- [ ] Absent path / refused connect retried within the window, then marked gone (bounded — total watch failure ≤ window + one interval)
- [ ] A successful connect followed by stream end still marks gone immediately (no retry after EOF — daemon death stays prompt)
- [ ] Connect-retry parameters (window ~2 s, interval ~100–250 ms) named consts; every wait bounded; no unbounded loops
- [ ] Testable seam: the watch body (connect → subscribe → park) extracted so tests can drive it against a tempdir socket with a late-bound listener (fake-server pattern; try headless, `#[ignore]` + run instruction only if proven flaky)

**Verification:**
- [ ] `cargo test -p flintshot-app -q tray` green
- [ ] Full gate (four commands, quiet)
- [ ] Manual check deferred to the manual matrix / diagnostic log kit (no rapid "tray helper spawned" repetition)

**Dependencies:** None (conceptually pairs with Task 005 — together
they close mechanism A3; code-independent)

**Files likely touched:**
- `crates/flintshot-app/src/tray_helper.rs` (watch body, consts, tests)

**Docs:**
- Comment on the watch: why the window exists (daemon-startup socket
  race + IPC-less churn) and why EOF still means immediate gone.

**Estimated scope:** Medium: 1 file
