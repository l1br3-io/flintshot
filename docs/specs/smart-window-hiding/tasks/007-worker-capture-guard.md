# Task 007: Worker `CaptureGuard` (PrepareCapture → grab → ReportCaptured)

**Description:** Swap the worker's pre-grab handshake from
tray-only to full-screen-clear. In `cli.rs`:

- Generalize `TrayHiddenGuard` → `CaptureGuard` (same RAII shape):
  construction sends `Request::PrepareCapture` and waits bounded
  for `Event::CaptureReady` (the existing `hide_tray_for_capture_to`
  client helper grows a `prepare_capture_to` sibling — same
  request/reply framing, same 2 s bound, same best-effort failure
  semantics: warn + proceed). Drop → `Request::ReportCaptured`
  (unchanged) — which now also respawns pins daemon-side (Task 005).
- `capture_with_tray_hidden` becomes `capture_prepared` using the
  new guard; callers unchanged (`capture_and_dispatch`,
  `recapture_image`).
- Version-skew note: if an old daemon answers `PrepareCapture` with
  the generic error (unknown variant), the client falls back to
  `HideTrayForCapture` once, then proceeds (warn). Keeps
  new-worker/old-daemon usable during upgrades.
- Superseded-before-grab check from Task 004 stays: subscription
  flag set during the handshake wait → abandon without grabbing.

**Acceptance criteria:**
- [ ] Client helper: bounded wait for `CaptureReady`; NoDaemon →
      vacuous proceed; transport error → warn + proceed (tests
      mirror the existing `HideTrayOutcome` ones)
- [ ] Old-daemon fallback: `unsupported` reply → one
      `HideTrayForCapture` attempt → proceed (unit test with a fake
      server reply, following the existing client-test pattern)
- [ ] Guard spans exactly the grab in both `capture_and_dispatch`
      and `recapture_image` (assert via existing structure/tests)
- [ ] End-to-end manual passes Checkpoint B scenarios

**Verification:**
- [ ] `cargo test -p flintshot-app`
- [ ] Standing gates
- [ ] Manual (Checkpoint B): (1) editor open + tray capture → shot
      has zero flintshot chrome, one editor remains, pins restored
      at their positions; (2) app window open + hotkey → window
      absent, returns after

**Dependencies:** 005 (daemon side), 004 (subscription flag)

**Files likely touched:**
- `crates/flintshot-app/src/cli.rs`
- `crates/flintshot-app/src/ipc_client.rs`

**Docs:** `CaptureGuard` doc comment carries the
bounds-and-best-effort rationale (ADR-0036 extension).

**Estimated scope:** Medium: 2 files
