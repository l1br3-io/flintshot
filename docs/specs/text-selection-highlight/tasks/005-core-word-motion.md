# Task 005: Core — word-boundary helpers + Ctrl+Shift word-wise chords

**Description:** Add free functions beside the existing cursor helpers in editor.rs: `word_left(text, char_idx) -> usize` and `word_right(text, char_idx) -> usize` (skip whitespace, then a maximal run of alphanumeric or non-alphanumeric-non-whitespace chars — the classic editor word model, `char::is_alphanumeric` boundary), plus `word_bounds_around(text, idx) -> (usize, usize)` (the maximal word span containing or adjacent to `idx` — the double-click payload, consumed by Task 007). Wire `Ctrl+Shift+Left/Right` in `dispatch_text_event` to extend the selection word-wise (anchor seeded on first press, cursor to the boundary). Ctrl+Left/Right *without* Shift collapses-and-moves word-wise (industry; cheap once the helpers exist).

**Acceptance criteria:**
- [ ] `"hello world"` cursor=8: `word_left` → 6, `word_right` → 11; cursor=5 (the space) → left 0, right 6 (whitespace skipped)
- [ ] Punctuation-adjacent: `"foo, bar"` cursor=4 → `word_left` = 3 (stop at the comma boundary class change)
- [ ] `word_bounds_around("hello world", 7)` = (6, 11); at a space = adjacent word span; at 0-length text = (0,0)
- [ ] Ctrl+Shift+Right ×2 from cursor=0 on "foo bar baz" → anchor=0, cursor=7 (word + space, word)
- [ ] Ctrl+Right (no Shift) collapses and jumps word-wise; Ctrl+Shift+Left extends backwards
- [ ] Plain arrow arms unchanged; existing tests pass

**Verification:**
- [ ] `cargo test -p flintshot-core`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`

**Dependencies:** Task 001 (anchor model), Task 002 (chord arm structure)

**Files likely touched:**
- `crates/flintshot-core/src/editor.rs`

**Docs:**
- Word-model doc comments (why the two-class boundary rule); boundary tables live in tests.

**Estimated scope:** Small: 1 file
