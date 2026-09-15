# Task 006: In-process capture (button → grab → editor page)

**Description:** The core vertical: the app window captures for itself. A
Capture click registers intent (existing) and closes the window; the outer
loop notifies the pump; the pump spawns a **grab thread** that runs the
Task-002 `capture_flow` pieces — backend resolve (CLI > config > auto), a
`CaptureGuard`-style prepare handshake, `native_grab_prepared` /
`capture_prepared` per `supports_selection()`, quiet-exit handling — and
hands the `RgbaImage` back. The pump then arms `PendingPage::Editor` with
the right landing state (plain `"capture"` mode on the native path;
selection-first `"region"` on the fallback — same labels as the worker) and
the outer loop opens the editor epoch. The pump keeps pumping while the
grab runs: the handshake's `PrepareCapture`→`CaptureReady` wait is a
spawned task, and because the window is already destroyed the
`ReportAppHidden` ack is sent immediately on `HideForCapture` (or before,
if the gate is already armed). No daemon: everything degrades exactly like
the worker path (`CaptureGuard` NoDaemon arms). A dismissed OS selection
UI = quiet exit → reopen the page that was open (home). Introduce the
pump-side capture state machine (`Idle / Grabbing`) so a re-trigger during
a grab restarts it cleanly (Task 007 depends on this).

**Acceptance criteria:**
- [ ] Native path: Capture click → window closes → OS selection UI →
      editor page opens with the selection; cancel in the OS UI → home
      reopens, no error surface
- [ ] Fallback path (`--backend xcap` config or pin): editor page opens
      selection-first over the primary-monitor grab
- [ ] Daemon absent (kill it): the flow still works; logs show the
      handshake degrading
- [ ] Window is never in its own shot (wayland + x11 manual check)
- [ ] State-machine unit tests: trigger-while-grabbing restarts; grab
      completion routes to Editor; quiet exit routes to the prior page

**Verification:**
- [ ] Full gate green
- [ ] Manual matrix on the dev session: portal happy path, portal cancel,
      xcap fallback, daemon-less run

**Dependencies:** Tasks 002, 003, 004

**Files likely touched:**
- `crates/flintshot-app/src/app_window.rs` (pump grab thread, capture state
  machine, routing)
- `crates/flintshot-app/src/capture_flow.rs` (entry the pump calls;
  `CaptureGuard`/handshake reuse may move pieces here)
- `crates/flintshot-app/src/cli.rs` (shared helpers, if any relocate)

**Docs:** GOTCHA at the handshake-await: why the ack is immediate (window
closed before the request left) and why the wait is a spawned task (pump
must stay responsive).

**Estimated scope:** Large: 3 files — acceptable; one coherent session, and
splitting would separate the grab from its routing.
