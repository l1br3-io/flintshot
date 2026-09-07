# Task 004: Docs closeout — ADR-0043, CHANGELOG, TEST_MATRIX

**Description:** Record the decision and ship the docs. ADR-0043
(`selection-phase-tool-lock`): context — region-selection-first
(ADR-0035) left the rail live during the phase and the overlay's
keyboard ungated; decision — disabled-not-hidden chips with a
`ToolGate` enable-set, phase-scoped input denial, per-shell unlock
points (editor: crop, overlay: usable selection), hardcoded;
consequences — mid-phase tool switching is impossible by design,
no config escape hatch until asked. CHANGELOG under
`[Unreleased]`; TEST_MATRIX rows for both shells; final standing
gate run.

**Acceptance criteria:**
- [ ] `docs/adrs/ADR-0043-selection-phase-tool-lock.md` follows the
      Context · Decision · Consequences format, references
      ADR-0035, and records the disable-vs-hide rationale and the
      planning correction (overlay rail already hides
      pre-selection).
- [ ] `CHANGELOG.md` `[Unreleased]` entry describes the user-
      visible behaviour (locked rail during region selection;
      unlock on crop / selection).
- [ ] `docs/TEST_MATRIX.md` rows: editor phase (click / hotkey /
      draw inert pre-crop, live post-crop), overlay pre-selection
      tool keys, `fullscreen` / `open` unregressed.
- [ ] Standing gate green:
      `cargo fmt --all --check` ·
      `cargo clippy --workspace --all-targets -- -D warnings` ·
      `cargo test --workspace --all-targets` ·
      `cargo test --doc -p flintshot-core -p flintshot-editor-egui`.
- [ ] Manual pass against spec exit criteria 1–5 recorded in
      `todo.md` (both shells; then the effort is ready for
      /review).

**Verification:**
- [ ] The four standing-gate commands above, quiet where possible;
      exit codes are the verdict.
- [ ] `python3 .opencode/harness/scripts/check-refs.py` (if wired
      to CI) or eyeball every relative link in the new ADR.

**Dependencies:** Tasks 001–003 (documents what they built).

**Files likely touched:**
- `docs/adrs/ADR-0043-selection-phase-tool-lock.md` (new)
- `CHANGELOG.md`
- `docs/TEST_MATRIX.md`
- `docs/specs/select-first-tool-lock/todo.md`

**Docs:**
- This task *is* the docs.

**Estimated scope:** Small (docs only)
