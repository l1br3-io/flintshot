# Task 006: Core — mouse selection state APIs (char-index)

**Description:** Core's half of the mouse path — public, char-index-based selection mutators the pumps will call after shell-side hit resolution (Task 008): `text_press_at_char(idx)` (collapse: cursor=idx, anchor=None — this IS the click-to-reposition-caret inside the draft), `text_drag_to_char(idx)` (cursor=idx, seeding anchor from the press if None — press-then-drag selects; no-op when no anchor was seeded, i.e. drag without an in-text press), `text_select_word()` (word bounds around cursor — Task 005 helper), `text_select_line()` (line bounds around cursor via `cursor_line_start`/`cursor_line_end`). All no-op (or total no-op) when not editing text; all clamp idx to `0..=len`. These are pure state APIs — no events, no measurement, no geometry.

**Acceptance criteria:**
- [ ] `text_press_at_char(3)` on "hello" (cursor anywhere) → cursor=3, anchor=None; idx clamps (`10` → 5, `usize::MAX` → 5)
- [ ] `text_drag_to_char(4)` after press at 1 → anchor=1, cursor=4; drag back to 1 → anchor=1, cursor=1 (collapsed); drag again to 2 → anchor=1, cursor=2 (anchor sticky)
- [ ] `text_drag_to_char` with anchor=None → no-op (a stray drag after the caret-placing press never selects)
- [ ] `text_select_word()` on "hello world" cursor=7 → anchor=6, cursor=11; `text_select_line()` on 2-line text cursor on line 1 → anchor=line-1 start, cursor=line-1 end
- [ ] Every API no-ops cleanly when `pending` is not a Text draft; none panic on empty text
- [ ] APIs don't touch `anns`, `selected`, or undo history (state-machine only — undo bracketing untouched)

**Verification:**
- [ ] `cargo test -p flintshot-core`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`

**Dependencies:** Task 001, Task 005 (`word_bounds_around`)

**Files likely touched:**
- `crates/flintshot-core/src/editor.rs`

**Docs:**
- Doc comments name the pump contract on each API (who calls it, when); the anchor-seeding subtlety gets an inline comment.

**Estimated scope:** Small: 1 file
