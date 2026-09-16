# ADR-0055: The side-panel pivot and chip-rect input gating

## Context

The pop-up book effort (spec `docs/specs/popup-book-ui/`) replaces
the editor's top toolbar with a docked right side panel and a
floating on-canvas quick cluster. Input gating historically
reasoned about the toolbar as a *top* strip: comments (and one
resize computation) leaned on "presses above `canvas_rect.min.y`
are chrome". The floating cluster broke that mental model entirely
— its chips sit **inside** the canvas rect, so a chip press is a
canvas hit by the old test.

## Decision

1. **Layout-reservation gating stays.** Panel chrome never needs
   axis-specific logic: egui reserves panels before the
   `CentralPanel`, so `canvas_rect.contains(press)` already
   excludes the side panel and header. The only real work was
   replacing the worker-window resize math (image + panel width +
   header height, not image + toolbar height).
2. **Floating chrome gates on chip rects, not the cluster
   bounding rect.** `floating::show` publishes the individual chip
   rects; the shell's press gate excludes only those. A pointer
   miss *between* chips passes through to the canvas — the spec's
   passthrough contract — and no canvas area is shadowed by the
   cluster's empty space.
3. **Inert seams are constants, not comments.** The header's AI
   slot and the panel's Objects section are `const …_LIVE: bool =
   false` with an `unreachable!` block: the seam compiles to
   nothing, has exactly one home, and flipping the constant is the
   whole integration point.

## Consequences

- A press matrix (panel edge, between chips, on a chip) is pinned
  headlessly; gestures start exactly on canvas pixels.
- Annotation fixtures must avoid the canvas's top-left ~150×48 px
  (the cluster's berth) — tests moved their annotations below it.
- egui `Area`-hosted widgets hit-test one pass behind their
  layout; click tests settle a hover pass before pressing (and
  popups anchor one pass after opening — the `+` popup helper
  settles twice).
- Spec waivers recorded: Pin stays out (pin-to-desktop was removed
  before this effort); blur strength stays out (no core model —
  the kind segmented control is Blur's whole option surface).
