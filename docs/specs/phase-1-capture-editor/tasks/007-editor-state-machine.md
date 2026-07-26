# Task 007 — Core editor state machine + `MockCanvas` test infrastructure

> Phase 1.C · size: M · depends on: 004

## Description

Add the editor logic core needs to support interactive tools. New
`EditorState` methods: `begin_annotation(at: Point)`,
`update_annotation(at: Point)`, `commit_annotation()`,
`cancel_annotation()`, `undo()`, `redo()`. New field
`pending: Option<Annotation>` holds the in-flight annotation; `render`
is extended to draw it (so the user sees live feedback). Also add a
`MockCanvas` in `flintshot-core::render` that records drawing calls into
a `Vec<DrawCall>` for snapshot tests.

This task delivers the *contract* every annotation tool will use; it's
M-sized because getting it right pays off across Tasks 8 / 11 / 12 / 13.

## Acceptance criteria

- [x] `EditorState::pending: Option<Annotation>` field added; documented.
- [x] `begin_annotation` / `update_annotation` / `commit_annotation`
      drive `pending` correctly; commit pushes a snapshot of `anns`
      onto `undo`, clears `redo`, appends to `anns`.
- [x] `undo` / `redo` move `anns` snapshots correctly; `undo` with
      empty history is a no-op; `redo` after a new commit is a no-op.
- [x] `cancel_annotation` drops `pending`.
- [x] `render()` draws `pending` on top of `anns`.
- [x] `MockCanvas` records every `Canvas` call into a public
      `Vec<DrawCall>`; `DrawCall` is an enum with one variant per Canvas
      method (with arguments).
- [x] Hit-testing helper `EditorState::hit_test(point: Point) ->
      Option<usize>` returns the topmost annotation under the cursor
      (used in Phase 1.F's select tool).

## Verification

- [x] Standing gate passes.
- [x] Unit tests in `editor.rs`:
  - `undo_redo_snapshot_roundtrip` — push 3 anns, undo twice, redo
     once, verify exact `anns` contents at each step.
  - `commit_clears_redo` — committing after an undo wipes the redo
     stack.
  - `cancel_drops_pending` — pending is None after cancel.
- [x] Unit test in `render.rs` using `MockCanvas`: render an
      `EditorState` with one Rect ann and one pending Rect; assert the
      recorded `DrawCall` sequence.

## Dependencies

Task 4 (render body exists).

## Files likely touched

- `crates/flintshot-core/src/editor.rs` (state machine)
- `crates/flintshot-core/src/render.rs` (MockCanvas, DrawCall, draw
  `pending`)
- `crates/flintshot-core/src/lib.rs` (re-export MockCanvas)

## Size

M
