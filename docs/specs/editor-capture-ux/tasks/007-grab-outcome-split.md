# Task 007: Grab outcome split — Ready / AwaitClipboard

**Description:** Split `native_grab_prepared` at the
`capture_selection` return so the pump can open the window before
the portal's clipboard write lands. New shape: a `NativeGrab` enum
— `Ready(RgbaImage)` (the direct path) vs `AwaitClipboard {
pre_grab }` (the `clipboard:///` marker path) — with
`finish_clipboard_grab(AwaitClipboard)` running the existing
read-back wait unchanged. The app-window grab thread delivers in
two phases through the grabs channel: `AppGrabEvent::Confirmed`
(then it continues the read-back) and
`AppGrabEvent::Finished(AppGrab)`; the pump folds repeats and
discards stale generations at either phase. Behavior-neutral on
its own: on `Confirmed` the pump logs only (Task 008 routes it);
`Finished` routes exactly as today. Tests ride the existing
`FakeCapturer` doubles.

**Acceptance criteria:**
- [ ] `NativeGrab::Ready` / `AwaitClipboard` classification
      unit-tested with `FakeCapturer` (ok / cancelled / marker /
      hard-fail), including the quiet-exit arms
- [ ] Grab-thread two-phase delivery unit-tested: Confirmed always
      precedes Finished; a superseded watch short-circuits before
      the backend (existing test) and folds Confirmed silently
- [ ] The monitor-fallback arm (no selection support) is unchanged
      (single-phase `Finished`)
- [ ] Interim behavior identical to today (`Confirmed` logs;
      `Finished` routes) — no user-visible change

**Verification:**
- [ ] `cargo test -p flintshot-app` green (new classification +
      delivery tests)
- [ ] Full gate green (see Task 001's gate commands)
- [ ] Manual: a `+` capture on the dev session behaves exactly as
      before this task

**Dependencies:** None hard; ordered after Task 006 so Task 008
wires routing onto a landed router (Task 007 itself is
`capture_flow.rs`-local)

**Files likely touched:**
- `crates/flintshot-app/src/capture_flow.rs`
- `crates/flintshot-app/src/app_window.rs` (pump select arm:
      interim Confirmed handling)

**Docs:**
- GOTCHA (read-back timing rationale) already documented at
  `read_portal_clipboard_grab`; extend only if the split surfaces
  something new.

**Estimated scope:** Medium: 2 files
