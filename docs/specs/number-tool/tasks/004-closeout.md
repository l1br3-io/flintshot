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
- [x] `CHANGELOG.md` entry: Fixed — Number tool digit centered in its
      bubble (editor + saved output); Added — adjustable bubble
      diameter (16–96 px) via the panel's Number section.
- [x] `docs/TEST_MATRIX.md` row(s) covering the manual pass from
      Tasks 1/3 verification (centered digits, size stepper, WYSIWYG
      at extremes).
- [x] Spec exit criteria 1–5 each annotated met/how (PR description
      or this task's checkbox list) — see the walk below.
- [x] `just ci` green on the final tree.

**Verification:**
- [x] `just ci`
- [x] Exit-criteria walk recorded (criteria 1–5 from `spec.md`)

**Exit-criteria walk** (spec § Exit criteria, annotated):

1. **Centered digits, 1- and 2-digit, WYSIWYG** — met by
   construction + tests: the fix lives in the shared `render_ann`
   arm (`render_ann_number_emits_filled_ellipse_and_text`,
   `..._centers_two_digit_label`,
   `..._centers_at_scaled_diameter`); both canvases render committed
   annotations through it. *Visual confirmation on a live display:
   pending human dogfood (TEST_MATRIX rows added for it).*
2. **Stepper changes next placement only** — met:
   `dispatch_number_uses_number_diameter_for_new_placements` +
   `number_diameter_change_leaves_committed_bubbles_immutable`.
3. **Clamps 16–96, default 28** — met:
   `number_size_row_clamps_stale_diameter_on_render` (panel),
   `dispatch_number_default_diameter_is_28` (core).
4. **Undo decrements; stepper never history** — met:
   `dispatch_number_custom_diameter_undo_redo_restores_counter` +
   existing Task-004 suite green; the stepper mutates tool state
   only (no snapshot paths touch it).
5. **Standing gates green** — met: `just ci` on the final tree
   (fmt / clippy `-D warnings` / workspace tests / doc tests).

**Dependencies:** Tasks 1, 2, 3

**Files likely touched:**
- `CHANGELOG.md`
- `docs/TEST_MATRIX.md`
- `docs/specs/number-tool/` (status flip if needed)

**Docs:**
- This task *is* docs; no code.

**Estimated scope:** Small (2–3 files)
