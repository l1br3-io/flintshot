# Task 009: Docs — ADR-0048, CHANGELOG, TEST_MATRIX

**Description:** Record the decisions and close the effort's paper
trail.

- `docs/adrs/ADR-0048-capture-session-replace-and-prepare.md` —
  **Context · Decision · Consequences** (house format, see
  `docs/adrs/README.md`):
  - Context: self-capture of non-initiating surfaces; duplicate
    editors on overlapping triggers; EditorOpened was a spawn
    proxy.
  - Decision: Replace policy via daemon session registry
    (graceful-close-then-kill); `PrepareCapture`/`CaptureReady`
    handshake clearing tray + app window + pins; worker-reported
    `EditorOpened`.
  - Consequences: one annotation surface at a time (unsaved
    annotations in a superseded session are lost by design);
    editor-hosted pins die with their host (durable pins =
    follow-up); pin registry is daemon-lifetime (orphans after
    daemon restart); all waits bounded; wire variants appended.
- `CHANGELOG.md` under `## [Unreleased]` → **Fixed**: self-capture
  of editor/overlay/app-window/pins; duplicate editor windows on
  overlapping triggers. **Changed**: `EditorOpened` now fires on
  real window open.
- `docs/TEST_MATRIX.md`: rows for the Checkpoint A/B manual
  scenarios (tray-while-editor, hotkey-while-app-window, pins
  across captures, kill -9 straggler, standalone unchanged) with
  results per compositor tried.
- Spec closeout: frontmatter stays `active` until merge; on merge,
  flip to `shipped: YYYY-MM-DD` (per the effort lifecycle — the
  merge commit does it, not this task).

**Acceptance criteria:**
- [ ] ADR-0048 follows the three-section format and records the
      four decisions + consequences above
- [ ] CHANGELOG entries under `[Unreleased]`, matching repo voice
- [ ] TEST_MATRIX rows added with actual manual results (not
      placeholders)
- [ ] `check-refs.py` / `lint-frontmatter.py` (if they cover
      these paths) pass

**Verification:**
- [ ] `python3 .opencode/harness/scripts/check-refs.py` (if
      applicable to docs)
- [ ] Full standing gates one final time:
      `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --all-targets && cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Spec exit criteria 1–8 walked and checked off

**Dependencies:** 001–008

**Files likely touched:**
- `docs/adrs/ADR-0048-capture-session-replace-and-prepare.md` (new)
- `CHANGELOG.md`
- `docs/TEST_MATRIX.md`

**Docs:** This task *is* docs.

**Estimated scope:** Small: 3 files
