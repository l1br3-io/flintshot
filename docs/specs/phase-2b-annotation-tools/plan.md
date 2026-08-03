# Plan — Phase 2b: Annotation Tools + Magnifier

> Thin index for the current effort. Requirements live in
> [`spec.md`](spec.md); flat status in [`todo.md`](todo.md); per-task
> acceptance / files / verification / dependencies in
> [`tasks/00N-*.md`](tasks/). For product context see
> [`../../VISION.md`](../../VISION.md); for sequence see
> [`../../ROADMAP.md`](../../ROADMAP.md); for the technical substrate
> see [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md) (§Data model,
> §UI abstraction layer, §Canvas trait, §What stays toolkit-specific).

## Overview

Close the annotation-tool gap deferred by Phase 1 and Phase 2. Five
tools — **Ellipse, Line, Marker, Pencil, Number** — plus the
**Magnifier**. The `Tool` / `Annotation` enums, the `Canvas` trait
surface, the core `render_ann()` dispatch, the editor state machine
(`begin_annotation` / `update_annotation` / `commit_annotation`), and
the `Annotation::contains` hit-test **already cover every variant**
(the codebase scaffolded further than the spec assumed). The remaining
work is surgical:

- the two `draw_ellipse` rasterizer backends are no-ops (egui
  `EguiCanvas::draw_ellipse` at `canvas.rs:208`, core
  `ImageCanvas::draw_ellipse` at `render.rs:743`) — the latter is a
  latent save/copy bug that drops ellipses from saved PNGs;
- the toolbar exposes only five of nine tools (`app.rs:557-561`);
- Marker stores full-opacity color (`editor.rs:355`) — no highlight
  semantics;
- freehand input pushes raw samples (`editor.rs:411`) — no smoothing;
- Number is unwired (`begin_annotation` returns early at
  `editor.rs:384`) and its render arm is a square-bubble placeholder
  (`render.rs:306-327`);
- the magnifier does not exist.

Phase 2b exit criteria (from [`spec.md`](spec.md)):

> A user captures a region, draws an outlined Ellipse (with optional
> fill), a straight Line, a translucent Marker highlight, a smoothed
> Pencil sketch, and places 1/2/3 Number bubbles — each with pending
> preview, clean commit into undo history, and identical rasterization
> to the eventual iced/slint shell (all drawing routed through
> `flintshot-core::render::render()`). Holding the magnifier modifier
> shows a zoomed pixel inset under the cursor. The abstraction holds:
> `MockCanvas` renders every variant and `no_gui_types_in_core.rs`
> stays green.

## Architecture decisions for Phase 2b

- **Each tool is a vertical slice; no new types.** Every deferred
  `Annotation` variant is already declared, dispatched in `render_ann`,
  and hit-tested in `contains`. Tasks complete the slice — rasterizer
  backend impl(s) + toolbar entry + the tool-specific gap (alpha /
  smoothing / click-to-place) — rather than designing new data models.
- **Ellipse first; it unblocks Number's bubble and fixes a latent
  bug.** `ImageCanvas::draw_ellipse` being a no-op means committed
  ellipses silently vanish from saved/copied PNGs today. A real
  `draw_ellipse` (outline + fill) also lets the Number tool render a
  true circular filled bubble (`draw_ellipse` with equal axes + `Some`
  fill), replacing the current square placeholder. (Locks ADR-0012
  abstraction; no new trait methods.)
- **Freehand smoothing is a pure fn in core.** Catmull-Rom with a
  configurable tension, applied on the pending + committed polyline so
  WYSIWYG holds. Lives in core so an iced/slint shell rasterizes
  identically. Marker and Pencil share the smoother (they share the
  polyline render + hit-test path already); they diverge only on
  stroke alpha. (ADR-0022.)
- **Marker translucency applied at `begin_annotation`, not at render.**
  The marker's `Annotation::Marker.color` is stored with reduced alpha
  at construction time; every backend (egui, ImageCanvas, MockCanvas)
  renders it translucently with no special-casing. Keeps the
  abstraction backend-agnostic.
- **Number is click-to-place with a snapshotted counter.** PointerDown
  with `Tool::Number` commits immediately (no drag) and increments
  `next_number`; `Snapshot` is extended to capture `next_number` so
  undo decrements it intuitively. Contrast text color picked by
  luminance threshold (white on dark fills, black on light).
- **Magnifier is toolkit-owned (egui only), reads `EditorState::image`
  directly.** Per ARCHITECTURE.md §What stays toolkit-specific, the
  magnifier does **not** get a core abstraction. New
  `crates/flintshot-editor-egui/src/magnifier.rs`; modifier-hold
  trigger (default configurable) with a config toggle for always-on.
  The pixel-sampling math is extracted into a pure helper so it is
  testable without a live egui window.

## Dependency graph

```text
            ┌─► 001 Ellipse (draw_ellipse impls) ──► 004 Number (circle bubble)
            │
core render ┼─► 002 Line (toolbar + degenerate guard)
state machine│
already done ├─► 003 Freehand smoother + Marker translucency
for all vars │
            └─► 005 Magnifier (pure egui, reads EditorState::image)

                       all five tools + magnifier
                                 │
                                 ▼
                            006 Hardening
              (boundary test, CHANGELOG, ADR-0022, TEST_MATRIX)
```

001 → 004 is the only hard dependency (Number's circular bubble reuses
the `draw_ellipse` impl). 002, 003, 005 are mutually independent and
independent of 001; they are ordered for coherent checkpoints, not for
correctness. 006 depends on all.

## Task index

Checkpoints gate phases — do not start the next sub-phase until the
previous checkpoint is signed off. Per-task acceptance criteria,
verification, dependencies, and files live in `tasks/00N-*.md`.

### Phase 2b.A — Foundation
- [`tasks/001-ellipse-tool.md`](tasks/001-ellipse-tool.md) — Ellipse end-to-end (egui + ImageCanvas `draw_ellipse` impls, toolbar)

**Checkpoint A** — ellipse renders in editor AND in saved/copied PNGs (latent `ImageCanvas` bug fixed); foundation for Number's bubble in place.

### Phase 2b.B — Shape + freehand tools
- [`tasks/002-line-tool.md`](tasks/002-line-tool.md) — Line toolbar entry + degenerate-pointer-up guard
- [`tasks/003-freehand-smoothing-marker.md`](tasks/003-freehand-smoothing-marker.md) — Shared freehand smoother (Catmull-Rom) + Marker translucency + toolbar entries

**Checkpoint B** — Ellipse, Line, Marker, Pencil all work end-to-end (4 of 5 deferred tools; spec exit steps 1–4).

### Phase 2b.C — Number
- [`tasks/004-number-tool.md`](tasks/004-number-tool.md) — Number click-to-place + circular bubble (reuses 001) + contrast text + undo counter

**Checkpoint C** — all five deferred tools work end-to-end (spec exit steps 1–5).

### Phase 2b.D — Magnifier
- [`tasks/005-magnifier.md`](tasks/005-magnifier.md) — Magnifier widget (egui-only, reads `EditorState::image`)

**Checkpoint D** — full editor exit criteria met including magnifier (spec exit step 6).

### Phase 2b.E — Hardening
- [`tasks/006-hardening.md`](tasks/006-hardening.md) — Boundary test, CHANGELOG, ADR-0022, TEST_MATRIX, doc cleanups

**Checkpoint E** — Phase 2b exit criteria met; ready for review.

## Risks and mitigations

| Risk                                                                 | Impact | Mitigation                                                                                                                  |
| -------------------------------------------------------------------- | ------ | --------------------------------------------------------------------------------------------------------------------------- |
| `ImageCanvas::draw_ellipse` no-op silently drops ellipses from saves | High   | Task 001 makes it the foundation + adds a `compose_final` pixel assertion so the regression can't return                    |
| Freehand smoothing destabilizes WYSIWYG (pending vs committed)       | Med    | Apply the smoother on both pending and committed paths in core; `MockCanvas` test asserts parity                            |
| Catmull-Rom overshoots on sharp direction changes (self-intersection) | Med   | Clamp tension / fall back to monotone interpolation if artifacts appear during Task 003; record choice in ADR-0022          |
| Marker alpha makes it hard to see on dark screenshots                | Low    | Pick alpha that keeps luminance visible (~0x66); slider already exists for stroke width; document the default                |
| Magnifier pixel sampling is slow per-frame                            | Low    | Sample a bounded region (zoom factor × inset size); cache the inset texture across frames while the modifier is held        |
| `Snapshot` extension (next_number) breaks existing serialization     | Low    | Add field with `#[serde(default)]`; it's an in-memory history struct (not persisted) — verify via existing undo/redo tests   |
| Scope creep from "polish" features (move/resize, shortcuts)          | Med    | Spec explicitly excludes them; tasks reject out-of-scope additions                                                          |

## Open questions (resolved to defaults — overridable until the relevant task starts)

1. **Marker vs Pencil: share or split?** *(spec Q1)* → **Share.** They
   already share `render_ann` polyline dispatch and `contains`
   hit-test; the smoother is shared too. Divergence (alpha) is
   isolated to one line in `begin_annotation`. (Task 003.)
2. **Number `next_number` undo policy.** *(spec Q2)* → **Snapshot it.**
   Extend `Snapshot` with `next_number: u32` so undo decrements
   intuitively. `Snapshot::anns_only` becomes a 2-field snapshot; all
   existing call sites updated. (Task 004.)
3. **Freehand smoothing algorithm.** *(spec Q3)* → **Catmull-Rom**,
   tension configurable (default 0.5), pure fn in core. Falls back to
   monotone interpolation if Task 003 finds overshoot artifacts.
   (Task 003; ADR-0022.)
4. **Magnifier trigger UX.** *(spec Q4)* → **Hold modifier** (default
   `Space`, configurable in `[ui]`), plus a `[ui] magnifier_always_on`
   toggle (default false). Matches Flameshot without forcing it.
   (Task 005.)
5. **Line/Arrow hit-test tolerance.** *(spec Q5)* → **Already
   implemented.** `contains` uses `stroke.width / 2 +
   HIT_TEST_TOLERANCE_PX` (3.0 px) at `editor.rs:1188`; Line reuses it
   unchanged. No work.
6. **Ellipse fill UI.** *(spec Q6)* → **Mirror Rect.** Expose an
   optional fill, identical UX to the existing Rectangle fill path.
   (Task 001.)
7. **Number bubble shape (plan-level refinement of spec).** The spec's
   deliverable bullet mentioned `draw_rect_filled` for the bubble, but
   its Objective says "filled circular bubble." → **Use
   `draw_ellipse` with equal axes + `Some(fill)`** (reuses Task 001's
   impl) for a true circle. The Objective governs. (Task 004.)

## Verification strategy

Every task clears the standing gate in [`AGENTS.md`](../../../AGENTS.md)
(`fmt --all --check`, `clippy --workspace --all-targets -- -D warnings`,
`test --workspace --all-targets`, doc tests for the two lib crates).
Tool behaviour needing a display is covered headless through the
`MockCanvas` render tests (toolkit-independent, no GPU) and editor
state-machine unit tests; `#[ignore]`'d integration tests cover the
visual path under `xvfb`, mirroring Phase 1's capture smoke test. Each
checkpoint adds a manual run on at least one platform from
[`docs/TEST_MATRIX.md`](../../TEST_MATRIX.md). Per-task verification
steps live in `tasks/00N-*.md`.

## Decisions to land this effort

Lasting technical decisions (full text in [`docs/adrs/`](../../adrs/)):

- **ADR-0022** — shared Marker/Pencil freehand state machine +
  Catmull-Rom smoothing in core (Task 003).

The magnifier's toolkit-ownership is already documented in
ARCHITECTURE.md §What stays toolkit-specific (no new ADR). The
Number/Snapshot-counter and ellipse-reuse-for-bubble choices are
plan-level refinements recorded above, not standalone ADRs.
