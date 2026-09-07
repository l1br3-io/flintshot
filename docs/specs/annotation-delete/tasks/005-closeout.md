# Task 005: Closeout — CHANGELOG, TEST_MATRIX, ADR-0044, standing gates

**Description:** Docs + final gates. CHANGELOG `[Unreleased]` Added
entry for the delete affordance; `docs/TEST_MATRIX.md` manual rows
(both shells: click-select outline, Del/Backspace + button delete,
undo/redo symmetry, deselect paths, outline never in saved output);
ADR-0044 recording the persistent-selection lifecycle (release keeps
the selection, `set_tool` owns tool-switch deselection, selection is
chrome-only) — the non-obvious choice planning surfaced, per the
spec's open question 3 default.

**Acceptance criteria:**
- [ ] CHANGELOG `[Unreleased]` / **Added**: select-then-delete
      (Move click selects with dashed outline; `Del` / `Backspace`
      + rail button delete one annotation, fully undoable).
- [ ] TEST_MATRIX rows for the manual pass, matching the spec's
      exit criteria 1–8 shape.
- [ ] `docs/adrs/ADR-0044-persistent-selection-lifecycle.md` in the
      Context · Decision · Consequences format.
- [ ] Manual pass against spec exit criteria 1–9 on a display
      (`fullscreen`, `region`); results recorded in TEST_MATRIX.
- [ ] Standing gates green: `cargo fmt --all --check`, `cargo clippy
      --workspace --all-targets -- -D warnings`, `cargo test
      --workspace --all-targets`, `cargo test --doc -p
      flintshot-core -p flintshot-editor-egui`,
      `no_gui_types_in_core.rs` (green by construction).

**Verification:**
- [ ] All gates above run clean from the repo root.

**Dependencies:** Tasks 001–004

**Files likely touched:**
- `CHANGELOG.md`
- `docs/TEST_MATRIX.md`
- `docs/adrs/ADR-0044-persistent-selection-lifecycle.md` (new)

**Docs:**
- This task *is* the docs.

**Estimated scope:** Small (3 doc files + gate runs)
