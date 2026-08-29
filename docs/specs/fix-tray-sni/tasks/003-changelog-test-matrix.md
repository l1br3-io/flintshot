# Task 003: CHANGELOG + TEST_MATRIX updates

**Description:** Record the fix in user-facing and test-facing
docs, mirroring how `fix-delegated-capture` Task 3 did it. CHANGELOG
`## [Unreleased]` gains an entry: tray icon now registers with
StatusNotifierWatcher hosts (COSMIC included) and tray menu clicks
dispatch — because the daemon's platform loop never iterated the
GLib main context libappindicator/muda drive from. TEST_MATRIX
finding 3 gets a **Fixed** annotation with the fix commit hash and
the honest "manual L5 pass pending" note; the tray rows that
reference the bug (row 4 area / "Tray icon appears after
activation", and the no-SNI-host row stays untouched — expected
behavior unchanged) get the same annotation once L5 confirms.

**Acceptance criteria:**

- [x] CHANGELOG entry under `## [Unreleased]` describes symptom,
      cause (no GLib iteration → registration/menu events never
      dispatch), fix, and the COSMIC caveat history
- [x] TEST_MATRIX finding 3 annotated **Fixed in `<sha>`** with
      regression-seam note (pump unit tests) and "manual L5 pass
      pending" honesty
- [x] No predicted-pass entries anywhere: rows are annotated as
      pending until the maintainer's L5 run records results

**Verification:**

- [x] Claims consistent with landed code (Tasks 1–2)
- [x] `python3 .opencode/harness/scripts/lint-frontmatter.py` clean
      (if it covers TEST_MATRIX frontmatter — check before relying
      on it)

**Dependencies:** Task 2 (records the landed fix)

**Files likely touched:**

- `CHANGELOG.md`
- `docs/TEST_MATRIX.md`

**Docs:**

- This task *is* docs.

**Estimated scope:** Small: 2 files
