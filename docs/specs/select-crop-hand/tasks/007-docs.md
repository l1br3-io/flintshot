# Task 007: Docs — CHANGELOG, ADR, CLI help

**Description:** Record the tool-semantics remap. CHANGELOG under
`## [Unreleased]`; ADR-0063 covering the remap rationale (including the
mainstream pan-only Hand decision over the earlier Shift/Ctrl select-and-move
draft) and the config migration table; CLI help text mentioning the region
tool updated.

**Acceptance criteria:**
- [ ] `CHANGELOG.md` entry under `## [Unreleased]`: Select/Crop/Hand remap,
      marquee multi-select, pan + wheel zoom, `flip-to-select` migration
- [ ] `docs/adrs/ADR-0063-select-crop-hand-tool-remap.md` in
      Context · Decision · Consequences form, including the migration table:
      old `flip-to-move` → `flip-to-select` (same value), old `S` hotkey →
      freed, `V`/`H`/`C` mapping, Hand pan-only (mainstream) with the
      dropped-modifier-draft note, overlay/selection-first pre-arm Crop
- [ ] `crates/flintshot-app/src/cli.rs` help/docs referencing the region
      tool name updated
- [ ] Spec frontmatter ready to flip to `active`→`shipped` flow at merge

**Verification:**
- [ ] `cargo test --workspace --all-targets` green (cli doc tests if any)
- [ ] Full AGENTS.md finish sequence green (final gate for the effort)
- [ ] Spec §Success Criteria checklist walked end-to-end

**Dependencies:** Tasks 001-006 (last)

**Files likely touched:**
- `CHANGELOG.md`
- `docs/adrs/ADR-0063-select-crop-hand-tool-remap.md`
- `crates/flintshot-app/src/cli.rs`

**Docs:** This task *is* the docs.

**Estimated scope:** Small (3 files)
