# Todo — Phase 2b: Annotation Tools + Magnifier

Flat status checklist. One line per task, mirroring
[`plan.md`](plan.md)'s task index. Per-task detail in
[`tasks/00N-*.md`](tasks/). This is the single status source the
project resumes from.

## Phase 2b.A — Foundation
- [x] Task 001 — Ellipse end-to-end (egui + ImageCanvas `draw_ellipse` impls, toolbar) → [`tasks/001-ellipse-tool.md`](tasks/001-ellipse-tool.md)

**Checkpoint A** — ellipse renders in editor AND in saved/copied PNGs; latent `ImageCanvas` bug fixed.

## Phase 2b.B — Shape + freehand tools
- [x] Task 002 — Line toolbar entry + degenerate-pointer-up guard → [`tasks/002-line-tool.md`](tasks/002-line-tool.md)
- [x] Task 003 — Shared freehand smoother (Catmull-Rom) + Marker translucency + toolbar entries → [`tasks/003-freehand-smoothing-marker.md`](tasks/003-freehand-smoothing-marker.md)

**Checkpoint B** — Ellipse, Line, Marker, Pencil work end-to-end (4 of 5 tools).

## Phase 2b.C — Number
- [x] Task 004 — Number click-to-place + circular bubble + contrast text + undo counter → [`tasks/004-number-tool.md`](tasks/004-number-tool.md)

**Checkpoint C** — all five deferred tools work end-to-end.

## Phase 2b.D — Magnifier
- [ ] Task 005 — Magnifier widget (egui-only, reads `EditorState::image`) → [`tasks/005-magnifier.md`](tasks/005-magnifier.md)

**Checkpoint D** — full editor exit criteria met including magnifier.

## Phase 2b.E — Hardening
- [ ] Task 006 — Boundary test, CHANGELOG, ADR-0022, TEST_MATRIX, doc cleanups → [`tasks/006-hardening.md`](tasks/006-hardening.md)

**Checkpoint E** — Phase 2b exit criteria met; ready for review.
