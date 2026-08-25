# Task 004: Docs — CHANGELOG, TEST_MATRIX, final gates

**Description:** Closeout: user-visible feature notes in the
CHANGELOG, manual QA rows in TEST_MATRIX, and the full standing gate
run. Also sweeps the spec's exit criteria (1–6) marking each
verified or manual-pending.

**Acceptance criteria:**
- [ ] `CHANGELOG.md` `## [Unreleased]`: launcher entry + desktop
      actions + autostart + install recipes, phrased for users.
- [ ] `docs/TEST_MATRIX.md`: rows for launch-from-menu (GNOME/KDE/
      COSMIC × X11/Wayland), desktop actions, autostart login,
      uninstall, idempotent reinstall.
- [ ] Spec exit-criteria sweep recorded (in the effort's todo or a
      short checklist note in the PR description — not new docs).
- [ ] Full gate run green:
      `cargo fmt --all --check`;
      `cargo clippy --workspace --all-targets -- -D warnings`;
      `cargo test --workspace --all-targets`;
      `cargo test --doc -p flintshot-core -p flintshot-editor-egui`.
- [ ] No ADR needed — decisions are minor and recorded in the spec's
      Decisions section (double-check none grew architectural
      during implementation; if one did, write it).

**Verification:**
- [ ] Gate commands above all pass
- [ ] `python3 .opencode/harness/scripts/lint-frontmatter.py`

**Dependencies:** Tasks 001–003

**Files likely touched:**
- `CHANGELOG.md`
- `docs/TEST_MATRIX.md`

**Docs:** This task is the docs.

**Estimated scope:** Small (2 files)
