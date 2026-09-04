# Task 004: Hover affordances — Move cursor + dashed bbox, both shells

**Description:** Under `Tool::Move`, hovering a committed
annotation shows the `Move` cursor and a thin dashed bounding-box
outline in both shells. The overlay already has the hover cursor
(`hover_cursor`); it gains the bbox chrome. The editor window has
no hover-cursor logic at all; it gains both. Chrome composes the
existing core queries (`hit_test` + `Annotation::bounding_box`)
shell-side — read-only, no dispatcher events, no history entries,
no new core public API (spec boundary: ask first).

**Acceptance criteria:**

- [ ] One shared `pub(crate)` chrome helper (home: `app.rs` or a
      small `chrome` module — decide in review) converts a hover
      hit into the dashed-bbox rect: `hit_test(at)` →
      `anns[i].bounding_box()`, physical → logical transform
      matching each shell's canvas mapping
- [ ] Editor window: with Move active and no drag in flight,
      hovering an annotation sets `CursorIcon::Move`; hovering
      empty canvas sets no cursor (default); other tools unchanged
      (Move-grab cursor only, per spec scope)
- [ ] Overlay: same chrome paints near its mask/hint paint site;
      existing `hover_cursor` behavior untouched (cursor already
      correct)
- [ ] Chrome visual: 1 px dashed stroke in the existing
      selection-stroke visual language (theme constant), clearly
      subordinate to real selection chrome; nothing paints while a
      gesture/drag is active or the toolbar is hovered
- [ ] Hover never mutates `EditorState` (no `selected_index`
      changes — that stays grab-only per the annotation-move
      contract)
- [ ] Pure parts unit-tested: hit → bbox composition, rect
      transform, cursor mapping

**Verification:**

- [ ] `cargo test -p flintshot-editor-egui` — helper + cursor
      mapping tests
- [ ] Manual: draw a rect + a text label, switch to Move (`V`) →
      hover each: cursor changes, dashed outline tracks the
      annotation (Text's is a coarse estimate — acceptable, noted
      in `bounding_box` docs); hover empty canvas: nothing; both
      shells; `Ctrl+Z` count unchanged by hovering

**Dependencies:** Task 001 (Move as resting state makes the
affordance meaningful); ideally after Task 003 so chrome styling
lands with the rail language settled

**Files likely touched:**

- `crates/flintshot-editor-egui/src/app.rs` (editor hover cursor,
  chrome paint or helper home)
- `crates/flintshot-editor-egui/src/overlay.rs` (chrome paint at
  its site)
- `crates/flintshot-editor-egui/src/theme.rs` (dashed stroke
  constant)

**Docs:**

- None (the `bounding_box` doc already anticipates this use).

**Estimated scope:** Medium: 3 files
