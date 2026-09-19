# Plan: Text select & re-edit

## Overview

Make committed text annotations first-class objects under
`Tool::Text`: single click selects (chrome + panel reflection),
double-click re-opens content editing via a lift-out session that
reuses the existing text modal path, panel style changes write
through to the selection (and sync session defaults) with
gesture-coalesced undo, and Del/Esc act on the selection. Semantics
live in `flintshot-core`'s dispatcher; both shells get identical
behaviour via routing only.

## Architecture decisions

- **`InputEvent::DoubleClick` variant, not `click_count` on
  `PointerDown`** — two prod pumps construct `PointerDown`
  (`app.rs`, `overlay.rs`) but ~80 test sites would churn on a new
  field; a variant is additive and no-ops cleanly.
- **Lift-out re-edit session** — the double-clicked annotation moves
  out of `anns` into `pending`, reusing the entire existing modal
  path (cursor ops, key suppression, commit/cancel) with zero
  duplication. A session record stores `{index, original}`; commit
  splices back and pushes exactly one undo snapshot containing the
  original; Esc reverts with no history touch.
- **Selection chrome widening, not new chrome** —
  `chrome::selected_chrome`'s Move-only gate widens to `Move |
  Text`; both shells and both delete affordances (panel Selection
  section, overlay delete chip) already key off `selected_index`
  tool-agnostically.
- **Gesture-bracketed restyle** — `begin/restyle/end` trio in core:
  begin stages the pre-gesture snapshot, restyle mutates target +
  syncs session defaults, end pushes one undo entry. Mid-session
  restyles absorb into the session's single entry (lift-out
  invariant). Panel wraps discrete controls as one-tick gestures and
  DragValue drags as one gesture.
- **Panel source switch** — the Text section's read/write source is
  session draft > selection > session state, so fresh-draft behavior
  is byte-identical when nothing is selected.

## Task index

### Phase 1: Selection

- [x] Task 1: Core — Text-tool click-to-select state machine → `tasks/001-core-text-select.md`
- [x] Task 2: egui — selection chrome under Text → `tasks/002-chrome-text-select.md`

### Checkpoint: Selection (after Task 2)

- [x] `cargo test -p flintshot-core -p flintshot-editor-egui` green
- [ ] Manual: select → dashed outline; Del deletes; Esc deselects
      without closing — both shells (`cargo run -- capture`)

### Phase 2: Re-edit

- [x] Task 3: Core — `DoubleClick` + lift-out re-edit session → `tasks/003-core-doubleclick-session.md`
- [x] Task 4: egui — double-click plumbing in both shells → `tasks/004-egui-dblclick-plumbing.md`

### Checkpoint: Re-edit (after Task 4)

- [ ] Manual: double-click → caret at end → edit → Enter; exactly
      one Ctrl+Z reverts; Esc mid-edit reverts with no undo entry

### Phase 3: Restyle

- [x] Task 5: Core — restyle API + gesture bracketing → `tasks/005-core-restyle-gesture.md`
- [x] Task 6: egui — style surfaces retarget (panel + popover) → `tasks/006-egui-style-retarget.md`

### Checkpoint: Restyle (after Task 6)

- [ ] Manual: panel/pod reflect selection; changes apply live, sync
      defaults; DragValue drag = one undo entry; mid-session restyle
      absorbed into the session entry

### Phase 4: Polish & closeout

- [x] Task 7: egui — hover affordance under Text → `tasks/007-egui-hover-affordance.md`
- [x] Task 8: Closeout — CHANGELOG, TEST_MATRIX, ADR, gates → `tasks/008-closeout.md`

### Checkpoint: Complete (after Task 8)

- [ ] Spec exit criteria 1–6 verified
- [ ] Standing gates green (fmt / clippy / test --workspace /
      doc tests)
- [ ] Ready for review

## Risks and mitigations

| Risk                                              | Impact | Mitigation                                                                                  |
| :--- | :--- | :--- |
| Stale index during lift-out session (shell-direct crop / undo mid-session) | High   | Defensive original-restore guards at the top of `crop` / `undo` / `redo`, covered by tests |
| egui double-click API varies by version           | Low    | Self-contained tracker module using egui's style thresholds; source-driven check in Task 4 |
| Panel dual-source complexity (session / selection / session state) | Med | Explicit source ordering + unit tests per source path |
| Undo-exactness regressions                        | Med    | Every core task asserts its one-entry contract; existing dispatcher tests must stay green |
| DragValue gesture detection edges (typed value, release outside) | Low | Worst case degrades to extra undo entries, never lost edits; acceptance test pins the normal path |

## Open questions

- Inherited from spec §Open questions (fresh-draft live restyle
  defaults to *no*; caret-from-click is v2; non-Text write-through
  is the future generalization).
