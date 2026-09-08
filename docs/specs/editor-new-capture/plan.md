# Plan: editor-new-capture — the `+` chip / Ctrl+N

## Overview

`EditorOutcome::NewCapture` turns "start over" into a terminal
outcome the *caller* services: the editor window closes (the
not-in-the-shot guarantee), the app crate waits the unmap grace,
captures a fresh monitor with the tray hidden, and re-runs the
session — selection-first for region, plain for fullscreen. The
worker process persists across the loop, so ADR-0045's
process-lifetime session accounting needs no changes.

## Architecture decisions

- **Outcome, not shell-side capture** — editor-egui publishes
  intent and closes; the loop + grab stay in the app crate
  (`recapture_image`), keeping the egui crate capture-free.
- **`publish_outcome`, not `if_unset`** — a prior copy's outcome
  must not mask an explicit "start over"; the clipboard side
  effect already happened either way.
- **Ctrl+N sits between the dispatcher chords and the tool keys**
  — `dispatch_shortcut_key` doesn't translate N, and the tool-key
  pass guards on the event's own modifiers, so plain N stays the
  Text tool key while the chord starts a capture. The modal
  text-draft branch returns earlier, so a draft can't swallow it.
- **One chip, popup for the second kind** — Region is the default
  (Win11 parity); Fullscreen rides the same popup rather than a
  second rail chip.

## Task index

1. `tasks/001-chip-outcome.md` — core outcome + toolbar chip +
   Ctrl+N + overlay twin (editor-egui, flintshot-core).
2. `tasks/002-app-loop.md` — the recapture loop in both shell
   paths; `recapture_image` / `selection_first_for_recapture`.
3. `tasks/003-docs-matrix.md` — ADR-0046, CHANGELOG, TEST_MATRIX
   rows + XV1 scripted pass.

## Verification

Unit: outcome mapping (`recapture_request`,
`selection_first_for_recapture`), Ctrl+N shell translation
(synthetic `egui::Context`, winit-shaped modifiers). Manual/XV1:
chain from the app window stays hidden; fresh frame lacks the old
editor; overlay chip reopens the overlay.
