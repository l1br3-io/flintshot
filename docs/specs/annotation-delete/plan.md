# Plan: annotation-delete — select an annotation, remove it

## Overview

The editor can move a committed annotation but cannot remove an
individual one — the only paths are Ctrl+Z (LIFO, unwinds everything
after the target) and crop (incidental). This effort adds the
Flameshot-standard select-then-delete affordance: a Move-tool click
selects an annotation (persistent after release, dashed ember
outline), `Del` / `Backspace` or a rail Delete button removes exactly
that annotation as a fully undoable `anns`-only snapshot, and both
shells (editor window, X11 overlay) share the core-owned semantics.
Selection is chrome-only — never composed into Save / Copy / Pin /
Upload output.

## Architecture Decisions

- **Core owns selection lifecycle + delete.** The dispatcher's
  Move-tool pointer events gain persistent-selection semantics
  (release keeps `selected_index`; a miss press clears it);
  `delete_selected()` follows the snapshot discipline of every other
  mutation (pre-delete `Snapshot::anns_only`, clear `redo`, drop the
  selection). Both shells inherit for free — no duplicated delete
  logic.
- **`EditorState::set_tool` owns tool-switch deselection.** Tool
  switches bypass the dispatcher today (shells write `state.tool`
  directly from rail chips and hotkeys), so the "switching away from
  Move clears the selection" invariant needs a core method the shells
  route through — a selection-lifecycle transition per the spec's
  core scope, recorded in ADR-0044.
- **`Del` + `Backspace` alias, no modifiers.** Plain Del or Backspace
  deletes; any modifier spoils it (Ctrl combos reserved). The
  text-editing modal mode keeps its Backspace monopoly by
  construction (dispatcher routes text-mode keys to the draft).
- **Esc layering is shell-side.** The dispatcher's Esc → `Cancel`
  contract is untouched; each shell checks `selected_index` first —
  deselect instead of cancel, second Esc cancels.
- **Rail button over an Eraser tool** (spec decision, user-confirmed):
  a `ToolbarAction::Delete` chip beside Undo/Redo on both rails,
  greyed (hover-only) until a selection exists, replaying the same
  `Key::Delete` dispatch the keyboard produces.
- **Selection outline reuses the move-chrome language**: dashed
  ember 1.5 px outline at the annotation's `bounding_box()` (+ pad),
  computed by a pure `chrome.rs` query, painted by both shells, never
  in composed output. Degenerate bbox → deselect instead of render.

## Task Index

### Phase 1: Core state machine

- [ ] Task 1: `Key::Delete` + persistent selection + `delete_selected` + `set_tool` → `tasks/001-core-delete-state-machine.md`

### Checkpoint: Core green
- [ ] `cargo test -p flintshot-core` + clippy clean; dispatcher tests pin every lifecycle rule

### Phase 2: Shells

- [ ] Task 2: `ToolbarAction::Delete` + trash glyph + selection-gated chips on both rails → `tasks/002-toolbar-delete-chip.md`
- [ ] Task 3: Editor window — Del/Backspace translation, Esc layering, selection outline, hint → `tasks/003-editor-shell-delete-ux.md`
- [ ] Task 4: Overlay — Del/Backspace passthrough, Esc layering, outline, `set_tool` routing → `tasks/004-overlay-delete-ux.md`

### Checkpoint: Both shells
- [ ] `cargo run -- fullscreen`: click → outline → Del → gone → Ctrl+Z restores; Esc/Esc cancels
- [ ] `cargo run -- region`: same affordance post-crop; overlay rail button works

### Phase 3: Closeout

- [ ] Task 5: CHANGELOG, TEST_MATRIX rows, ADR-0044, standing gates → `tasks/005-closeout.md`

### Checkpoint: Complete
- [ ] Spec exit criteria 1–9 verified (tests + manual pass)
- [ ] Full standing gate green; ready for /review

## Risks and Mitigations

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| Persistent selection changes `PointerUp` semantics pinned by existing Move tests (`selected_index.is_none()` after release) | Low | Expected RED→GREEN updates confined to the release assertion; the rest of the Move contract (one gesture = one undo entry, lazy snapshot) must stay green untouched |
| `set_tool` is new core API near the spec's ask-first boundary | Med | Framed (and ADR'd) as the selection-lifecycle transition the spec's core scope names; shells' direct `state.tool` writes are routed, not duplicated |
| Esc layering regresses the selection-first phase's Esc-cancel contract | Med | Mid-phase Move is gated so no selection can exist — layering is provably a no-op there; pin with a test |
| Backspace alias collides with text drafting | High | Dispatcher's modal text branch owns Backspace before `dispatch_key` runs — pin with a test (draft edits, committed anns untouched) |
| Hint copy is an ask-first boundary | Low | Task 3 asks the user for the exact Move-hint string before editing it |

## Open Questions

None — the spec's three open questions adopt their defaults:
replace-on-click (last click wins, `begin_move` already does this),
Number-tool gaps stay (no renumbering), and the persistent-selection
lifecycle gets ADR-0044 at closeout.
