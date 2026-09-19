# Task 005: Core — word-boundary helpers + Ctrl+Shift word-wise chords

**Description:** Add free functions beside the existing cursor helpers in editor.rs: `word_left(text, char_idx) -> usize` and `word_right(text, char_idx) -> usize` (skip whitespace, then a maximal run of alphanumeric or non-alphanumeric-non-whitespace chars — the classic editor word model, `char::is_alphanumeric` boundary), plus `word_bounds_around(text, idx) -> (usize, usize)` (the maximal word span containing or adjacent to `idx` — the double-click payload, consumed by Task 007). Wire `Ctrl+Shift+Left/Right` in `dispatch_text_event` to extend the selection word-wise (anchor seeded on first press, cursor to the boundary). Ctrl+Left/Right *without* Shift collapses-and-moves word-wise (industry; cheap once the helpers exist).

**Acceptance criteria:**
- [x] `"hello world"` cursor=8: `word_left` → 6, `word_right` → 11; cursor=5 (the space) → left 0, right 6 (whitespace skipped)
- [x] Punctuation-adjacent: `"foo, bar"` cursor=4 → `word_left` = 3 (stop at the comma boundary class change)
- [x] `word_bounds_around("hello world", 7)` = (6, 11); at a space = adjacent word span; at 0-length text = (0,0)
- [x] Ctrl+Shift+Right ×2 from cursor=0 on "foo bar baz" → anchor=0, cursor=7 (word + space, word)
  > Implementation note: rows "cursor=5 → right 6" and "×2 → 7" are
  > mutually inconsistent — no single word model satisfies both. The
  > classic next-word-start model (word run + trailing whitespace per
  > press, matching row 1 and the "word + space, word" parenthetical
  > exactly) gives ×2 → 8 (start of "baz"); implemented as 8.
- [x] Ctrl+Right (no Shift) collapses and jumps word-wise; Ctrl+Shift+Left extends backwards
- [x] Plain arrow arms unchanged; existing tests pass

**Verification:**
- [x] `cargo test -p flintshot-core`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`

**Dependencies:** Task 001 (anchor model), Task 002 (chord arm structure)

**Files likely touched:**
- `crates/flintshot-core/src/editor.rs`

**Docs:**
- Word-model doc comments (why the two-class boundary rule); boundary tables live in tests.

**Estimated scope:** Small: 1 file
