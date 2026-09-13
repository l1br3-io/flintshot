# Task 010: ADR + docs sweep + CHANGELOG

**Description:** Record the effort. New ADR (next number:
ADR-0051): single-Capture UX — OS selection via portal interactive,
capability-based flow, after_grab routing, clean break of
region/fullscreen across CLI/IPC/config; supersedes the Spike B
"we have our own editor" note's UX stance and extends ADR-0050.
Docs: AGENTS.md (run examples → `capture`), ARCHITECTURE.md
(subcommand list, IPC schema, home-window + tray descriptions,
Wayland note), TEST_MATRIX.md (native-capture-ux section: OS UI
flow, cancellation, after_grab variants, fallback, per-desktop
interactive rows), spec.md consistency pass. CHANGELOG under
`## [Unreleased]`: **Changed** (breaking: `region`/`fullscreen` →
`capture`; `[hotkeys]` rename; IPC fold) + **Added** (portal
interactive selection, `after_grab`, home Open button).

**Acceptance criteria:**
- [ ] ADR-0051 written (Context · Decision · Consequences), indexed,
      cross-linked from ADR-0050
- [ ] AGENTS / ARCHITECTURE / TEST_MATRIX reference only `capture`
      (no stale `region`/`fullscreen` run examples)
- [ ] CHANGELOG breaking entries under **Changed**
- [ ] `spec.md` reconciled with shipped reality; frontmatter
      untouched (`status` flips at merge)
- [ ] `python3 .opencode/harness/scripts/check-refs.py` and
      `lint-frontmatter.py` green; `lint-plan.py` passes on the
      effort's plan

**Verification:**
- [ ] Full finish gate green (docs-only, but the discipline holds)
- [ ] Human reviews the ADR

**Dependencies:** Tasks 1-9 (records what shipped)

**Files likely touched:**
- `docs/adrs/ADR-0051-*.md` (new), `docs/adrs/README.md`,
  `docs/adrs/ADR-0050-*.md` (link)
- `docs/ARCHITECTURE.md`, `docs/TEST_MATRIX.md`, `AGENTS.md`,
  `CHANGELOG.md`
- `docs/specs/native-capture-ux/spec.md`

**Docs:**
- This task *is* docs.

**Estimated scope:** Medium (5-8 files, docs-only)
