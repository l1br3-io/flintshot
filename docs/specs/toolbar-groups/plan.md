# Plan: Toolbar grouped pods

## Overview

Reorganize both tool surfaces — the editor window's rail and the
Phase 5 overlay's floating toolbar — from flat chip rows into
semantically grouped pods rendered as rounded islands. Pure layout
change: same 11 tools, same single-key shortcuts, same glyphs,
same actions, same `ToolGate` semantics, no core-crate changes.
Spec: [`spec.md`](spec.md).

## Architecture Decisions

- **D1 — Island containers, not hairlines.** Each pod sits in a
  rounded graphite container one step off the panel (new
  `GRAPHITE_800`-style constant), gaps between pods replace
  `ui.separator()`. Strongest grouping signal; matches the
  Excalidraw/CleanShot references. *(This resolves spec Open
  Question 1 — confirm at plan review; fallback is wider gaps +
  existing hairlines.)*
- **D2 — `TOOL_PODS` replaces `TOOLS` as the single table**;
  pod walk preserves chip ids (`("tool", key)` /
  `("overlay_tool", key)`), so click-through and gate tests pass
  unmodified. The table lives in `toolbar.rs`; both rails consume
  it.
- **D3 — Overlay adapts through measurement, not new logic.** The
  floating toolbar already re-measures every frame and re-anchors
  via `toolbar_anchor`; a wider pod rail flows through unchanged
  (`overlay.rs` measured-size repaint path).
- **D4 — History pod (Undo/Redo/Delete) and outputs pod
  (Save/Pin/Upload/Copy) are islands too**, on both surfaces; the
  ember Copy primary keeps its visual weight inside the outputs
  island.

## Task Index

### Phase 1: Editor surface
- [x] Task 1: Editor rail pods — table, islands, rail rewrite → `tasks/001-editor-rail-pods.md`

### Checkpoint: Editor surface
- [x] Gates green (`fmt` / `clippy -D warnings` / workspace tests /
  doc tests)
- [x] Human eyeballs `cargo run -- app`: island visuals approved
  **before** the overlay adopts them (D1 decision gate) — approved
  2026-09-13 on the closeout screenshots (perceivable without
  tooltips)

### Phase 2: Overlay surface
- [x] Task 2: Overlay floating toolbar adopts the pods → `tasks/002-overlay-pod-rail.md`

### Checkpoint: Both surfaces
- [x] Gates green; `cargo run -- region` — overlay anchored and
  fully on-panel (live overlay N/A on Wayland — session-wedge
  observed; anchoring pinned by the wide-pod-rail unit test added in
  task 003)

### Phase 3: Closeout
- [x] Task 3: Closeout — narrow-width matrix, CHANGELOG, review kit → `tasks/003-closeout-verification.md`

### Checkpoint: Complete
- [x] Spec success criteria 1-4 verified
- [x] Ready for `/review` → `/ship`

## Risks and Mitigations

| Risk                                             | Impact | Mitigation                                                                   |
| :----------------------------------------------- | :----- | :--------------------------------------------------------------------------- |
| Islands read as mud against graphite-700 chips    | Med    | Checkpoint 1 is the explicit human eyeball gate; tweak the pod constant there |
| Pod padding pushes rail past narrow captures     | Med    | Rail's horizontal-scroll fallback already holds; Task 3 runs the 500 px matrix |
| Overlay toolbar too wide for small selections    | Med    | `toolbar_anchor` clamps + measured-size repaint already adapt; shrink gaps before wrapping (spec OQ2) |
| Outputs island breaks right-alignment inside Frame| Low    | Layout trick documented at the site if non-obvious; click ids frozen        |

## Open Questions

- **D1 confirmation** (islands vs. hairline fallback) — human call
  at plan review / Checkpoint 1.
- Spec OQ2 (may the overlay tool row wrap at extreme narrowness?)
  stays **no** unless Task 3's matrix produces evidence otherwise.
