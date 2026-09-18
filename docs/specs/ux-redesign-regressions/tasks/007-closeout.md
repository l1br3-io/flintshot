# Task 007: Closeout — CHANGELOG, ADR, full gate + manual protocol

**Description:** Effort closeout: record the two non-obvious technical
findings durably, update the CHANGELOG, and run the full gate plus the
end-to-end manual protocol against every spec success criterion. ADR
candidate: "Painted vector glyphs over font-fallback icons" (the cmap
finding — Unicode symbol chars are absent from the embedded chain;
icons must never depend on font fallback) and the worker copy-lifetime
policy (one ADR covering both closeout decisions is fine, or two short
ones — match `docs/adrs/` granularity precedent, next free number).

**Acceptance criteria:**
- [ ] `CHANGELOG.md` updated under `## [Unreleased]` covering: home
      geometry, painted icons + tofu ban, Copy feedback, Settings
      capture dispatch, worker copy lifetime
- [ ] ADR(s) in `docs/adrs/` (Context · Decision · Consequences)
      recording the painted-glyph rule and the copy-lifetime policy
      with the probe evidence
- [ ] Full gate green: `cargo fmt --all --check`, `cargo clippy
      --workspace --all-targets -- -D warnings`, `cargo test
      --workspace --all-targets`, `cargo test --doc -p flintshot-core
      -p flintshot-editor-egui`
- [ ] Manual protocol (spec §Commands) end-to-end: Home 1024×640
      resizable; all icons render; Copy shows "Copied" and
      `wl-paste --list-types` lists `image/png` on the editor page;
      worker Copy lingers and pastes; Settings Capture dispatches
- [ ] Spec success criteria 1-6 each checked against the running app

**Verification:**
- [ ] The four gate commands, exit codes zero
- [ ] Manual protocol checklist captured in the PR description

**Dependencies:** Tasks 1-6

**Files likely touched:**
- `CHANGELOG.md`
- `docs/adrs/ADR-NNNN-*.md`

**Docs:**
- This task IS the docs task.

**Estimated scope:** Small: 2 files

**Git Commit:** `git commit -m "docs: regression closeout — changelog + glyph/copy-lifetime ADRs"`
