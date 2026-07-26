# Task 011 — Arrow tool end-to-end

> Phase 1.E · size: S · depends on: 008

## Description

`Tool::Arrow` becomes a working tool. Same input model as Rect (down →
drag → up), produces `Annotation::Arrow`. Adds a toolbar button.
`EguiCanvas::draw_arrow` gets a real implementation (line + two barbs at
the head). The shared `render()` dispatches the new variant.

## Acceptance criteria

- [x] Drag with Arrow tool active commits an `Annotation::Arrow` with
      tail at the down point and head at the up point.
- [x] `head_size` defaults to 10 physical px; subsequent arrows inherit
      `state.stroke_width` for the shaft.
- [x] `EguiCanvas::draw_arrow` renders barbs at ±30° from the shaft
      direction, length = `head_size`.
- [x] Toolbar adds an Arrow button; clicking switches `state.tool`.
- [x] Undo/redo works for arrows (no extra plumbing — Task 7 covers it).

## Verification

- [x] Standing gate passes.
- [x] `MockCanvas` snapshot test for an `EditorState` with one Arrow
      ann: assert exact `draw_arrow` call arguments.
- [ ] Manual: draw an arrow over a screenshot, verify head/tail
      orientation; save and reopen. _(interactive verification owed —
      see Checkpoint D / E close-out)_

## Dependencies

Task 8.

## Files likely touched

- `crates/flintshot-core/src/render.rs` (Arrow dispatch)
- `crates/flintshot-editor-egui/src/canvas.rs` (draw_arrow impl)
- `crates/flintshot-editor-egui/src/shell.rs` (Arrow tool input, toolbar
  button)

## Size

S
