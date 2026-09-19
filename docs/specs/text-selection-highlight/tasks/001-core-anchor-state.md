# Task 001: Core — anchor state + selection-aware edit ops

**Description:** Introduce the selection model: a `text_anchor: Option<usize>` pub field on `EditorState` (char index, mirrors `text_cursor`; `None` = collapsed). Add the range helper (`selection_range() -> Option<Range<usize>>`), make every plain caret move clear the anchor (collapse rule), teach the edit ops to consume a non-empty selection (`type_char` / `insert_newline` / `backspace_text` replace the range; `Delete` handled in Task 2 with the chords), and clear the anchor on commit / cancel / re-edit open so no stale anchor survives a session boundary.

**Acceptance criteria:**
- [ ] `text_anchor` defaults `None`; `EditorState::new` and every commit/cancel/re-edit-open path leaves it `None`
- [ ] `selection_range()` returns `Some(min..max)` only when `anchor != cursor` and a Text draft is pending; `None` otherwise (incl. no draft)
- [ ] Plain `cursor_left/right/up/down/home/end` and `type_char` after a selection leave `text_anchor == None`
- [ ] With anchor=0 cursor=5 on "hello": `type_char('X')` → "X", cursor=1; `insert_newline` → "\n"; `backspace_text` → "" with cursor=0 — each in one stroke
- [ ] Backspace with collapsed selection behaves exactly as today (delete char before caret, column-0 no-op)
- [ ] Multi-byte: selection edges set at emoji boundaries replace/delete whole chars, never panic or split

**Verification:**
- [ ] `cargo test -p flintshot-core` — new dispatcher tests + all existing text-modal tests pass unchanged
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-core/src/editor.rs` (field, helper, edit ops, collapse rules, session-boundary clears)

**Docs:**
- Field + helper doc comments (missing_docs gate). Gotchas → inline comments at the collapse-rule sites.

**Estimated scope:** Small: 1 file
