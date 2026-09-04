# Task 003: Closeout — CHANGELOG, TEST_MATRIX, manual pass

**Description:** User-visible change: CHANGELOG under
`## [Unreleased]`; TEST_MATRIX row(s) for the move interaction in
both shells; manual pass of the spec's exit criteria via
`cargo run -- region`.

**Acceptance criteria:**
- [ ] CHANGELOG entry (move semantics + undo behavior).
- [ ] TEST_MATRIX row(s) covering: move in editor window, move in
      overlay, undo/redo round-trip, region gestures unaffected.
- [ ] Manual pass of exit criteria 1–4 recorded.
- [ ] Standing gate green:
      `cargo fmt --all --check`;
      `cargo clippy --workspace --all-targets -- -D warnings`;
      `cargo test --workspace --all-targets`;
      `cargo test --doc -p flintshot-core -p flintshot-editor-egui`.

**Dependencies:** Tasks 001, 002

**Files likely touched:**
- `CHANGELOG.md`
- `docs/TEST_MATRIX.md`
