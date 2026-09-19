# Task 004: Closeout — CHANGELOG + TEST_MATRIX + exit-criteria walk

**Description:** Record and close. Add a `CHANGELOG.md` entry under
the development version (Keep-a-Changelog categories: **Fixed** for
the centered digit, **Added** for the size control) and a
`docs/TEST_MATRIX.md` row for the Number tool manual pass. Walk the
spec's five exit criteria end-to-end and annotate each met/how in the
PR description (the app-redesign final-gate pattern). Spec frontmatter
flips to `status: active` here if not already (it flips at `/build`
start); `status: shipped` happens at merge, not in this task.

**Acceptance criteria:**
- [ ] `CHANGELOG.md` entry: Fixed — Number tool digit centered in its
      bubble (editor + saved output); Added — adjustable bubble
      diameter (16–96 px) via the panel's Number section.
- [ ] `docs/TEST_MATRIX.md` row(s) covering the manual pass from
      Tasks 1/3 verification (centered digits, size stepper, WYSIWYG
      at extremes).
- [ ] Spec exit criteria 1–5 each annotated met/how (PR description
      or this task's checkbox list).
- [ ] `just ci` green on the final tree.

**Verification:**
- [ ] `just ci`
- [ ] Exit-criteria walk recorded (criteria 1–5 from `spec.md`)

**Dependencies:** Tasks 1, 2, 3

**Files likely touched:**
- `CHANGELOG.md`
- `docs/TEST_MATRIX.md`
- `docs/specs/number-tool/` (status flip if needed)

**Docs:**
- This task *is* docs; no code.

**Estimated scope:** Small (2–3 files)
