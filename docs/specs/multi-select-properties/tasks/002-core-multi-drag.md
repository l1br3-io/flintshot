# Task 002: Core: multi-annotation drag

**Description:** Generalize the in-flight move so a drag translates
every selected annotation by the incremental pointer delta, with the
existing lazy-snapshot contract (first actual movement = the one
undo entry; click-without-drag records nothing). Degenerate-bbox
members are skipped, not fatal.

**Acceptance criteria:**
- [ ] `AnnotationDrag` tracks the last pointer position; each
      `update_move` applies `translate(dx, dy)` to all selected
      members (grab-offset feel preserved for the single case)
- [ ] One drag gesture = exactly one undo entry regardless of set
      size; undo mid-drag is safe (existing clear rules)
- [ ] Members without a bounding box are skipped during the drag
- [ ] Single-selection behavior is indistinguishable from today
      (regression: existing move tests pass unchanged)
- [ ] The text-draft `PointerDown` commit-then-route path mirrors
      the same Shift-aware selection routing

**Verification:**
- [ ] `cargo test -p flintshot-core` (new multi-drag tests + existing move suite)
- [ ] `cargo test --workspace --all-targets`
- [ ] Manual: two rects move together; one Ctrl+Z restores both

**Dependencies:** 001

**Files likely touched:**
- `crates/flintshot-core/src/editor.rs`

**Docs:**
- GOTCHA comment where the drag delta is computed (why incremental,
  not absolute — index stability + grab-point contract).

**Estimated scope:** Small
