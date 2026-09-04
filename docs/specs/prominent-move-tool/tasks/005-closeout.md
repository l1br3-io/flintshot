# Task 005: Closeout — ADR-0040, CHANGELOG, TEST_MATRIX, manual pass

**Description:** Record the decisions, document the user-visible
changes, and run the full manual pass against the spec's exit
criteria. The ADR captures the non-obvious choice: a config-gated
flip-to-Move commit policy with an ownership guard, and
per-session config reads over cross-process live reload.

**Acceptance criteria:**

- [x] `docs/adrs/ADR-0040-flip-to-move-after-commit.md` written
      (Context · Decision · Consequences): flip policy + ownership
      guard, `flip-to-move` default-on gate, per-session config
      read rationale; `docs/adrs/README.md` index line added
- [x] `CHANGELOG.md` under `## [Unreleased]`: flip-to-Move (with
      config knob), Move default edit-phase tool, labeled rail
      chip, hover affordances
- [x] `docs/TEST_MATRIX.md` rows for the new behaviors (flip on /
      off, phase-end tool, hover chrome, labeled chip)
- [x] Manual pass through spec exit criteria 1–6 recorded (both
      shells; `cargo run -- region` and a plain editor capture)
- [x] Standing gates green:
      `cargo fmt --all --check`;
      `cargo clippy --workspace --all-targets -- -D warnings`;
      `cargo test --workspace --all-targets`;
      `cargo test --doc -p flintshot-core -p flintshot-editor-egui`

**Verification:**

- [x] All four gate commands exit 0
- [x] Spec exit-criteria checklist walked end-to-end with the
      running binary (scripted on a synthetic Xvfb display, TEST
      MATRIX platform XV1: editor shell + overlay shell, pixel-
      level assertions; details + caveats in the TEST_MATRIX
      section)

**Dependencies:** Tasks 001–004

**Files likely touched:**

- `docs/adrs/ADR-0040-flip-to-move-after-commit.md` (new)
- `docs/adrs/README.md`
- `CHANGELOG.md`
- `docs/TEST_MATRIX.md`

**Docs:**

- This task *is* the docs task.

**Estimated scope:** Small: 4 doc files
