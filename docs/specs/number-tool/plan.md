# Plan: Number tool — centered digits + a panel size control

## Overview

Two user-visible fixes to `Tool::Number`: center the digit inside the
bubble (shared `render_ann` path, so editor view and saved output fix
together), and make the bubble diameter adjustable (16–96 px DragValue
stepper in the panel's Number section, feeding a new
`EditorState::number_diameter`; new placements only). Spec:
[`spec.md`](spec.md).

## Architecture Decisions

- **Centering at the shared call site, not in the canvases.** The bug
  is a contract mismatch: `render_ann` passes the bubble center to
  `draw_text`, whose trait contract anchors top-left. Fix in the
  `Annotation::Number` arm via `canvas.measure_text` (existing trait
  method, same pattern as `draw_text_indicators`); both `EguiCanvas`
  and `ImageCanvas` inherit the fix through the shared path, keeping
  the WYSIWYG contract intact. Changing canvas anchor semantics would
  ripple into the Text tool, whose `pos` genuinely is top-left.
- **Diameter is per-annotation state, set at commit.** Annotations
  already store `diameter`; the panel edits session-level
  `EditorState::number_diameter`, and `place_number` stamps it into
  the committed annotation. Committed bubbles never change; no
  history entries (matches color/stroke tool-state controls).
- **Range constants are `pub` in core.** `NUMBER_MIN_DIAMETER` /
  `NUMBER_MAX_DIAMETER` live beside `NUMBER_DEFAULT_DIAMETER` in
  `editor.rs`; the panel imports them instead of duplicating literals
  (geometry bounds are semantic, not panel layout). Default stays
  private — only core constructs it.

## Task Index

### Phase 1: Core (state machine + render path)

- [ ] Task 1: Centered digit anchor in the shared render path → `tasks/001-centered-digit-anchor.md`
- [ ] Task 2: `number_diameter` tool state + pub range constants → `tasks/002-number-diameter-state.md`

### Checkpoint: Core

- [ ] `cargo test -p flintshot-core --quiet` green; new render + state tests in place
- [ ] Standing gates green (`just ci`)

### Phase 2: UI

- [ ] Task 3: Size stepper in the panel's Number section → `tasks/003-panel-size-stepper.md`

### Checkpoint: UI

- [ ] Manual exit criteria 1–3 (spec) pass: centered digits WYSIWYG, stepper drives next placement, clamps hold
- [ ] Standing gates green

### Phase 3: Closeout

- [ ] Task 4: CHANGELOG + TEST_MATRIX + exit-criteria walk → `tasks/004-closeout.md`

### Checkpoint: Complete

- [ ] All spec exit criteria (1–5) annotated met
- [ ] Ready for review/merge

## Risks and Mitigations

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| Geometric centering sits optically low (galley height includes leading) | Med | Spec assumption 4 allows a small tuned optical constant in review — not a new mechanism |
| egui layouter vs ImageCanvas font metrics drift → editor/save anchors differ by a hair | Low | Each canvas centers by its own `measure_text`; per-path centering is the WYSIWYG contract. Compare a saved PNG at 96 px in Task 3 verification |
| Two-digit label overflows at extreme diameters | Low | Math fits both ends (16 px → ~11.5 px label width; 96 px → ~69 px vs 96 px disc); Task 1 tests pin both cases |

## Open Questions

- None new — spec's two deferred items (retroactive resize of selected
  bubbles; 3+ digit shrink-to-fit) stay deferred.
