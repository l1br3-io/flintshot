# Task 004: Shell — History timeline + click-to-jump

**Description:** Replace Task 003's placeholder with the timeline
body in `panel::show`'s History tab (`panel.rs`). Layout:
`undo` entries oldest-first (`undo[0]` is the oldest — iterate in
index order), then the current-state marker row (ink dot +
"Current", not clickable), then dimmed future rows from `redo`
iterated `rev()` (nearest future first). Row labels come from
`EditKind`'s `Display`. Clicking past row `i` calls
`state.restore_to(i + 1)`; clicking future row `j` (0 = nearest)
calls `state.restore_to(undo.len() + j + 1)`. The body wraps in
`egui::ScrollArea::vertical` so long histories scroll instead of
overflowing the 236 px panel. Empty state (both stacks empty):
a quiet hint line ("No edits yet") in the Crop hint styling.

**Acceptance criteria:**
- [ ] A scripted mixed session (commit ×2, number, move, delete,
      crop, property edit) renders one labeled row per entry in
      order, oldest first, with the marker at the boundary
- [ ] Clicking a past row restores that state in one action
      (`undo.len()` after the click pass equals `i + 1`); clicking
      a dimmed future row redoes forward
- [ ] Future rows render dimmed (half-alpha ink) but stay clickable
- [ ] `Ctrl+Z` / floating chips still work and the timeline
      reflects them live (immediate-mode redraw)
- [ ] Empty state renders the hint; no rows, no marker
- [ ] Long histories scroll; the marker stays reachable

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui`
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo fmt --all --check`
- [ ] Manual: full spec success story — draw, numbers, crop,
      recolor, click around the timeline, chips + chords live

**Dependencies:** Tasks 001, 002 (labels + `restore_to`),
Task 003 (the tab to render inside)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/panel.rs` (timeline render +
  click handling + tests via the `draw` / `click_events` harness)

**Docs:**
- Panel module doc's History-tab description finalized here (not
  Task 003's placeholder text).

**Estimated scope:** Medium: 1 file, dense
