# Task 008: Loading route + in-window delivery

**Description:** Close the loop: on `AppGrabEvent::Confirmed` the
pump routes `Show` with the start route = Loading; the generation
opens at the editor's stable size with a spinner ("retrieving
capture…") and a repaint heartbeat. On `Finished` with the window
open, the image delivers into the open window through the shared
slot + a wake (a new `HomeDirective` variant or a dedicated slot;
the egui `Context` is documented-safe for cross-thread
`request_repaint`, the `CloseHandle` precedent) — the router swaps
Loading → Editor in-place. If the read-back out-raced the window
(window still closed), `Finished` opens the generation directly on
Editor with the image (no spinner flash). Failures and quiet exits
surface correctly from either phase: failure while Loading shows
the error status and routes home; a quiet exit (dismissed /
superseded) reopens the prior route with no error; a supersede
arriving mid-read-back folds silently (the generation machine
already discards stale outcomes — extend to both phases).

**Acceptance criteria:**
- [ ] After confirming an OS-selection capture (tray, home, `+`,
      hotkey), the editor window is on screen with the progress
      indicator within ~200 ms of the confirm; the image swaps
      in-place when the grab delivers (no window recreation)
- [ ] Delivery-order coverage unit-tested at the pump/router seam:
      deliver-to-open-window, deliver-to-closed-window (fast
      read-back opens Editor directly), superseded mid-read-back
      folds, failure-while-Loading routes home with status
- [ ] Direct grabs (`Ready`) skip the spinner (open on Editor)
- [ ] A wedged read-back (portal write never lands) surfaces the
      existing timeout error on the open page — never a hang, never
      a stranded spinner

**Verification:**
- [ ] `cargo test -p flintshot-app -p flintshot-editor-egui` green
      (delivery state machine + Loading route tests)
- [ ] Full gate green (see Task 001's gate commands)
- [ ] Manual on the dev session: tray capture → confirm selection
      in the OS UI → spinner editor appears immediately → image
      swaps in; repeat from home Capture and `+`; kill the portal
      mid-read-back (or simulate via a pinned config) → error
      surfaces, Esc returns home

**Dependencies:** Task 006 (router + Editor route), Task 007
(two-phase delivery)

**Files likely touched:**
- `crates/flintshot-app/src/app_window.rs` (pump routing: Confirmed
  → Loading, Finished → deliver)
- `crates/flintshot-editor-egui/src/{window,home}.rs` (Loading
  route UI; delivery directive/slot + wake)
- `crates/flintshot-editor-egui/src/app.rs` (Editor route receives
  a swapped-in state)

**Docs:**
- GOTCHA (cross-thread wake pattern) → inline comment at the wake
  site referencing the `CloseHandle` precedent.

**Estimated scope:** Medium-Large: 4–5 files
