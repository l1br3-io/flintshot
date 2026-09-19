# Task 006: Closeout

**Description:** Record the effort: CHANGELOG entry, TEST_MATRIX
rows, an ADR for the property-edit undo-coalescing mechanism, and a
final full-gate + manual pass of the spec's exit criteria.

**Acceptance criteria:**
- [ ] `CHANGELOG.md` `## [Unreleased]` entry (user-visible summary)
- [ ] `docs/TEST_MATRIX.md` rows: shift-select, group move, group
      delete, inspector edits, per-shell chrome
- [ ] ADR-NNNN: context/decision/consequences for kind-based undo
      coalescing + shell bracketing
- [ ] All seven spec exit criteria verified manually; standing gates
      green (`fmt --check`, `clippy -D warnings`, workspace tests,
      doc tests for the two lib crates)

**Verification:**
- [ ] The four standing commands from AGENTS.md, in order
- [ ] Manual walkthrough of spec §Exit criteria

**Dependencies:** 001–005

**Files likely touched:**
- `CHANGELOG.md`
- `docs/TEST_MATRIX.md`
- `docs/adrs/ADR-NNNN-property-edit-undo-coalescing.md`

**Docs:**
- This task *is* docs.

**Estimated scope:** Small
