# Task 008: Closeout — CHANGELOG, TEST_MATRIX, ADR, gates

**Description:** Record the effort. Add the CHANGELOG entry under
`## [Unreleased]` (user-visible: select / re-edit / restyle /
delete committed text). Add `docs/TEST_MATRIX.md` rows covering the
new flows. Write the ADR for the committed-annotation write-through
model (annotations stop being pure style snapshots; selection makes
them restylable; lift-out sessions keep undo exact) in `docs/adrs/`
(numbered `ADR-NNNN`, Context · Decision · Consequences). Run the
full standing gate and walk the spec's exit criteria 1–6.

**Acceptance criteria:**

- [ ] CHANGELOG entry present; TEST_MATRIX rows for select /
      re-edit / restyle / delete in both shells.
- [ ] ADR covers the write-through model, DoubleClick event, and
      lift-out session with its undo invariants.
- [ ] Spec exit criteria 1–6 each verified (manually where
      behavioral).

**Verification:**

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`

**Dependencies:** Tasks 001–007

**Files likely touched:**

- `CHANGELOG.md`
- `docs/TEST_MATRIX.md`
- `docs/adrs/ADR-NNNN-text-write-through.md` (next free number)

**Docs:**

- This task *is* the docs task.

**Estimated scope:** Small (3 files, no logic)
