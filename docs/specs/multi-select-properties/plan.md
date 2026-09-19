# Plan: Multi-select annotations + Properties right-panel refactor

## Overview

Activate the selection-model seam `popup-book-ui` reserved: replace
`EditorState::selected_index: Option<usize>` with an ordered
selection set (Shift+click toggles, click replaces, miss clears),
make drag and `Del` operate on the whole set (one gesture = one undo
entry), and refactor the right panel into a live **selection
inspector** — intersection-of-properties sections whose edits apply
undoably to the selected annotations and sync the session tool
defaults. Spec: [`spec.md`](spec.md).

## Architecture Decisions

1. **`InputEvent::PointerDown` gains `modifiers: Modifiers`** — the
   only lossless way to route Shift+click through the pure
   dispatcher. Workspace-internal break; both shells' construction
   sites (and tests) update in the same task as the field addition.
   `translate_modifiers` already exists in `app.rs`.
2. **Selection representation: `EditorState::selected: Vec<usize>`**,
   maintained sorted + deduped (documented invariant, private
   helpers). The region selection keeps the name `selection` — the
   collision hazard flagged in the spec is why the set is `selected`.
3. **Multi-drag = incremental pointer delta** over the whole set.
   The single-drag grab-offset contract (grab point tracks cursor,
   no snap) is preserved: per-event `translate(dx, dy)` from the
   pointer's own delta is identical for the single case and
   trivially generalizes. Lazy undo snapshot on first movement
   stays.
4. **Undo coalescing for property edits lives in core** (resolves
   spec OQ1): mutators push a snapshot only when the edit *kind*
   differs from the previous mutator call; shells bracket continuous
   widget interactions with `end_property_edit()` on interaction end
   (egui `drag_stopped` / `lost_focus` / discrete click). Discrete
   clicks are one entry each by construction.
5. **No `flip_to_move` interaction** (resolves spec OQ2): the
   post-commit flip lands on Move with a single-member set; nothing
   extra to do.
6. **Capabilities are a plain bool-field struct, `#[non_exhaustive]`**
   — mirrors the `Modifiers` precedent (independent bits, not a
   state machine); matrix lives in one pure query.

## Task Index

### Phase 1: Core selection foundation
- [x] Task 1: Selection set + Shift routing + delete-all → `tasks/001-core-selection-set-routing.md`
- [x] Task 2: Multi-annotation drag → `tasks/002-core-multi-drag.md`

### Checkpoint: Core behaviors complete
- [ ] `cargo test --workspace --all-targets` + clippy + fmt green
- [ ] Manual: shift-click builds a set, drag moves it, one Ctrl+Z, Del clears all — via `cargo run -- app`
- [ ] Review with human before proceeding

### Phase 2: Property editing
- [x] Task 3: Capabilities query + property mutators + undo coalescing → `tasks/003-core-capabilities-mutators.md`
- [x] Task 4: Panel selection-inspector mode → `tasks/004-panel-inspector.md`

### Checkpoint: Inspector usable end-to-end
- [ ] Exit criteria 1–5 of the spec pass manually
- [ ] Human review of the panel UX before chrome polish

### Phase 3: Chrome + closeout
- [ ] Task 5: Plural selection chrome in both shells → `tasks/005-chrome-shells.md`
- [ ] Task 6: Closeout (CHANGELOG, TEST_MATRIX, ADR, full gates) → `tasks/006-closeout.md`

### Checkpoint: Complete
- [ ] All spec exit criteria met; standing gates green; ready for PR

Note: Task 5 depends only on Tasks 1–2 and may run parallel to
Tasks 3–4 if agents are available.

## Risks and Mitigations

| Risk                                                               | Impact | Mitigation                                                                                        |
| :----------------------------------------------------------------- | :----- | :------------------------------------------------------------------------------------------------ |
| `PointerDown` field addition ripples through many test sites        | Med    | Compiler-driven sweep in the same task; no half-updated state possible                             |
| Undo bloat if continuous-widget bracketing is missed                | Med    | Core-side kind-based coalescing + dedicated sequence tests; manual DragValue check in Task 4       |
| Overlay gesture-routing races regressing (hard-won fixes)          | High   | Shift only branches the Move-tool arm; full overlay suite is the gate; no region-selection changes |
| Index invalidation while selected (crop racing a selection)        | Med    | Standing rule kept: crop / undo / redo clear the set wholesale                                     |
| Panel regression in no-selection mode                               | Med    | Byte-for-byte mode asserted by existing panel tests, kept passing unchanged                        |

## Open Questions

- None blocking. Spec OQ1 and OQ2 are resolved by Architecture
  Decisions 4 and 5; the ADR in Task 6 records the coalescing
  rationale.
