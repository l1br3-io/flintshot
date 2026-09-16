# Plan: Pop-up Book UI

## Overview

Replace Flintshot's egui skin and structure with the pop-up-book
design-token system from `spec.md`: light-only cream/forest palette,
cutout cards with white borders and painted shadows, Plus Jakarta
Sans, spring easing, paper grain — while keeping Gradia layout
patterns (welcome screen with recents, docked side panel with
contextual tool options, popovers, floating on-canvas controls,
export cluster with Copy as hero) and inert seams for tabs, Objects,
and AI. Ten current tools, all shortcuts, and the capture
choreography are preserved unchanged.

## Architecture Decisions

- **`theme/` module is the only style authority** — `palette.rs`
  holds every `Color32`; `mod.rs` holds spacing/rounding/stroke/
  shadow/type tokens and `apply(ctx)`. Call sites consume tokens;
  the final sweep enforces "no `Color32::from_` outside palette".
- **Light-only** — `ThemePreference::Light` pinned in `theme::apply`;
  dark variant is a follow-up effort (spec decision).
- **Toolbar dissolves into three modules** — `panel.rs` (docked right
  side panel: tools + contextual options + export cluster),
  `popover.rs` (color/stroke bay), `floating.rs` (on-canvas
  undo/redo/magnifier/zoom). `toolbar.rs` is deleted, not wrapped.
- **Recents live in `flintshot-core::recents`** (pure path index,
  TOML beside `config.toml`) so the no-GUI-types-in-core boundary
  test keeps passing; thumbnails decode on demand in editor-egui.
- **Web-spec adaptations are code seams** — `ease.rs` (cubic-bezier
  spring evaluator) and `grain.rs` (deterministic procedural tile)
  are pure, unit-tested modules consumed by widgets; no Z-axis, no
  scroll-linked animation (spec's feasibility table is binding).
- **Header spans both pages** — `header.rs` drawn by the window
  router for Home and Editor routes; tab strip renders one tab, AI
  slot exists but is hidden.

## Task Index

### Phase 1: Foundation (skin + primitives)
- [x] Task 1: Design-token module (`theme/`) with light visuals → `tasks/001-theme-token-module.md`
- [x] Task 2: Embed Plus Jakarta Sans + type ramp → `tasks/002-jakarta-fonts.md`
- [x] Task 3: Spring easing + paper-grain primitives → `tasks/003-motion-texture-primitives.md`

### Checkpoint: Foundation
- [x] Build + full test suite green; app runs re-skinned (light cream)

### Phase 2: Editor structure
- [x] Task 4: `flintshot-core::recents` + recording wiring → `tasks/004-core-recents.md`
- [x] Task 5: Docked side panel replaces the toolbar rail → `tasks/005-side-panel.md`
- [x] Task 6: Floating on-canvas controls → `tasks/006-floating-controls.md`
- [x] Task 7: Contextual tool options + picker popover → `tasks/007-contextual-options-popover.md`
- [x] Task 8: Export cluster with Copy hero → `tasks/008-export-cluster.md`

### Checkpoint: Editor complete
- [x] Capture → annotate → copy works end-to-end from the panel; shortcuts unregressed

### Phase 3: Welcome + header + polish
- [x] Task 9: Header bar with tab-strip and AI seams → `tasks/009-header-bar.md`
- [x] Task 10: Welcome screen (hero + recents grid + drop) → `tasks/010-welcome-screen.md`
- [x] Task 11: Token-only sweep + regression + CHANGELOG → `tasks/011-final-sweep.md`

### Checkpoint: Complete
- [x] All spec success criteria verified; ready for review

## Risks and Mitigations

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| Canvas input gating assumes top toolbar (negative-y drag origin, `canvas_rect.min.y`) | High | Task 5 re-derives gating for a right panel on both axes; dedicated manual test: press starting on panel must never start an annotation drag |
| Selection-first phase (region landing) is toolbar-coupled (`rail_gate`) | Med | Task 5 keeps the `ToolGate` contract; phase tests in `app.rs` must pass unchanged |
| Right panel width vs 1024×640 default editor page | Med | Task 5 verifies canvas fit; `EDITOR_PAGE_DEFAULT_SIZE` bump allowed (test update in same task) |
| Wayland drag-and-drop onto egui viewport | Med | Task 10 treats drop as best-effort; welcome remains complete via open/paste/recents if winit DND is flaky |
| egui 0.35 Shadow/FontDefinitions API drift | Low | All shadow/font setup isolated in `theme/`; call sites never touch egui styling structs directly |
| Font sourcing (download + OFL text) | Low | Task 2 fetches from Google Fonts; license committed beside TTFs; fallback: keep egui default fonts behind the same type ramp |

## Parallelization

Task 4 (core recents) is independent of Phase 1 — safe to run
alongside Tasks 1–3. Tasks 6, 7, 9 are mutually independent after
Task 5. Everything else is sequential per the dependency chain.

## Open Questions

- Exact `INK`/neutral ramp hues — tuned during Task 1 against real
  screenshots (spec allows).
- Backends diagnostics in-app surface — default is an overflow-menu
  hint only (no new plumbing); revisit if the user wants embedded
  probes.
