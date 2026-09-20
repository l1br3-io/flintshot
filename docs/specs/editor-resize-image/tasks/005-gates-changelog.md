# Task 005: Full gates + CHANGELOG + success-criteria sweep

**Description:** Close the effort: run every gate, add the
CHANGELOG entry, and sweep the spec's success criteria one by one,
checking them off in `spec.md` only where verifiably true.

**Acceptance criteria:**

- [ ] `cargo fmt --all --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace --all-targets` passes
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
      passes
- [ ] `CHANGELOG.md` has an entry under `## [Unreleased]` describing
      the resize feature (user-visible)
- [ ] Every checkbox in `spec.md` §Success criteria is either
      checked (verified) or has a written reason it was re-scoped
- [ ] No ADR needed (no invariant changed — ADR-0015 preserved by
      design); confirm and note in the plan's Complete checkpoint

**Verification:**

- [ ] The four gate commands above, run in order, from repo root
- [ ] Manual spot-check: fresh `cargo run -- app` session exercises
      open → 75% → undo → custom width → save

**Dependencies:** Task 004

**Files likely touched:**

- `CHANGELOG.md`
- `docs/specs/editor-resize-image/spec.md` (criteria checkboxes)

**Docs:**

- CHANGELOG per AGENTS.md convention.

**Estimated scope:** XS — 2 files
