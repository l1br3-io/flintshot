# Task 002: Toolbar — `ToolbarAction::Delete` + trash glyph + both rails

**Description:** The button affordance. `toolbar.rs` gains a
`ToolbarAction::Delete` variant, a painter-drawn trash glyph
(`g_trash`, the 2 px stroke language of the other glyphs), and a
Delete chip beside Undo/Redo on the editor rail (`draw_rail`) and on
the overlay rail row (`overlay_rail_row` in `overlay.rs`). The chip
is enabled iff `state.selected_index.is_some()` (both rails already
hold `&mut EditorState`), reading greyed / hover-only / no click when
disabled — the same `enabled` param the locked tool chips use. Both
shells' action matches gain a `Delete` arm replaying the identical
`Key::Delete` (no modifiers) dispatch the keyboard path produces.

**Acceptance criteria:**
- [ ] `ToolbarAction::Delete` variant documented; exhaustive-match
      fallouts (editor `ui_impl`, overlay `apply_toolbar_action`)
      compile only with the new arms.
- [ ] `g_trash` glyph: lid + can in the shared glyph-space style;
      visually distinct from Save's tray (test: renders via the
      chip path without panic; glyph distinctness is manual).
- [ ] Editor rail renders `Delete · Del` after Redo; overlay rail
      row renders it after Redo (`overlay_delete` id).
- [ ] Chip enabled iff `state.selected_index.is_some()`: greyed +
      hover-only + inert with no selection (synthetic-context click
      test), enabled + emitting `ToolbarAction::Delete` with one.
- [ ] Both action arms replay `InputEvent::Key { key: Key::Delete,
      modifiers: default }` through `dispatch_input` — the same
      single dispatch path as the `Del` key (test: clicking with a
      selection removes the annotation through the shell handler).
- [ ] Toolbar unit tests: enabled predicate + click emission +
      greyed inertness, following the `tool_after_pen_click`
      synthetic-context pattern.

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui --quiet`
- [ ] `cargo clippy -p flintshot-editor-egui --all-targets -- -D warnings`
- [ ] Manual (needs display, may ride task 003's pass): greyed chip
      with no selection, enabled + deletes with one, both rails.

**Dependencies:** Task 001 (needs `Key::Delete` in the replay and
persistent `selected_index` to gate on)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/toolbar.rs`
- `crates/flintshot-editor-egui/src/app.rs` (action arm only)
- `crates/flintshot-editor-egui/src/overlay.rs` (rail row + action
  arm only)

**Docs:**
- None — CHANGELOG at task 005.

**Estimated scope:** Medium (3 files; two are small match/glyph
additions)
