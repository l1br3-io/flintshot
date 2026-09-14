# Task 002: CHANGELOG entry + spec reconcile

**Description:** Add a **Fixed** entry under `## [Unreleased]` in
`CHANGELOG.md`: COSMIC's screenshot portal answers Copy-confirmed
captures with `clipboard:///`; flintshot rejected it and the
capture died invisibly — now the grab is read back from the
clipboard and post-grab dispatch proceeds. Reconcile
`docs/specs/portal-clipboard-uri/spec.md` only if implementation
reality diverged.

**Acceptance criteria:**
- [x] CHANGELOG `Fixed` entry names the symptom (capture did
      nothing when confirming with Copy) and the fix (clipboard
      read-back), not the internal marker mechanics
- [x] No doc under `docs/` (outside `docs/specs/` history) claims
      non-file portal URIs are fatal

**Verification:**
- [x] Finish gate green (docs-only change; discipline holds):
      `cargo fmt --all --check` +
      `cargo clippy --workspace --all-targets -- -D warnings` +
      `cargo test --workspace --all-targets` +
      `cargo test --doc -p flintshot-core -p flintshot-editor-egui`

**Dependencies:** Task 001 (records what shipped)

**Files likely touched:**
- `CHANGELOG.md`

**Docs:**
- This task *is* docs.

**Estimated scope:** Small (1 file, docs-only)
