# Task 001: Core — `Annotation::translate` + dispatcher move state machine

**Description:** Add the pure per-variant translate helper, the
`AnnotationDrag` in-flight state (index, grab offset, moved flag),
and wire Select-tool pointer events through `dispatch_input`:
`PointerDown` hit-tests before `begin_selection`, `PointerDrag`
translates with a lazy undo snapshot on first movement, `PointerUp`
ends the drag. `undo`/`redo`/`crop` cancel an in-flight move.
`selected_index` is set on grab and cleared on release.

**Acceptance criteria:**
- [ ] `Annotation::translate(dx, dy)` shifts every geometry field
      verbatim for all nine variants (doc comment present).
- [ ] `EditorState::moving: Option<AnnotationDrag>` (public,
      documented); `AnnotationDrag { index, grab_offset, moved }`
      documented, `Debug, Clone, Copy, PartialEq`.
- [ ] Select `PointerDown` on a committed annotation starts a move
      (no selection change); on empty space starts a selection
      exactly as before.
- [ ] Drag translates the annotation so the grab point tracks the
      cursor; size/style/index unchanged.
- [ ] First actual movement pushes exactly one
      `Snapshot::anns_only` and clears `redo`; zero movement pushes
      nothing.
- [ ] `PointerUp` ends the move, clears `moving` and
      `selected_index`.
- [ ] `undo` / `redo` / `crop` clear `moving` + `selected_index`
      before swapping anns.
- [ ] Unit tests: move lifecycle, single-snapshot-per-gesture,
      click-without-move, undo-mid-drag restore, fresh-selection
      regression, per-variant translate geometry, degenerate-bbox
      fallback (empty polyline press falls back to selection).

**Verification:**
- [ ] `cargo test -p flintshot-core`
- [ ] `cargo clippy -p flintshot-core --all-targets -- -D warnings`

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-core/src/editor.rs`
