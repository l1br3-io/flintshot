# Task 001: Core — `Key::Delete` + persistent selection + `delete_selected` + `set_tool`

**Description:** The full core state machine behind select-then-delete.
`geom.rs` gains the `Key::Delete` variant. `editor.rs` makes the Move
tool's selection persistent (release keeps `selected_index`; a Move
press that misses every annotation clears it — in both the normal and
the text-mode commit-then-press branches), adds
`EditorState::delete_selected()` following the snapshot discipline of
every other mutation, dispatches plain `Del` / `Backspace` to it
(any modifier spoils the chord), routes tool switches through a new
`set_tool` that drops the selection when leaving Move, and keeps
`undo` / `redo` / `crop` clearing the selection via `end_move`.
Deleting an annotation whose drag is in flight ends the drag first.

**Acceptance criteria:**
- [ ] `Key::Delete` variant added to `geom.rs` with a doc comment;
      no exhaustive-match fallouts (existing matches use wildcards).
- [ ] Move `PointerUp` clears `moving` but **keeps** `selected_index`
      — both the click-without-drag and the post-move release.
- [ ] Move `PointerDown` miss clears `selected_index`; hit replaces
      it with the newly grabbed index (last click wins).
- [ ] `delete_selected()`: no-op with no selection; otherwise ends
      any in-flight drag, pushes pre-delete `Snapshot::anns_only`
      (with `next_number`), clears `redo`, removes
      `anns[selected_index]`, clears the selection. Undo restores
      exactly the deleted annotation (redo re-deletes);
      `next_number` and sibling annotations untouched by the delete.
- [ ] `dispatch_key`: `Key::Delete | Key::Backspace` with no
      modifiers → `delete_selected()`, outcome `Continue`; with any
      modifier → no-op (reserved).
- [ ] Text-draft mode: `Backspace` edits the draft; committed anns
      untouched (the modal branch owns it — pin with a test).
- [ ] `undo()` / `redo()` / `crop()` clear `selected_index` (they
      call `end_move`; tests pin it survives no longer).
- [ ] `set_tool(tool)` sets the tool and clears `selected_index`
      when switching away from Move; staying on / switching to Move
      preserves it. Doc comment names it the selection-lifecycle
      owner for shell-side switches.
- [ ] Deleting the annotation being dragged leaves `moving == None`
      and a consistent `anns`.
- [ ] Unit tests for every rule above, in the `editor.rs` test
      module (same suite as undo/move).

**Verification:**
- [ ] `cargo test -p flintshot-core --quiet`
- [ ] `cargo clippy -p flintshot-core --all-targets -- -D warnings`
- [ ] Full standing gates green (fmt / workspace clippy / workspace
      tests / doc tests) — existing Move tests updated only where
      they pinned the old release-clears-selection behaviour.

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-core/src/geom.rs`
- `crates/flintshot-core/src/editor.rs`

**Docs:**
- CHANGELOG + ADR land at task 005; new public items carry doc
  comments here (`#![warn(missing_docs)]`).

**Estimated scope:** Medium (2 files, one cohesive state machine +
test suite)
