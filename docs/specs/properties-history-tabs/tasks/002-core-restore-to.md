# Task 002: Core — `restore_to` time-travel mutation

**Description:** Add
`EditorState::restore_to(&mut self, target_undo_len: usize)` to
`crates/flintshot-core/src/editor.rs`: clamp the target to
`undo.len() + redo.len()`, then loop `self.undo()` /
`self.redo()` until `undo.len() == target`. By delegating to the
existing swaps it inherits their mid-session guards
(`restore_active_reedit`, `end_move`, pending-drop,
`last_edit_kind` clearing) for free, so time-travel can never
diverge from chord-stepping.

**Acceptance criteria:**
- [ ] Jump back N entries in one call (`undo.len()` shrinks, redo
      grows, labels mirror per Task 001)
- [ ] Jump forward M entries through the redo side
- [ ] Out-of-range targets clamp (both directions); equal target is
      a no-op
- [ ] Empty stacks: any target is a safe no-op
- [ ] Round trip: back to `k`, then forward to the end reproduces
      the pre-jump `anns` / `image` / `next_number` exactly
- [ ] Jump mid-gesture (in-flight move or text session) restores
      the same state an equivalent number of `Ctrl+Z` presses
      would (uses the same guard path)

**Verification:**
- [ ] `cargo test -p flintshot-core`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo fmt --all --check`

**Dependencies:** Task 001 (mirrored labels make round trips
assertable; the loop itself only needs the swap functions)

**Files likely touched:**
- `crates/flintshot-core/src/editor.rs` (impl + inline tests)

**Docs:**
- Doc comment on `restore_to` states the clamp contract and the
  "reuses undo/redo swaps" invariant (the why).

**Estimated scope:** Small: 1 file
