# Task 002: Doc + CHANGELOG sweep

**Description:** Update every doc that describes `after_grab =
clipboard` as a window-less flow. (1) `docs/TEST_MATRIX.md` — step 3
of the native-capture-ux acceptance list (~line 2324: "copies the
grab with no windows") and the matrix row (~line 2353: "`--after
clipboard` / `upload`: windowless flow") — clipboard now copies +
notifies + opens the editor/overlay; upload stays window-less, so
split the row if cleaner. (2) `CHANGELOG.md` `[Unreleased]` — amend
the `after_grab` **Added** entry (lines 56-63) in place (never
shipped; see plan decision) to describe copy-then-editor, keeping
the fallback-ignores-it sentence. (3) `docs/adrs/ADR-0051` — dated
addendum note on the consequence line (~line 90) pointing to
`docs/specs/clipboard-opens-editor/`. (4) `docs/specs/clipboard-opens-editor/spec.md`
— reconcile only if implementation reality diverged from the spec.

**Acceptance criteria:**
- [x] No doc under `docs/` (excluding `docs/specs/` history) claims
      the clipboard flow opens no windows — verified via
      `rg -n "window-less|windowless" docs/ CHANGELOG.md AGENTS.md`
      hitting only upload-appropriate or historical-spec contexts
- [x] CHANGELOG `after_grab` entry describes copy + notification +
      editor open; no separate contradicting **Changed** bullet
- [x] ADR-0051 carries the addendum; no other ADR edited
- [x] `AGENTS.md` needs no change (its run examples don't describe
      the clipboard flow) — verify, don't assume

**Verification:**
- [x] `python3 .opencode/harness/scripts/check-refs.py`
      (one pre-existing unrelated broken ref:
      `docs/specs/launcher-desktop-entry/spec.md` →
      `../snipping-modes/spec.md`; not introduced here, left for
      its owning effort)
- [x] `python3 .opencode/harness/scripts/lint-frontmatter.py`
- [x] `python3 .opencode/harness/scripts/lint-plan.py`
- [x] Full finish gate green (docs-only, but the discipline holds):
      `cargo fmt --all --check` +
      `cargo clippy --workspace --all-targets -- -D warnings` +
      `cargo test --workspace --all-targets` +
      `cargo test --doc -p flintshot-core -p flintshot-editor-egui`

**Dependencies:** Task 001 (records what shipped)

**Files likely touched:**
- `docs/TEST_MATRIX.md`
- `CHANGELOG.md`
- `docs/adrs/ADR-0051-single-capture-action-os-selection.md`
- `docs/specs/clipboard-opens-editor/spec.md` (only if diverged)

**Docs:**
- This task *is* docs.

**Estimated scope:** Small (3-4 files, docs-only)
