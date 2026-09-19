# Task 003: Size stepper in the panel's Number section

**Description:** Surface the size control. In
`flintshot-editor-egui/src/panel.rs`, the Number section's
`number_row` gains a Size control: an `egui::DragValue` bound to
`&mut state.number_diameter` with
`.range(NUMBER_MIN_DIAMETER..=NUMBER_MAX_DIAMETER).speed(1.0)`,
labeled "Size", styled identically to the existing "Start at" stepper
row (`TYPE_SM` label, same horizontal layout). Import the two consts
from `flintshot-core` — no literal duplication. The control mutates
session tool state only: no history entries, no effect on committed
bubbles, no persistence.

**Acceptance criteria:**
- [ ] Number section renders both rows: "Start at" (existing,
      unchanged) and "Size" (new DragValue on
      `state.number_diameter`).
- [x] DragValue range comes from the imported core consts (16.0–96.0);
      typing/dragging outside the range clamps at the bounds.
- [ ] Fresh editor session shows 28 (the core default); the value
      survives tool switches within the session.
- [ ] Committed bubbles on canvas do not change while dragging the
      stepper; the next placed bubble uses the new diameter.
- [x] No other panel section, tool hint, or the overlay rail changes.

**Verification:**
- [x] `cargo clippy --workspace --all-targets -- -D warnings` and
      `just ci` green.
- [ ] Manual (`cargo run -- editor` → Number tool): set Size 60 →
      next bubble lands at 60 px with centered digit; set 16 → small
      but centered; committed bubbles keep their sizes; Save → PNG
      matches the editor at both extremes (WYSIWYG at max diameter
      covers the metrics-drift risk from the plan).
- [ ] Manual: stepper input of 8 / 200 clamps to 16 / 96.

**Dependencies:** Task 2 (field + pub consts)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/panel.rs` (`number_row` + import)

**Docs:**
- None beyond code; panel behavior notes live in the section's
  existing comment style.

**Estimated scope:** Small (1 file)
