# Task 008: Living docs, justfile, ADR-0049, CHANGELOG

**Description:** Rewrite the living docs now that the code truth is
final: README feature list + Wayland note, VISION pin bullets,
ARCHITECTURE pin sections (diagram, IPC enum samples, trait listing,
`[pin]` config sample), ROADMAP Phase 3 annotation, TEST_MATRIX pin
rows, the `just pin` recipe. Write ADR-0049 (removal decision) and
the CHANGELOG `[Unreleased]` entry. Historical records (CHANGELOG
history, `docs/specs/phase-3-*`, existing ADRs) stay untouched.

**Acceptance criteria:**
- [ ] README: Wayland always-on-top callout + both feature bullets
      ("pin to desktop", "Pin-to-desktop (`flintshot pin` …)") removed
- [ ] VISION: "Pin as floating always-on-top window" bullet and
      `+ pin` in the v1-ships line dropped
- [ ] ARCHITECTURE: pin window in the component diagram, IPC enum
      sample lines, `run_pin` trait method listing, `[pin]` config
      block, and pin mentions in prose removed
- [ ] ROADMAP Phase 3 row annotated "(pin later removed, ADR-0049)" —
      row kept as history
- [ ] TEST_MATRIX: `[pin]` config rows, `flintshot pin` CLI row,
      `pin` in the help-list expectation, Phase 3 pin sections
      removed/reworked; COSMIC applet coverage kept
- [ ] justfile `pin` recipe deleted
- [ ] `docs/adrs/ADR-0049-remove-pin-to-desktop.md` written
      (Context · Decision · Consequences, matching existing ADR
      format; indexed in `docs/adrs/README.md` if that's the pattern)
- [ ] CHANGELOG `[Unreleased]`: removal entry incl. the wire-format
      break note and `[pin]`-section-now-inert note
- [ ] No edits to CHANGELOG history entries or phase-3 spec dirs

**Verification:**
- [ ] `python3 .opencode/harness/scripts/check-refs.py` (if it covers
      docs) or manual link check of edited files
- [ ] `rg -in "pin" README.md docs/VISION.md docs/ARCHITECTURE.md docs/TEST_MATRIX.md justfile` returns only false positives
- [ ] ADR number/format matches `docs/adrs/ADR-0048-*`

**Dependencies:** Task 007 (docs describe final code truth)

**Files likely touched:**
- `README.md`, `docs/VISION.md`, `docs/ARCHITECTURE.md`,
  `docs/ROADMAP.md`, `docs/TEST_MATRIX.md`, `justfile`,
  `CHANGELOG.md`, `docs/adrs/ADR-0049-remove-pin-to-desktop.md` (new),
  `docs/adrs/README.md`

**Docs:**
- This task *is* docs.

**Estimated scope:** Medium (8-9 files, all mechanical text edits)
