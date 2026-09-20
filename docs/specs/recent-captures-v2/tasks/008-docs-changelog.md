# Task 008: Docs + CHANGELOG + full gate run

**Description:** Close the loop. Fix `docs/ARCHITECTURE.md` §Config —
the example schema is stale snake_case and missing `[output]`; rewrite
it to mirror the real kebab-case struct including the new fields. Add
the CHANGELOG `[Unreleased]` entry (Added: standard save location +
picker, richer cards with hover actions, clipboard-copy persistence,
configurable capacity; Fixed: the `[output]`/`[ui]` re-save merge gap).
Run all four finish gates and the manual verification checklist for
acceptance criteria 1–4.

**Acceptance criteria:**
- [ ] ARCHITECTURE.md config block matches the shipped struct field-for-field (kebab-case, `[output]` present)
- [ ] CHANGELOG `[Unreleased]` entry covers all four axes + the merge fix
- [ ] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --all-targets`, `cargo test --doc -p flintshot-core -p flintshot-editor-egui` all pass
- [ ] Manual checklist recorded in the commit message or summary: default-config save lands in `~/Pictures/Flintshot`; Settings picker redirect survives restart; editor Copy lands in recents; card copy/reveal/delete behave (criteria 1–4)

**Verification:**
- [ ] The four gates, verbatim
- [ ] Manual pass flagged for the user where headless verification is impossible

**Dependencies:** Tasks 001–007

**Files likely touched:**
- `docs/ARCHITECTURE.md`
- `CHANGELOG.md`

**Docs:**
- Schema fix IS the task

**Estimated scope:** Small
