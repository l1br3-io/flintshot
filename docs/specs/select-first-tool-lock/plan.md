# Plan: select-first-tool-lock

## Overview

Enforce "select the region first, edit second" on both shells.
Editor window: while the selection-first phase is live, every rail
chip except Select renders disabled (greyed, hover-only, lock
tooltip), tool-switch hotkeys are inert, and annotation pointer
input is denied — the full rail restores on first crop. Overlay:
gate the ungated single-key tool switch on `usable_selection` (the
floating rail already hides without a selection — planning
correction in `spec.md`). No core changes, no config keys.

## Architecture Decisions

- **`ToolGate` enum in `toolbar.rs`** (`All` / `SelectOnly`), pure
  and unit-testable; shells compute it, the rail renders it. Chip
  primitives (`chip`, `labeled_chip`) gain an `enabled: bool` param
  rather than a parallel "locked chip" widget — one rendering path,
  mechanical `true` at non-tool call sites.
- **Editor input gating is phase-scoped, not geometry-scoped.** The
  phase has no committed region, so annotation presses are denied
  wholesale (`phase_active() && tool != Select → skip dispatch`) —
  simpler and stricter than mirroring the overlay's
  `annotation_press_allowed` geometry.
- **Overlay = keyboard gate only.** `show_toolbar` (overlay.rs)
  never renders without a usable selection; `annotation_press_allowed`
  already denies presses. The one residual hole is the `tool_for_key`
  block in `logic_impl` — gate it on `usable_selection`, keep Select
  armable.
- **Unlock points follow each shell's region-commit semantics**:
  editor on crop (`SelectionPhase::Ended`), overlay on usable
  selection. `end_phase_on_crop`'s flip-Select→Move stays.

## Task Index

### Phase 1: Editor rail lock (visible)

- [ ] Task 1: `ToolGate` + disabled-chip rendering + editor rail wiring → `tasks/001-toolgate-disabled-chips.md`

### Phase 2: Editor input gates (behavioral)

- [ ] Task 2: Phase-scoped hotkey + pointer gating in `EditorApp` → `tasks/002-editor-input-gates.md`

### Checkpoint: Editor shell complete
- [ ] `cargo run -- region`: chips greyed pre-crop, click/hotkey/draw all inert, Enter crops, rail wakes up
- [ ] Standing gates green

### Phase 3: Overlay keyboard gate

- [ ] Task 3: Overlay tool-switch keys gated on usable selection → `tasks/003-overlay-key-gate.md`

### Phase 4: Closeout

- [ ] Task 4: ADR-0043, CHANGELOG, TEST_MATRIX rows → `tasks/004-docs-closeout.md`

### Checkpoint: Complete
- [ ] Spec exit criteria 1–5 verified (manual pass + tests)
- [ ] Full standing gate green; ready for /review

## Risks and Mitigations

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| Synthetic-context click tests for disabled chips (no precedent for pointer-on-chip, only keys) | Med | Gate predicate + `rail_gate(phase_active)` tested pure; click-through tested via the existing synthetic-`egui::Context` pattern (`enter_key_crops_through_shell`) |
| `enabled: bool` churn across ~15 chip call sites | Low | Mechanical; clippy `-D warnings` catches unused-param mistakes; only tool-chip call sites pass a live value |
| Phase pointer-gate blocks more than intended (e.g. Select drags, outputs) | Med | Gate narrowly: `phase_active() && state.tool != Tool::Select`; existing phase tests (Esc-cancel, Enter-crop, selection-aware outputs) must stay green untouched |

## Open Questions

None — spec open questions resolved during planning (see
`spec.md`).
