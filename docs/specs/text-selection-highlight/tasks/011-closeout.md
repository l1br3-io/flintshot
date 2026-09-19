# Task 011: Closeout — CHANGELOG, TEST_MATRIX, spec sync, gates

**Description:** Ship hygiene: CHANGELOG entry under `## [Unreleased]` (user-visible: text selection + highlight gestures, bar caret, forward Delete); `docs/TEST_MATRIX.md` rows for the new gesture surface (draft + re-edit, both shells); spec sync check (structure table already corrected at planning — verify no further drift, flip frontmatter `status: draft → active` if `/build` hasn't already); ADR decision: whether "shell resolves hits, core owns selection" + tracker-count extension merits an ADR (bar is low — it extends the documented shell-owns-glyph-metrics precedent; add one only if the reviewer wants the pump-interception contract recorded beyond code docs). Full four-gate run + the complete spec success-criteria walk (1–8).

**Acceptance criteria:**
- [x] CHANGELOG entry merged under Unreleased
- [x] TEST_MATRIX rows for drag/dblclick/triple/keyboard selection, replace/delete-over-selection, bar caret, both shells
- [x] Spec success criteria 1–8 each verified (tests or manual note in the PR description)
- [x] Gates: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --all-targets`, `cargo test --doc -p flintshot-core -p flintshot-editor-egui` — all clean
- [x] ADR skipped with a one-line reason: the pump-interception contract extends the documented shell-owns-glyph-metrics precedent (ADR-0062 era) and is recorded in `texthit.rs` module docs + pump inline comments; no new architectural decision was made

**Verification:**
- [x] The four gate commands, run in order from AGENTS.md
- [x] Human review checkpoint (Checkpoint C) before merge

**Dependencies:** Tasks 001–010

**Files likely touched:**
- `CHANGELOG.md`
- `docs/TEST_MATRIX.md`
- `docs/specs/text-selection-highlight/spec.md` (frontmatter status)
- `docs/adrs/ADR-NNNN-*.md` (conditional)

**Docs:**
- This task IS docs.

**Estimated scope:** Small: 2–4 files
