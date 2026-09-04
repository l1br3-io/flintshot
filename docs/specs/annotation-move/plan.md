# Plan: annotation-move — reposition committed annotations

## Overview

Annotations are placed once and frozen; the only remedy for a
mis-placed rect / arrow / text / blur is undo or redraw. This
effort adds grab-and-drag repositioning under `Tool::Select`,
owned entirely by the `flintshot-core` dispatcher so both shells
(editor window, fullscreen overlay) inherit it. The overlay's only
change is routing: its chrome intercepts Select presses for region
move/resize today, so it must let annotation hits fall through to
the dispatcher instead.

## Architecture decisions

- **Core owns the move.** The dispatcher already sees every
  Select-tool pointer event in both shells; `hit_test`,
  `bounding_box`, and `selected_index` (an existing placeholder
  documented as "Phase 2 select-and-move tool") are the full
  supporting cast. No new `Gesture` variant in the overlay —
  annotation presses become ordinary dispatcher events.
- **Grab priority: handles → annotation → region interior → fresh
  selection.** The handle is the region's affordance; the
  annotation is the topmost drawn thing under the cursor;
  everything else keeps today's semantics.
- **Lazy undo snapshot on first actual movement.** Pushing at grab
  time would record click-without-move no-ops as history entries;
  first movement is the honest commit point. One gesture = one
  undo entry.
- **Wholesale `anns` swaps cancel the drag.** `undo` / `redo` /
  `crop` replace the annotation list; an in-flight move referencing
  an index into the old list must not survive.
- **Unclamped translation.** Paint-time clip (overlay) and `crop`
  (output) already own overflow — same semantics as mid-drag
  stroke overflow. Moving a blur recomputes it per frame
  (parameter-keyed `BlurCache`), identical cost to drawing one.

## Task index

1. `tasks/001-core-move-state-machine.md` — `Annotation::translate`,
   `AnnotationDrag`, dispatcher wiring, undo guards, tests.
2. `tasks/002-overlay-gesture-fallthrough.md` —
   `gesture_for_press` annotation fall-through, hover cursor,
   tests.
3. `tasks/003-closeout.md` — CHANGELOG, TEST_MATRIX, manual pass,
   standing gate.

## Verification strategy

Core + overlay unit tests cover the lifecycle and routing; the
spec's exit criteria drive the manual pass (`cargo run -- region`).
Standing gate per [`AGENTS.md`](../../../AGENTS.md) at closeout.
