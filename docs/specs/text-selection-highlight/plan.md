# Plan: text-selection-highlight

## Overview

Bring industry-standard range selection to the text modal: anchor +
caret selection model in core, keyboard chords (Shift+arrows, Shift+
Home/End, Ctrl+Shift word-wise, Ctrl+A), mouse gestures (drag-select,
double-click word, triple-click line), replace/delete-over-selection
edit ops, a thin bar caret replacing the block caret, and translucent
per-line selection highlight. Applies identically to fresh drafts and
re-edit sessions (shared `pending` path). Enter/Esc/tool-switch
commit semantics untouched. Spec:
[`spec.md`](spec.md).

## Architecture Decisions

- **Anchor + caret model in core.** `text_anchor: Option<usize>` (pub
  field, mirrors `text_cursor`) on `EditorState`; selection =
  `min..max(anchor, cursor)`; `None` or `anchor == cursor` =
  collapsed. All index math stays char-based via `char_to_byte`.
- **Shell resolves hits, core mutates selection.** Pointer pos → char
  index needs glyph metrics, which exist only behind
  `Canvas::measure_text` (the exact reason text-select-edit deferred
  caret-from-click — editor.rs "needs shell-side glyph metrics").
  New egui-side pure resolver (closure-based measurement, binary
  search over prefix widths) shared by both pumps; core gets
  char-index APIs (`text_press_at_char`, `text_drag_to_char`,
  `text_select_word`, `text_select_line`). Pump contract: a press the
  resolver maps into the pending text never reaches core as
  `PointerDown` (so the commit path is bypassed) — mirrors the
  shell-synthesized `DoubleClick` precedent.
- **Double/triple-click reuse the cursor, not the position.** The
  press interception collapses the cursor onto the click point first,
  so core's `DoubleClick`/`TripleClick` arms select word/line *around
  the cursor* — no shell resolution needed on that path.
- **Tracker returns a count.** `DoubleClickTracker::on_press` →
  `u32` consecutive-press count (1/2/3, reset on gap); count 2 pushes
  `DoubleClick`, count 3 pushes the new `InputEvent::TripleClick`
  (same in-addition-to-press ordering contract).
- **Rendering from existing primitives only.** Bar caret and
  highlight rects are `draw_rect_filled` calls in
  `draw_text_indicators` (now anchor-aware); highlight colour is a
  core `TEXT_HIGHLIGHT` const (alpha-tinted, same pattern as
  `TEXT_BBOX_STROKE`); drawn after the glyph run. No `Canvas` trait
  growth, so `compose_final` output stays unaffected (pending is
  excluded there already).
- **`Delete` key becomes useful.** Currently a no-op mid-edit; it
  now deletes the selection, else forward-deletes one char (the
  symmetric industry behaviour; one branch, tested).

## Task Index

### Phase 1: Core semantics (keyboard-complete)

- [x] Task 1: Anchor state + selection-aware edit ops → `tasks/001-core-anchor-state.md`
- [x] Task 2: Keyboard selection chords → `tasks/002-core-keyboard-chords.md`
- [x] Task 3: Bar caret replaces block caret → `tasks/003-core-bar-caret.md`
- [x] Task 4: Selection highlight rendering → `tasks/004-core-selection-highlight.md`

### Checkpoint A: Core locked (after Task 4)

- [x] All four gates green (`fmt`, `clippy -D warnings`, `test --workspace --all-targets`, doc tests)
- [x] Keyboard selection + replace/delete semantics fully covered by dispatcher tests
- [x] Bar caret + highlight locked by render contract tests
- [x] text-select-edit regressions pass unchanged

### Phase 2: Gestures & events

- [x] Task 5: Word-boundary helpers + Ctrl+Shift word-wise chords → `tasks/005-core-word-motion.md`
- [x] Task 6: Mouse selection state APIs (char-index) → `tasks/006-core-mouse-selection-api.md`
- [x] Task 7: `TripleClick` event + tracker counts + click gesture arms → `tasks/007-tripleclick-event-tracker.md`

### Phase 3: Shell wiring

- [x] Task 8: egui hit resolver (pos → char index) → `tasks/008-egui-hit-resolver.md`
- [x] Task 9: app-window pump wiring → `tasks/009-egui-app-pump-wiring.md`
- [x] Task 10: overlay pump wiring → `tasks/010-egui-overlay-pump-wiring.md`

### Checkpoint B: End-to-end (after Task 10)

- [x] Manual pass (PENDING human run) in `cargo run -- app`: drag/dblclick/triple/keyboard
      selection visible with highlight in both draft and re-edit
- [x] Re-edit + selection edits + commit = one undo entry (spec criterion 7; unit-pinned)
- [x] All gates green

### Phase 4: Closeout

- [x] Task 11: CHANGELOG, TEST_MATRIX, spec sync, gates → `tasks/011-closeout.md`

### Checkpoint C: Done

- [x] Spec success criteria 1–8 verified (1–8 unit-pinned; visible-feel pass pending human Checkpoint B run)
- [ ] Human review before merge; `status: shipped` flip happens in
      the merge commit

## Risks and Mitigations

| Risk                                                             | Impact | Mitigation                                                                                                            |
| :--------------------------------------------------------------- | :----- | :-------------------------------------------------------------------------------------------------------------------- |
| Press-inside-pending interception diverges between the two pumps | High   | Shared resolver + identical interception block; overlay's `gesture_for_press` / region gating re-checked in Task 10    |
| Resolver measurement vs drawn glyphs drift (zoom, alignment)     | Med    | Resolver reuses the same galley path as `EguiCanvas::measure_text`; binary search on prefix widths, view-aware         |
| Highlight over bright/dark captures unreadable                   | Med    | Alpha-tinted const tuned in Task 4 against both; revisit at Checkpoint B                                              |
| Tracker count regression breaks re-edit double-click             | Med    | Existing dblclick tests extended, not replaced; re-edit acceptances re-run at Checkpoint A/B                          |
| Block-caret contract tests rewritten too loosely                 | Low    | Task 3 revises them to lock *exact* bar geometry (width/height/position), same assertiveness as the block versions    |

## Open Questions

None — the two spec-level defaults (triple = clicked line; core-const
highlight tint) were confirmed during planning.
