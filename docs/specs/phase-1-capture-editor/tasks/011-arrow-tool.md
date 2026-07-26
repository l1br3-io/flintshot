# Task 011 — Arrow tool end-to-end

> Phase 1.E · size: S · depends on: 008

## Description

`Tool::Arrow` becomes a working tool. Same input model as Rect (down →
drag → up), produces `Annotation::Arrow`. Adds a toolbar button.
`EguiCanvas::draw_arrow` gets a real implementation (line + two barbs at
the head). The shared `render()` dispatches the new variant.

## Acceptance criteria

- [ ] Drag with Arrow tool active commits an `Annotation::Arrow` with
      tail at the down point and head at the up point.
- [ ] `head_size` defaults to 10 physical px; subsequent arrows inherit
      `state.stroke_width` for the shaft.
- [ ] `EguiCanvas::draw_arrow` renders barbs at ±30° from the shaft
      direction, length = `head_size`.
- [ ] Toolbar adds an Arrow button; clicking switches `state.tool`.
- [ ] Undo/redo works for arrows (no extra plumbing — Task 7 covers it).

## Verification

- [ ] Standing gate passes.
- [ ] `MockCanvas` snapshot test for an `EditorState` with one Arrow
      ann: assert exact `draw_arrow` call arguments.
- [ ] Manual: draw an arrow over a screenshot, verify head/tail
      orientation; save and reopen.

## Dependencies

Task 8.

## Files likely touched

- `crates/flintshot-core/src/render.rs` (Arrow dispatch)
- `crates/flintshot-editor-egui/src/canvas.rs` (draw_arrow impl)
- `crates/flintshot-editor-egui/src/shell.rs` (Arrow tool input, toolbar
  button)

## Size

S
