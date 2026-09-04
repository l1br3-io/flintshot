# Task 002: Overlay — gesture fall-through + hover cursor

**Description:** In the fullscreen overlay, a Select-tool press on
an annotation must reach the dispatcher (which now starts the
move) instead of being intercepted as a region move.
`gesture_for_press` gains an `anns` parameter and returns `None`
for an annotation hit — ordered after handles, before the
region-interior branch. `hover_cursor` reports `Move` over an
annotation under Select. No new `Gesture` variant: drags dispatch
`PointerDrag` through the existing `drag_active` wiring.

**Acceptance criteria:**
- [ ] `gesture_for_press(sel, anns, at, tool, hit_radius)`:
      handle hit → `Resize`; annotation hit → `None` (fall through
      to dispatcher); region interior → `Move`; else `None`.
      Annotation-tool presses still return `None` (draw path).
- [ ] `hover_cursor`: handle cursor → annotation `Move` → region
      interior `Move` → `Crosshair`.
- [ ] Existing region move/resize behavior untouched (existing
      `gesture_for_press_*` / `moved_rect_*` tests keep passing,
      updated for the new signature).
- [ ] New tests: annotation press inside region falls through;
      handle precedence over annotation hit; annotation-tool press
      on an annotation still falls through to the draw path.

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui`

**Dependencies:** Task 001 (the dispatcher must own the move
before the fall-through is observable end-to-end; the pure-fn
tests could land either way, but the feature is only real once
core handles it).

**Files likely touched:**
- `crates/flintshot-editor-egui/src/overlay.rs`
