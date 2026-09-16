# Task 006: Floating on-canvas controls

**Description:** New `floating.rs`: undo/redo, magnifier toggle, and
zoom controls render as lightweight cutout chips floating over the
canvas (top-left cluster), removed from the panel footer. Chips use
the shared cutout stroke/shadow tokens and a compact hover-lift;
they must not intercept canvas gestures except on direct hit
(pointer-miss passes through to the canvas).

**Acceptance criteria:**
- [ ] Undo/redo/magnifier/zoom operate from floating chips;
      equivalents gone from the panel
- [ ] Pointer events on the canvas outside chip rects behave exactly
      as before (annotation drags unaffected) — gating tests/manual
      matrix from Task 005 extended to floats
- [ ] Magnifier behavior (modifier-held loupe) unchanged

**Verification:**
- [ ] Full suite + clippy green
- [ ] Manual: annotate near the floating cluster; gesture starts
      correctly when not over a chip

**Dependencies:** Task 005

**Files likely touched:**
- `crates/flintshot-editor-egui/src/floating.rs` (new)
- `crates/flintshot-editor-egui/src/app.rs`
- `crates/flintshot-editor-egui/src/panel.rs`

**Docs:**
- None expected.

**Estimated scope:** Medium (3 files)
