# Plan: Editor capture UX — home-parity `+`, stable window, routing, capture progress

## Overview

Four deliverables on the single app window: (1) the editor `+` popup
mirrors home (Capture / Open… / Paste) with the portal-native grab
guaranteed first and xcap as fallback only; (2) a stable
fit-to-view editor viewport replacing image-sized windows; (3)
in-window page routing — one eframe lifetime per window generation,
surface destruction reserved for captures and user quit; (4) a
loading route that puts the editor on screen with a progress
indicator the moment the OS selection UI confirms, bridging the
portal's >1 s clipboard-write gap. Spec:
`docs/specs/editor-capture-ux/spec.md`.

## Architecture Decisions

- **Fit-to-view scale lives in the canvas transform.** Image and
  annotations stay in physical image px (no data-model change);
  `EguiCanvas`'s physical→logical mapping gains a fit factor
  (`scale`), threaded through every pointer/chrome site via one
  shared view helper. With an image-sized window, fit == 1.0, so the
  change can land byte-identical before the viewport flips (Task
  003 → 004 ordering).
- **One `run_window`, routes not epochs.** `run_home` /
  `run_editor_page` merge into a router app (editor-egui) holding
  Home / Editor / Loading routes; transitions swap routes in-place
  with a one-time viewport command batch (InnerSize / Resizable /
  Title). The app crate's outer loop keeps only what destruction
  needs: close classification (capture intent, hidden-by-event,
  user quit) and generation spawn on `Show`. Bridge contracts
  (`HomeBridge`, `EditorPageBridge`) survive unchanged — only the
  *reaction* to a routed Open/Paste changes (swap, not close).
- **The grab splits into two phases.** `native_grab_prepared` splits
  at the `capture_selection` return: image-in-hand (`Ready`) vs
  awaiting the portal's clipboard write (`AwaitClipboard`). The grab
  thread reports `Confirmed` (→ generation opens on the Loading
  route; pixels are already frozen, so a window cannot pollute the
  grab) then `Finished` (→ image delivered into the open window via
  the shared slot + directive; a delivery to a still-closed window
  opens on the Editor route — fast read-backs included). Superseded
  generations fold silently at both phases.
- **Core types stay put.** `NewCaptureMode` and
  `EditorOutcome::NewCapture` remain (the worker recapture loop
  uses them); only the page UI stops offering capture kinds.

## Task Index

### Phase 1: Quick wins (independent)

- [ ] Task 001: `+` popup mirrors home → `tasks/001-plus-popup-home-parity.md`
- [ ] Task 002: portal-first determinism (diagnose the observed xcap fallback) → `tasks/002-portal-first-determinism.md`

### Checkpoint A: after 001–002
- [ ] Full gate green; manual: `+` popup shows the three home actions; `+` Capture opens the compositor selection UI on the dev session

### Phase 2: Stable editor size

- [ ] Task 003: fit-to-view canvas scale (lands at scale = 1.0) → `tasks/003-fit-to-view-canvas.md`
- [ ] Task 004: stable editor viewport + crop refit → `tasks/004-stable-editor-viewport.md`

### Checkpoint B: after 003–004
- [ ] Full gate green; manual: every capture opens the editor at the same size, image fitted and centered, annotations/magnifier/move-chrome correct under scale

### Phase 3: Routing + progress

- [ ] Task 005: router skeleton — `run_window` with the Home route; app crate adopts → `tasks/005-router-skeleton.md`
- [ ] Task 006: Editor joins the router — in-window swaps, close classification shrinks → `tasks/006-editor-joins-router.md`
- [ ] Task 007: grab outcome split (`Ready` / `AwaitClipboard`, two-phase delivery) → `tasks/007-grab-outcome-split.md`
- [ ] Task 008: Loading route + in-window delivery → `tasks/008-loading-route-delivery.md`

### Checkpoint C: after 005–008
- [ ] Full gate green; manual: Open/Paste/Esc transitions keep the same window (no WM churn); tray capture → editor with spinner <~200 ms after confirm → image swaps in-place; wedged read-back shows an error, never a hang

### Phase 4: Close-out

- [ ] Task 009: remove the editor chrome strip (contingent — OQ-1) → `tasks/009-chrome-strip-removal.md`
- [ ] Task 010: CHANGELOG + ADR + final gate → `tasks/010-closeout-changelog-adr.md`

### Checkpoint D: complete
- [ ] All spec success criteria met; ready for review

## Risks and Mitigations

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| Runtime viewport resize on Wayland is compositor-negotiated (async, fractional-scale drift) | Med | Only two fixed geometries (home compact, editor default clamped to work area); accept one-frame pop; clamp via egui monitor info |
| Fit-scale threaded through many pointer/chrome paint sites — a missed site corrupts input or chrome placement | High | One shared view helper + round-trip unit tests (logical→image→logical) + manual smoke checklist (annotate, move-chrome, magnifier, selection-phase, crop) in Task 003 |
| Delivery-order races: read-back finishes before the Loading generation opens; supersede arrives mid-read-back | Med | Delivery state machine handles both window-open and window-closed deliveries; generation checks fold stale phases (Task 008 acceptance) |
| EditorApp outcome lifecycle changes under routing (Esc routes home instead of closing) regress worker path | Med | Worker path uses `run_editor_blocking` (untouched); page-path outcome tests ported in Task 006 |
| Portal determinism investigation finds nothing (stale build) | Low | Task 002 accepts a verified-behavior-only outcome; log line `backend=portal` is the evidence |

## Open Questions

- OQ-1 chrome strip fate (Task 009 contingent; lean: remove).
- OQ-2 editor default size (~1024×640 logical; encoded in Task 004).
- OQ-4 home geometry under routing (lean: keep compact 360×240;
  encoded in Task 005).
