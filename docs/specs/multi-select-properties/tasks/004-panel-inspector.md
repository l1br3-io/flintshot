# Task 004: egui: panel selection-inspector mode

**Description:** The panel grows its selection mode: when the set is
non-empty the title reads `N selected · Properties` and the body
renders exactly the intersection sections (reusing today's widgets
bound to selection properties) plus the Selection section with count
and pluralized Delete. Widget interactions bracket continuous edits
with `end_property_edit()`. No-selection mode is untouched.

**Acceptance criteria:**
- [ ] Title and section set switch on selection presence; empty
      intersection renders count + delete-only state
- [ ] Sections render per `selected_capabilities()`: Stroke (swatch
      row; stroke chips only when width-capable), Fill, Text
      (popover reuse), Effects, Number, Selection
- [ ] A swatch/preset/toggle click changes every supporting selected
      annotation immediately and produces exactly one undo entry
- [ ] Continuous widgets (DragValue, sliders, color editor)
      coalesce via `end_property_edit()` bracketing
- [ ] Delete button enabled iff selection non-empty; label
      pluralizes; fires `ToolbarAction::Delete`
- [ ] Existing no-selection panel tests pass unchanged; new run_ui
      tests cover title, per-capability section fixtures, edit
      routing, and delete gating

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui`
- [ ] `cargo test --workspace --all-targets` + clippy + fmt gates
- [ ] Manual: spec exit criteria 1–5 via `cargo run -- app`

**Dependencies:** 003

**Files likely touched:**
- `crates/flintshot-editor-egui/src/panel.rs`
- `crates/flintshot-editor-egui/src/popover.rs` (selection color target)
- `crates/flintshot-editor-egui/src/widgets.rs` (only if a section
  widget needs a selection-aware variant)

**Docs:**
- Panel-mode branch comment explaining the two modes and the
  byte-for-byte guarantee for the no-selection path.

**Estimated scope:** Medium
