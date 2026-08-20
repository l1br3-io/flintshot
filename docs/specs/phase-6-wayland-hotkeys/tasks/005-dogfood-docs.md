# Task 005: Dogfood COSMIC + KDE, TEST_MATRIX, ADR-0029, CHANGELOG

**Description:** Ship-prep task: exercise the full spec §Exit
criteria on both target compositors (COSMIC, KDE Plasma — the
`/spec` dogfood answers), record the evidence, and write the docs.
ADR-0029 covers the portal-over-alternatives decision (context:
Wayland has no portable global-grab API; decision: GlobalShortcuts
via `ashpd`, portal-first on Wayland, suggestions-not-authority
config semantics, IPC-mediated bind; consequences: ≥ 1.16 portal
requirement, GNOME/wlroots fallback gap, app-id nuance for
windowless daemons). TEST_MATRIX Phase 6 rows get real values
(promoted from "Not yet implemented"); sway or a mocked-old-portal
run covers the fallback row. CHANGELOG `[Unreleased]` gets the
user-visible summary.

**Acceptance criteria:**
- [ ] COSMIC: fresh daemon start silent; `hotkeys bind` dialog with
      suggestions; bound key fires capture; restart re-attaches
      without dialog; re-bind replaces cleanly.
- [ ] KDE Plasma: same five flows verified.
- [ ] Fallback verified on sway (or portal-disabled session):
      classified notice, tray + IPC functional, clean Ctrl-C exit.
- [ ] X11 regression: hotkey + tray + reload flows re-run (TEST
      MATRIX X11 rows) — unchanged.
- [ ] `docs/TEST_MATRIX.md` Phase 6 section: rows filled with
      L1/L2/W1/W2 evidence + commit hashes.
- [ ] `docs/adrs/ADR-0029-ashpd-global-shortcuts.md` written
      (Context · Decision · Consequences format).
- [ ] `CHANGELOG.md` `[Unreleased]` updated (feature + new
      subcommand + `ashpd` dep note).
- [ ] Spec §Exit criteria 1–6 each checked against a concrete
      observation.

**Verification:**
- [ ] Standing gates: `cargo fmt --all --check`, `cargo clippy
      --workspace --all-targets -- -D warnings`, `cargo test
      --workspace --all-targets`, `cargo test --doc -p
      flintshot-core -p flintshot-editor-egui`
- [ ] TEST_MATRIX "How to verify a cell" procedure followed
      (commit hashes recorded)
- [ ] Human reviews the dogfood evidence before `/ship`

**Dependencies:** Tasks 002, 003, 004

**Files likely touched:**
- `docs/TEST_MATRIX.md`
- `docs/adrs/ADR-0029-ashpd-global-shortcuts.md` (new)
- `CHANGELOG.md`
- `docs/ROADMAP.md` (milestone flips at ship time — only in the
  merge/`/ship` step, not in this task)

**Docs:**
- This task *is* the docs task.

**Estimated scope:** Medium (3–4 doc files + manual test passes)
