---
status: active
started: 2026-09-09
---

# Spec — Editor new-capture: the `+` chip / Ctrl+N

> Mini-effort spec. Product context: `../../VISION.md`. Builds on
> the app-window effort (ADR-0045's process-lifetime session
> semantics) and the region-selection-first landing experience.

## Objective

Win11's Snipping Tool editor has a **"+"** control: start a fresh
snip without leaving the editor's flow. Add the same to flintshot:

- A `+` chip leading the editor toolbar's rail, opening a small
  popup: *Region · Ctrl+N* / *Fullscreen*.
- `Ctrl+N` as the keyboard fast path for the region default
  (Win11's chord).
- The initiating window must never be in the new shot: the editor
  window **closes** (a destroyed window can't be captured) and the
  caller waits out the same 250 ms unmap grace the app window's
  hide uses before grabbing.

## Decisions (interview follow-up, 2026-09-09)

1. **In-process re-capture loop, not daemon delegation** — the
   editor worker process closes its window, re-captures, and opens
   a fresh session. ADR-0045 counts sessions by worker *process*,
   so the app window stays hidden across the whole chain and
   restores only when the user finally exits — no
   spawn/exit flash, no double editors. See ADR-0046.
2. **Unsaved annotations are discarded** on a new capture,
   Win11-style (no confirmation in v1).
3. **Both shells** — the overlay shares the toolbar (and thus the
   chip), so its `NewCapture` arm is the editor's twin; a drawn
   chip that does nothing would be a dead control.

## Scope (in / out)

**In:**

- `EditorOutcome::NewCapture { mode }` + `NewCaptureMode`
  (`flintshot-core`, appended; the enum is not wire-encoded).
- `ToolbarAction::NewCapture(NewCaptureMode)`; `new_capture_chip`
  popup leading the rail; Ctrl+N in `EditorApp::logic_impl`
  (after the dispatcher chords, before the tool keys — plain N
  stays the Text tool key).
- Overlay: publish-and-close twin arm.
- App crate: `run_editor_with_config` / `run_overlay_with_config`
  loop — close → grace → `capture_with_tray_hidden` → fresh
  session (region re-enters selection-first; fullscreen stays
  plain). Monitor: default (active), like every other quick path.
- ADR-0046, CHANGELOG, TEST_MATRIX rows + XV1 scripted pass.

**Out:**

- Confirmation dialog for discarding unsaved annotations.
- Per-monitor recapture pickers; delay options.
- Snipping-mode changes (freeform/window — the `snipping-modes`
  future effort).

## Exit criteria

1. `+` → *Region* / Ctrl+N from a region session reopens the
   editor selection-first over a fresh freeze-frame; the closed
   editor is not in the new frame.
2. `+` → *Fullscreen* reopens plain, whole-monitor.
3. The overlay's chip behaves the same (reopens the overlay).
4. A capture chain started from the app window keeps the app
   window hidden until the chain ends, then restores it.
5. Plain `N` still switches to the Text tool; Ctrl+N never
   triggers while a text draft is live (the modal branch returns
   first).
6. Standing gates green.

## Boundaries

- **Always:** editor-egui stays capture-free (the outcome is a
  request; the app crate owns the loop + the grab).
- **Ask first:** new dependencies (zero expected).
- **Never:** capturing in the daemon; GUI types in core's API.
