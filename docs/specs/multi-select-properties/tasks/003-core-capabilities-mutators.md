# Task 003: Core: capabilities query + property mutators + undo coalescing

**Description:** Add the pure intersection query
(`selected_capabilities`) and one mutator per property section
(color, stroke width, fill, text style fields, blur kind/strength,
number diameter). Mutators write only fields every selected
annotation carries, re-apply `MARKER_ALPHA` for markers, sync the
corresponding session tool default, and coalesce undo entries by
edit kind (plan.md Architecture Decision 4).

**Acceptance criteria:**
- [ ] `SelectionCapabilities` (`#[non_exhaustive]` bool struct) +
      `selected_capabilities()` returning the full spec matrix
      (property × annotation type), unit-tested per cell
- [ ] Mutators skip non-supporting members; marker color edits
      re-apply `MARKER_ALPHA`; defaults (`color`, `stroke_width`,
      `fill_shapes`, text fields, `blur_kind`, `number_diameter`)
      sync
- [ ] Undo coalescing: consecutive same-kind mutator calls = one
      snapshot; kind change or `end_property_edit()` starts a new
      entry; discrete calls after `end_property_edit` each snapshot
- [ ] No mutator touches `pending` or the selection set; all are
      no-ops on an empty selection
- [ ] `no_gui_types_in_core.rs` still passes (pure core API only)

**Verification:**
- [ ] `cargo test -p flintshot-core` (matrix, coalescing sequence, alpha, default-sync tests)
- [ ] `cargo test --workspace --all-targets`

**Dependencies:** 001

**Files likely touched:**
- `crates/flintshot-core/src/editor.rs` (capabilities + mutators may
  move to a new `crates/flintshot-core/src/inspector.rs` module if
  editor.rs grows past comfortable size — decide during build)

**Docs:**
- Doc comments on every public item (`missing_docs` warns in core);
  the coalescing contract gets a worked example in the
  `end_property_edit` doc.

**Estimated scope:** Medium
