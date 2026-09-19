# Plan: Properties | History tabs

## Overview

Make the docked editor panel a two-tab surface (PROPERTIES |
HISTORY). Core first: every undo snapshot gains an `EditKind`
label and a `restore_to(target_undo_len)` mutation enables
one-call time-travel. Then the shell: a segmented-chip tab header
in `panel::show`, with the History tab rendering a Photoshop-style
timeline (oldest first, current-state marker, dimmed clickable
future rows). The Properties tab stays byte-identical; floating
undo/redo chips and key chords are untouched. Spec:
[`spec.md`](spec.md).

## Architecture Decisions

1. **Labels live on `Snapshot` as an `EditKind` enum, not strings.**
   Copy-safe, exhaustive-match-checked, `#[non_exhaustive]`; new
   `flintshot-core/src/history.rs` module owns the enum + `Display`.
   `PropertyEditKind` (inspector) is referenced, not duplicated.
2. **`restore_to` reuses `undo()` / `redo()` swaps in a loop.**
   Time-travel and chord-stepping share one code path so they can
   never diverge; the mid-session guards (`restore_active_reedit`,
   `end_move`) are idempotent under repetition.
3. **Undo/redo label mirroring.** When `undo()` builds the redo
   entry, it inherits the popped entry's kind — the same edit
   viewed from the other side. Same in `redo()`.
4. **Tab state is per-window `PanelState`, not persisted.** Matches
   the collapse flag's contract; default Properties.
5. **Timeline order: oldest-first-top with a plain "Current"
   marker row** (planning-time open questions resolved by
   proposal; forward edits are named by their dimmed redo rows).
6. **The panel has no footer to preserve** — `show()` renders
   title + sections only (the module doc's "bottom cluster" text
   is stale; fixing it is Task 5). The spec was amended.

## Task Index

### Phase 1: Core history model
- [x] Task 1: `EditKind` + labeled snapshots at every push site → `tasks/001-core-edit-kind.md`
- [x] Task 2: `restore_to` time-travel mutation → `tasks/002-core-restore-to.md`

### Checkpoint: Core
- [x] `cargo test --workspace --all-targets` + doc tests pass
- [x] No UI behavior change (kinds are inert metadata)

### Phase 2: Shell tabs
- [x] Task 3: `PROPERTIES | HISTORY` tab header → `tasks/003-shell-tab-header.md`
- [x] Task 4: History timeline + click-to-jump → `tasks/004-shell-timeline.md`

### Checkpoint: Shell
- [ ] Manual: `cargo run -- app` → annotate, crop, recolor, then
      click around the timeline; chips + chords unaffected

### Phase 3: Polish
- [x] Task 5: stale module docs, CHANGELOG, full gate → `tasks/005-polish-gate.md`

### Checkpoint: Complete
- [x] All spec success criteria met
  (criteria 3 / 4 manual pass flagged for the user — automated
  coverage: `clicking_rows_time_travels_and_chords_reflect_live`,
  `restore_to_jumps_back_and_forward_round_trip`)
- [x] `fmt` / `clippy -D warnings` / full test set green

## Risks and Mitigations

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| `restore_to` diverges from chord semantics (mid-drag, mid-text-session) | High | Loop over existing `undo()`/`redo()`; add tests for jump-mid-gesture |
| Timeline overflows the 236 px panel after many edits | Med | Wrap the timeline body in `egui::ScrollArea::vertical` |
| Snapshot struct literals in tests (~10 sites) break the field add | Low | Mechanical `EditKind::Generic`; compile-driven |
| egui row clicks need settle passes before hit-testing | Med | Follow the `draw` + `click_events` + `read_response` harness idiom in `panel.rs` tests |
| `commit_annotation`'s tool label needs the owning tool | Low | `owning_tool(&ann)` already exists (editor.rs) |

## Open Questions

None.
