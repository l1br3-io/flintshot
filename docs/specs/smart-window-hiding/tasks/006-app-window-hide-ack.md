# Task 006: App window hides for any capture + acks

**Description:** Close the app-window self-capture gap: the app
window must vanish from **every** grab, not just ones it triggered,
and confirm destruction to the daemon.

`HomeCore` (pure, `app_window.rs`):
- `CaptureStarted` → `in_flight += 1` **and** `hidden = true`,
  returning `Some(HomeDirective::Hide)` (new variant)
- `HideForCapture` → `hidden = true` + `Hide` directive (the
  pre-grab nudge from Task 005; idempotent with the above)
- restore logic unchanged (`SessionEnded` → `maybe_show`)

Plumbing:
- Pump: `Hide` directive → `shared.hide_requested()` (new flag).
- `HomeApp`/bridge (`home.rs`): per-frame `bridge.take_hide()` →
  `ctx.send_viewport_cmd(ViewportCommand::Close)`.
- Outer loop (`run`): after `run_home` returns, continue waiting
  when `take_pending_capture()` is `None` **but** a hide was
  requested (`hidden-by-event`) — otherwise today's code would
  treat it as a user quit and exit the process. Then
  `wait_for_reopen()` as usual.
- Ack: once closed for a hide (self-triggered or external), the
  outer loop sends best-effort `Request::ReportAppHidden` (new
  `ipc_client` helper, `ReportSaved`-style) **before** delegating
  (self-trigger path) — the daemon's post-ack grace covers the
  compositor unmap, so the current fixed `HIDE_GRACE_MS` sleep on
  the self-trigger path can shrink or go (keep behavior identical:
  verify no regression when daemon is absent — the sleep stays for
  the no-daemon standalone case).

**Acceptance criteria:**
- [ ] `HomeCore` unit tests: external `CaptureStarted` hides +
      emits `Hide`; `HideForCapture` idempotent with it; restore on
      last `SessionEnded` still passes (existing tests updated)
- [ ] Shared/outer-loop test: hide-by-event (no pending capture
      command) → loop waits for reopen instead of exiting; ack sent
      exactly once per hide
- [ ] AppFocus while hidden stays suppressed (existing behavior —
      regression test still green)
- [ ] No-daemon path: unchanged (ack helper is a silent no-op)

**Verification:**
- [ ] `cargo test -p flintshot-app`
- [ ] Standing gates
- [ ] Manual: app window open → hotkey capture → window absent from
      the shot; reopens after the editor closes; tray *Open app*
      during the capture does not raise it over the session

**Dependencies:** 001 (variants), 005 (daemon intercepts the ack;
`HideForCapture` has a sender worth coordinating with)

**Files likely touched:**
- `crates/flintshot-app/src/app_window.rs`
- `crates/flintshot-editor-egui/src/home.rs` (Hide directive + bridge)
- `crates/flintshot-app/src/ipc_client.rs` (ReportAppHidden helper)

**Docs:** `HomeDirective::Hide` doc notes it covers external
triggers (the ADR-0047 gap).

**Estimated scope:** Medium: 3 files
