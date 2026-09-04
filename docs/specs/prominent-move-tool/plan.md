# Plan: Prominent move tool

## Overview

Make `Tool::Move` the resting state of the editor via four measures:
config-gated flip-to-Move after every draw commit (`[ui]
flip-to-move`, default on), Move as the default edit-phase tool,
rail emphasis (Move first + labeled chip in both toolbars), and
hover affordances (Move cursor + dashed bbox in both shells). Spec:
[`spec.md`](spec.md).

## Architecture Decisions

- **Flip lives in `commit_annotation`** (`flintshot-core/src/editor.rs`)
  — the single commit funnel for all eligible tools (pointer-up and
  Text's modal commit both route through it; `place_number` bypasses
  it, exempting Number naturally). Guarded by an **ownership check**:
  flip only when the pending annotation's owning tool matches the
  active tool, so the shell's flush-on-toolbar-switch
  (`app.rs` text-draft commit before an explicit chip/hotkey tool
  change) never clobbers a deliberate tool choice.
- **Policy flag is a plain `EditorState` bool** (`flip_to_move`,
  default `true`), set from config at `state_from_config`
  (`flintshot-app/src/cli.rs`) — the single production construction
  site shared by editor window and overlay. Read-once-per-session
  (magnifier precedent); no cross-process live reload (the daemon's
  watcher is hotkeys-only, and the editor runs in a worker process).
- **Phase-end transition, not construction-time**: the region
  capture flow still opens on `Select` (region-first contract);
  `end_phase_on_crop` flips `Select` → `Move` when the phase
  latches, so the edit phase starts on Move. The overlay keeps its
  region-first entry and relies on the commit flip.
- **Labeled chip** reuses the Copy-primary's glyph+label pattern
  (`copy_chip`); `TOOLS` reorder puts Move first. Shared `TOOLS`
  table means one reorder covers both rails.
- **Hover chrome composes existing core queries** (`hit_test` +
  `Annotation::bounding_box`) shell-side — read-only, no new core
  public API.

## Task Index

### Phase 1: Policy + wiring

- [ ] Task 1: Core flip policy, Move default, config field → `tasks/001-core-flip-policy.md`
- [ ] Task 2: Entry-state wiring (config → state, phase-end → Move) → `tasks/002-entry-state-wiring.md`

### Checkpoint: Policy live end-to-end

- [ ] `cargo run -- region` → draw rect → release → tool flips to Move; drag repositions
- [ ] `[ui] flip-to-move = false` honored on next capture
- [ ] Standing gates green; review with human before Phase 2

### Phase 2: Discoverability surfaces

- [ ] Task 3: Rail emphasis — Move first + labeled chip, both rails → `tasks/003-rail-emphasis.md`
- [ ] Task 4: Hover affordances — cursor + dashed bbox, both shells → `tasks/004-hover-affordances.md`

### Checkpoint: Complete

- [ ] All spec exit criteria 1–6 pass (manual pass through both shells)
- [ ] Standing gates green

### Phase 3: Closeout

- [ ] Task 5: ADR-0040, CHANGELOG, TEST_MATRIX, manual pass → `tasks/005-closeout.md`

## Risks and Mitigations

| Risk                                                            | Impact | Mitigation                                                                                     |
| :-------------------------------------------------------------- | :----- | :---------------------------------------------------------------------------------------------- |
| Ownership guard subtly wrong (toolbar-switch draft flush case)  | High   | Task 1 dedicates a test matrix to the flush ordering; guard ships in the first, smallest task      |
| `#[default]` change breaks tests relying on `Tool::Rect`        | Med    | Audit in Task 1 (`Tool::default()` test) + Task 2 (`plain_editor_keeps_default_tool…`); run gates |
| Chip geometry not unit-testable (needs live egui `Ui`)          | Low    | Data-level tests for `TOOLS` order; visual check in Task 3's manual verification                  |
| Flip annoys rapid multi-draw users                              | Med    | `flip-to-move = false` escape hatch ships in the same effort (Task 1–2)                           |
| Text bbox is a coarse width estimate (see `bounding_box` docs)  | Low    | Accept loose dashed chrome on Text; noted in Task 4                                              |

## Open Questions

- None beyond the spec's own (crop-confirm flip: leaning no).
