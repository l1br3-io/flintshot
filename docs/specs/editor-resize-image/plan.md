# Plan: editor-resize-image

## Overview

Destructive, crop-style resize: `EditorState::resize(w)` scales the
screenshot and every annotation to a smaller pixel width (height
derived, aspect-locked), undoable via the existing `Snapshot::full`
path. Entry point is a Resize item in the editor topbar's overflow
menu opening a presets + custom-width popover. Never upscales.

Spec: [`spec.md`](spec.md).

## Architecture decisions

- **Mirror `crop()`, don't invent.** `EditorState::resize` follows
  `crop`'s exact sequence (`restore_active_reedit` → `Snapshot::full`
  → mutate → clear pending/move/redo). Undo/redo machinery already
  restores images from snapshots — zero history changes.
- **UI placement correction (spec amended):** the spec's original
  "export island, bottom cluster" was based on stale `panel.rs`
  module docs — export actions live in the **topbar overflow**
  (Save / Upload / Paste; see `panel.rs` `ToolbarAction` docs).
  Resize joins that menu. Editor surface only.
- **Scale magnitudes, not just geometry.** `Annotation::scale`
  multiplies points/rects/stroke widths/font sizes/head
  sizes/diameters/`TextOutline::width`, and rounds blur `strength`
  (u8, min 1) so a blur keeps its relative visual size on the
  smaller image.
- **Clamp twice.** The UI clamps (DragValue max = width, apply
  disabled at ≥ current width) and core clamps defensively
  (`resize` target axes clamp to source) — no path produces a
  larger image even if called wrong.

## Task index

### Phase 1: Core capability

- [x] Task 1: `image_ops::resize` — clamped Triangle downscale → `tasks/001-image-ops-resize.md`
- [ ] Task 2: `Annotation::scale` — per-variant transform → `tasks/002-annotation-scale.md`
- [ ] Task 3: `EditorState::resize` — state transition + undo/redo → `tasks/003-editor-state-resize.md`

### Checkpoint: Core complete

- [ ] All four gate commands pass (`fmt --check`, `clippy -D warnings`, both test cmds)
- [ ] Resize fully drivable from a unit test: dims shrink, anns scale, undo restores bit-for-bit
- [ ] Review with human before UI work

### Phase 2: Surface

- [ ] Task 4: Topbar overflow entry + presets/custom popover → `tasks/004-topbar-resize-popover.md`

### Checkpoint: Surface complete

- [ ] Manual: open image in `cargo run -- app`, 50% via popover, view re-fits, save dims correct, Ctrl+Z restores

### Phase 3: Ship prep

- [ ] Task 5: Full gates + CHANGELOG + spec success-criteria sweep → `tasks/005-gates-changelog.md`

### Checkpoint: Complete

- [ ] All spec success criteria checked
- [ ] Ready for review

Tasks 1 and 2 are independent (parallelizable). 3 depends on both.
4 depends on 3. 5 depends on 4.

## Risks and mitigations

| Risk                                                                | Impact | Mitigation                                                                  |
| :------------------------------------------------------------------ | :----- | :-------------------------------------------------------------------------- |
| Resize breaks the selection-first phase latch (app.rs keys off `undo` snapshots with images, app.rs:418) | Med | The latch is one-way by design ("undo must not resurrect the phase" test) — resize pushing an image snapshot mid-phase is harmless; Task 4's app tests cover the routed editor, Task 3 documents the interaction |
| egui menu + popover interplay (Popup from a menu item can close the menu first) | Med | Task 4 explores menu-submenu vs `Popup` off the menu response; acceptance is the popover opening, not a specific mechanism |
| Blur-strength scaling rounding (u8, tiny strengths at 25%)          | Low   | Clamp to ≥ 1; documented in Task 2 acceptance                               |
| `app.rs` is a 5k+ line file — routing changes can break unrelated tests | Low | Task 4 adds one action variant + one popover flag; existing tests must stay green |

## Open questions

- None blocking. Popover visuals follow the existing popover/menu
  styling conventions.
