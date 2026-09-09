# Task 005: `PrepareCapture` daemon coordination

**Description:** The daemon side of the generalized pre-grab
handshake. Intercept `Request::PrepareCapture` in
`ipc_server.rs::handle_connection` (request/reply, mirroring
`HideTrayForCapture`'s interception) and coordinate until the screen
is clear of flintshot chrome, then reply `Event::CaptureReady`:

1. Re-assert no live capture sessions (`sessions.live_captures()`;
   dispatch already superseded — belt-and-braces, cheap).
2. Broadcast `Event::HideForCapture`; arm an app-window-ack wait
   (one `ReportAppHidden` expected — intercept that request too and
   mark the ack). If no app window is subscribed, vacuous.
3. `sessions.close_all_pins()` — kill pin workers, park their argvs
   in the respawn queue.
4. Drive the existing tray gate (the `HideTrayForCapture` path —
   same graces, reuse `TrayGate`).
5. When tray-gate done **and** (app-ack received **or** no app
   window **or** bound elapsed) **and** pin processes reaped →
   sleep a small post-ack compositor unmap grace (reuse
   `HIDE_GRACE_MS` from `app_window.rs`, hoisted to a shared const)
   → reply `CaptureReady`. On any timeout: reply anyway + `warn`
   (a capture must never fail because a window was slow — ADR-0036
   philosophy, extended).

`Request::ReportCaptured` interception (exists for the tray) grows:
drain `sessions.take_respawn_queue()` and re-spawn each pin argv
through the spawner. `HideTrayForCapture` continues to work
unchanged for old workers (rolling-upgrade skew).

Coordination state: a small pure machine (`PrepareGate`) in
`sessions.rs` (or its own module beside `tray_gate.rs`) — inputs:
app-subscribed?, ack-received?, pins-gone?, tray-gate-done?,
deadline; output: Wait / Ready / ReadyTimedOut. Stepped by the
daemon main loop like the tray gate is today (no blocking in the
tokio task beyond the bounded reply wait the tray path already
does).

**Acceptance criteria:**
- [ ] Pure `PrepareGate` unit tests: all-clear → Ready; ack
      missing + no subscriber → Ready; deadline → ReadyTimedOut;
      pins alive → Wait
- [ ] `ipc_server` interception tests with a live-stepped gate (the
      existing tray-handshake test pattern): `PrepareCapture`
      replies `CaptureReady` exactly once the conditions hold;
      `ReportAppHidden` unblocks the ack wait
- [ ] Pin respawn: `ReportCaptured` re-spawns queued argvs verbatim
      (RecordingSpawner assertions); queue drains once
- [ ] `HideTrayForCapture` path still passes its existing tests
      untouched
- [ ] Bounds respected: no interception path can block the
      connection past the existing 2 s/3 s handshake bounds

**Verification:**
- [ ] `cargo test -p flintshot-app`
- [ ] Standing gates
- [ ] Manual smoke (worker side lands in Task 007): send
      `PrepareCapture` via a test client / `flintshot hotkeys`-style
      probe — daemon logs the coordinated steps and replies

**Dependencies:** 001 (variants), 002 (registry + queue). Not
dependent on 004/006 — no senders exist yet; every wait degrades to
vacuous/timeout.

**Files likely touched:**
- `crates/flintshot-app/src/ipc_server.rs`
- `crates/flintshot-app/src/daemon.rs` (gate stepping in the loop)
- `crates/flintshot-app/src/sessions.rs` (PrepareGate + respawn drain)
- `crates/flintshot-app/src/tray_gate.rs` (at most: shared grace const)

**Docs:** PrepareGate doc comment records the ack-then-grace
rationale (close ≠ pixels gone — ADR-0047 lesson).

**Estimated scope:** Medium-Large: 4 files
