# Plan: Select / Crop / Hand tool refactor

## Overview

Split the editor's conflated tool palette into three proper jobs: `Tool::Select`
becomes an object-selection tool (click / toggle / marquee / drag-moves),
`Tool::Crop` (new) inherits today's region-crop behavior, and `Tool::Move` is
renamed `Tool::Hand` and pans the view — pan-only, per the mainstream
hand-tool contract. Adds wheel zoom + middle-drag pan from any tool. Spec:
`docs/specs/select-crop-hand/spec.md`.

## Architecture Decisions

- **Core owns semantics, shell owns the view.** Marquee state and press
  routing live in `EditorState` / `dispatch_input` (toolkit-free, testable).
  Pan/zoom lives only in `EditorApp` (`pan` offset composed into the canvas's
  `content_offset`); core never sees pan. Pointer mapping already flows
  through `CanvasView::over(rect.translate(content_offset))`, so pan composes
  for free at every existing mapping site (canvas.rs:271).
- **One cutover task (002), then semantics.** The region machinery
  (`selection` field → `crop_region`, Enter path, gates, pre-arms, hotkeys,
  rail) moves from Select to Crop in a single working slice because the field
  rename ripples through every shell anyway. Select is briefly dispatcher-dead
  until Task 003 gives it selection semantics.
- **Hand is pan-only (mainstream contract).** Photoshop / GIMP / Figma /
  Excalidraw all keep object manipulation off the hand tool; we follow. The
  shell never dispatches Hand presses to core (core no-ops them
  defensively); selection and moving live on Select alone. An earlier draft
  carried Shift/Ctrl+drag select-and-move on Hand — dropped (see spec
  decision log).
- **Selection-owning tools stay `Select | Text`** in `set_tool` clearing,
  `selected_chrome`, and the inspector gating — the same pair as today's
  `Move | Text`, with Select taking Move's seat.
- **Wheel zoom reuses the 0.25-step quantization** of the floating chips so
  `at_fit_zoom == 1.0` stays exact (powers of two in f32) and the worker
  resize policy keeps its trigger.
- **Default tool becomes `Select`**; the post-commit flip policy renames
  `flip-to-move` → `flip-to-select` with silent legacy-key migration.

## Task Index

### Phase 1: Cutover
- [ ] Task 1: Rename `Tool::Move` → `Tool::Hand` (mechanical, workspace-wide) → `tasks/001-rename-move-to-hand.md`
- [ ] Task 2: Crop tool cutover — region machinery, rail/hotkeys/gates, overlay + phase pre-arm → `tasks/002-crop-tool-cutover.md`

### Checkpoint: Cutover
- [ ] Full finish sequence green (fmt / clippy `-D warnings` / tests / doc tests)
- [ ] Overlay + selection-first flows work with Crop pre-armed
- [ ] Review with human before Phase 2

### Phase 2: Core semantics
- [ ] Task 3: Select becomes the selection tool (marquee + routing + chrome + phase latch) → `tasks/003-select-tool-semantics.md`
- [ ] Task 4: `flip-to-select` config rename + migration → `tasks/004-flip-to-select-config.md`

### Checkpoint: Core semantics
- [ ] Full finish sequence green
- [ ] Select: click / toggle / marquee / drag-move / Del all work in `cargo run -- app`
- [ ] Hand: plain drag does nothing yet (pan lands in Phase 3) — expected
- [ ] Review with human before Phase 3

### Phase 3: View + polish
- [ ] Task 5: Pan + wheel zoom (pan state, clamping, zoom-to-cursor, middle-drag, hand-drag) → `tasks/005-pan-zoom.md`
- [ ] Task 6: Panel / statusline polish (sections map, hints, glyphs final pass) → `tasks/006-panel-polish.md`
- [ ] Task 7: Docs — CHANGELOG, ADR-0063 (tool remap + config migration), CLI help → `tasks/007-docs.md`

### Checkpoint: Complete
- [ ] All spec acceptance criteria verified (manual checklist in spec)
- [ ] Full finish sequence green
- [ ] Ready for review / PR

## Risks and Mitigations

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| Task 002 cutover touches 6 files at once | High | Mostly mechanical renames + branch-key flips; the old Select test suite renames to Crop and must stay byte-identical in behavior; lands immediately after the trivial 001 to shrink the diff window |
| Config comment preservation when renaming `flip-to-move` on re-save | Med | Mirror the existing comment-preservation test (config.rs:1304); migration writes the new key and moves any comment with it |
| Pan breaking pointer mapping / magnifier | Med | All mappings go through `CanvasView` with `content_offset`; pan only changes that offset — plus pure-fn tests for clamp and zoom-to-cursor |
| `at_fit_zoom` float drift with wheel input | Med | Quantize wheel to the same 0.25 steps as the chips (powers of two stay exact in f32) |
| Mid-drag tool switches under the new Select | Low | Existing guards (`moving` survives tool switch, chrome keys off `moving`) carry over; add tests mirroring chrome.rs:321 |
| Hand tool feels inert before Task 005 lands | Low | Same-window sequence on one branch; checkpoints note the expected gap |

## Open Questions

- Spec §Open Questions 1–2 (wheel granularity, marquee modifiers) are
  answered by the recommended options baked into the tasks; revisit at
  checkpoints if the human overrides.
