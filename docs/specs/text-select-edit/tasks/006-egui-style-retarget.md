# Task 006: egui — style surfaces retarget (panel + popover)

**Description:** Retarget the Text style surfaces so they read from
— and write through to — the selection. Source order: re-edit
session draft > Text-tool selection > session defaults (current
behavior, byte-identical when nothing is selected). Writes to a
session draft or selection route through the Task 005 gesture API;
writes to session defaults keep the existing direct-field writes.
Surfaces: `panel.rs` `text_style_row` (font size −/+/DragValue) and
`text_style_popup` (family / weight / style / align / outline /
background), and `popover.rs` `ColorTarget::Text` (the shared color
pod — this is what gives the overlay color restyle for free).
Gesture wiring: discrete controls (buttons, selectable values,
swatches, toggles) wrap one begin/restyle/end; the DragValue begins
on the first `changed()` while dragging and ends on release.

**Acceptance criteria:**

- [ ] With a Text-tool selection: panel + pod display the selected
      annotation's values; changes restyle it immediately, sync
      defaults (next placed text inherits), and are undoable one
      entry per gesture (a full DragValue drag = one entry).
- [ ] Mid-session: panel edits apply live to the draft.
- [ ] No selection / no session: behavior byte-identical to today
      (fresh-draft mid-typing still does *not* live-restyle — spec
      open question default).

**Verification:**

- [ ] `cargo test -p flintshot-editor-egui` (source-switch tests
      per surface)
- [ ] Manual: select → change size / color / family / align → live
      update + next text inherits; drag the size DragValue → one
      `Ctrl+Z`; both shells (overlay via the color pod).

**Dependencies:** Task 005

**Files likely touched:**

- `crates/flintshot-editor-egui/src/panel.rs`
- `crates/flintshot-editor-egui/src/popover.rs`

**Docs:**

- None beyond code comments; the source-order rule belongs in the
  `panel` module doc.

**Estimated scope:** Medium (2 files)
