# Task 012: Fidelity sweep + docs

**Description:** Whole-effort closeout: side-by-side fidelity check
against `reference/*.html`, spec success-criteria audit (1-11), docs.
Confirms every criterion with a concrete check, updates CHANGELOG,
records the five-zone rework + settings route as ADRs.

**Acceptance criteria:**
- [ ] Spec success criteria 1-11 each verified (checklist in PR
      description); token scan green.
- [ ] Side-by-side: `reference/zone-map.html` + `settings.html` beside
      `cargo run -- app` — zones, colors, states read as the same
      design (arrangement fidelity, not pixel identity).
- [ ] CHANGELOG entry under `## [Unreleased]` (re-layout + settings).
- [ ] ADR(s): five-zone layout rework (header retirement, action
      redistribution, no-fake-controls) and settings config schema
      (additive fields, live-apply) — Context · Decision · Consequences.
- [ ] Full gate: fmt, clippy `-D warnings`, workspace tests, doc
      tests — all green.

**Verification:**
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`

**Dependencies:** Tasks 1-11

**Files likely touched:**
- `CHANGELOG.md`
- `docs/adrs/ADR-00NN-*.md` (new, 1-2 files)

**Estimated scope:** Small (2-3 files + verification)
