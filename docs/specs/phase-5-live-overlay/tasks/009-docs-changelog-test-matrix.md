# Task 009: Docs — ADR-0028, CHANGELOG, TEST_MATRIX

**Description:** Write the documentation artifacts the spec's
Deliverables section requires, and close the effort out:
- **ADR-0028** formalizing the capture-then-overlay decision (and the
  Wayland editor-window fallback that follows from it)
- **CHANGELOG** `[Unreleased]` ship note for Phase 5 (the per-task
  feature entries for Tasks 001–008 landed with their code)
- `docs/TEST_MATRIX.md`: replace the forward-looking Phase 5
  placeholder with the real test setup / script / matrix rows
  (per-OS overlay flows + Wayland fallback)
- Flip `spec.md` frontmatter to `status: shipped` with started /
  shipped dates (started = scaffold commit date, matching the
  Phase 2b/3/4 convention)

**ADR to create:**
- **ADR-0028: Capture-then-overlay** — frozen pixels over live
  transparency / layer-shell; overlay as a new shell over the
  unchanged dispatcher; Wayland honest fallback (spec Decisions §1,
  Architecture decisions 1/2/5).

**TEST_MATRIX rows for:** mode resolution (flag / config / unknown
value), overlay open + Esc cancel, selection chrome (mask, handles,
move, badge, magnifier), floating toolbar, all ten tools clipped to
the region, Enter=Copy / Save / Pin / Upload outputs, X11 clipboard
linger, Wayland fallback + `--overlay` best-effort, idle-repaint CPU,
editor-window non-regression.

**Acceptance criteria:**
- [x] ADR-0028 created in `docs/adrs/` (Context · Decision ·
      Consequences format)
- [x] CHANGELOG `[Unreleased]` Notes records the Phase 5 ship
- [x] `docs/TEST_MATRIX.md` Phase 5 section rewritten as a testable
      script (no "not yet implemented" banner left behind)
- [x] `spec.md` frontmatter `status: shipped`, `started` / `shipped`
      dates present
- [x] Standing gates stay green (docs-only change)

**Verification:**
- [x] `cargo fmt --all --check`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo test --workspace --all-targets`
- [x] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`

**Dependencies:** Tasks 001–008 (documents the completed work)

**Files likely touched:**
- `docs/adrs/ADR-0028-capture-then-overlay.md` (NEW)
- `docs/specs/phase-5-live-overlay/tasks/009-docs-changelog-test-matrix.md` (NEW)
- `docs/TEST_MATRIX.md`
- `CHANGELOG.md`
- `docs/specs/phase-5-live-overlay/spec.md`
- `docs/specs/phase-5-live-overlay/todo.md`

**Estimated scope:** Small: 6 files (documentation only)
