# Task 005: Resume path + reopen-restore rebuild

**Description:** Getting back in. New `HomeBridge` method
`open_session_capture(&mut self, id) -> bool` (same contract as
`open_image`: `true` = routed, the window swaps to the editor page):
loads the `SessionCapture` from the store, arms a `PendingEditor`
carrying `session_id: Some(id)` (reuse — never re-insert), `false` on
store miss. Reopen-restore: `next_start_after_reopen`'s
`Page::Editor` restore (`app_window.rs:954-958`) rebuilds from the
store when the stashed pending's `session_id` entry still exists —
fresh decode + stored `anns` (undo/redo empty by design) — so an
external capture's close returns the editor **with annotations** (the
`app_window.rs:2062` test family's documented loss flips meaning).
Store miss falls back to today's bare stash.

**Acceptance criteria:**
- [ ] Resume arms the editor with stored `anns` intact, undo/redo empty, same `session_id` (no duplicate entry: `list()` count unchanged)
- [ ] Store-miss resume returns `false` / restore falls back to the bare stashed image — never an error surface
- [ ] The `hidden_close_restores_the_page` test family asserts annotations now survive the hidden-close round-trip when the entry exists
- [ ] Resume from a card while another editor page is open routes through the existing swap machinery (no double epoch)

**Verification:**
- [ ] `cargo test -p flintshot-app --lib` (round-trip: arm → mutate anns → leave → resume → anns equal; reopen-restore rebuild)
- [ ] Full gate commands

**Dependencies:** Task 003 (flush must land before resume can see annotations)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/home.rs` (`HomeBridge` method, default `false`)
- `crates/flintshot-app/src/app_window.rs` (bridge impl, `next_start_after_reopen`, test family)

**Docs:**
- Update the stale "Replace policy's loss" comments where the tests flip

**Estimated scope:** Medium
