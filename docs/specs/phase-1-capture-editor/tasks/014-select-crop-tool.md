# Task 014 — Select / crop region tool

> Phase 1.F · size: M · depends on: 008

## Description

`Tool::Select` lets the user drag-select a rect and crop the editor to
that region. Crop replaces `state.image` with the cropped sub-image
(translated so the new origin is `(0, 0)`), and translates every
existing annotation the same way. This is the "drag-select a region"
promise in Phase 1's exit criteria.

## Acceptance criteria

- [ ] Dragging with `Tool::Select` shows a selection rectangle.
- [ ] Confirming the selection (button, double-click, or Enter) crops
      `state.image` to the selected rect.
- [ ] Existing annotations translate correctly (subtract the crop
      origin); annotations fully outside the crop are dropped; partial
      overlap is clipped.
- [ ] Crop is undoable (snapshot of pre-crop image + anns pushed to
      `undo`).
- [ ] `EditorOutcome` is unaffected — crop is an in-editor action.

## Verification

- [ ] Standing gate passes.
- [ ] Unit test: `EditorState::crop(rect)` on a known state produces
      the expected translated `anns` and cropped `image`.
- [ ] Manual: select a small region, crop, draw a new rect, save, verify
      the saved PNG matches the cropped dimensions.

## Dependencies

Task 8 (selection rect plumbing reuses Rect's input shape).

## Files likely touched

- `crates/flintshot-core/src/editor.rs` (`crop` method, selection state)
- `crates/flintshot-editor-egui/src/shell.rs` (Select tool input,
  confirm-crop UX)

## Size

M
