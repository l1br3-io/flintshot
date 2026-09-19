# Task 005: Polish — stale module docs, CHANGELOG, full gate

**Description:** Close out. The `panel.rs` module doc still
describes a "bottom cluster: history + export chips + Copy hero"
that `show()` no longer renders (Copy hero → topbar, history chips
→ floating cluster) — rewrite the doc to the real layout,
including the new tab header + timeline. Add the `CHANGELOG.md`
entry under `## [Unreleased]`. Run the full verification gate.

**Acceptance criteria:**
- [ ] `panel.rs` module doc matches what `show()` actually renders
      (action row, tab header, sections; no bottom-cluster mention)
- [ ] `CHANGELOG.md` has a user-visible entry under
      `## [Unreleased]`
- [ ] Full gate green: `cargo fmt --all --check`;
      `cargo clippy --workspace --all-targets -- -D warnings`;
      `cargo test --workspace --all-targets`;
      `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Spec success criteria 1–5 spot-checked (criteria 3 and 4 by
      manual run)

**Verification:**
- [ ] All four gate commands pass
- [ ] Manual: `cargo run -- app` end-to-end pass through both tabs

**Dependencies:** Tasks 001–004

**Files likely touched:**
- `crates/flintshot-editor-egui/src/panel.rs` (module doc only)
- `CHANGELOG.md`

**Docs:**
- CHANGELOG per `AGENTS.md`; no ADR needed (no non-obvious
  technical choice beyond what the spec records).

**Estimated scope:** Small: 2 files
