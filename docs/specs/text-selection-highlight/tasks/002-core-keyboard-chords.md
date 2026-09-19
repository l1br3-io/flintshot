# Task 002: Core — keyboard selection chords (Shift+arrows, Shift+Home/End, Ctrl+A, Delete)

**Description:** Wire selection creation via the keyboard in `dispatch_text_event`: Shift+Left/Right extend by char, Shift+Up/Down by line, Shift+Home/End to the line edges (first Shift press seeds the anchor at the current cursor; repeated presses move only the cursor — standard anchor semantics). `Ctrl+A` (`Key::Char('a')` + `ctrl_only()`) selects the whole draft. `Key::Delete` deletes the selection when non-empty, else forward-deletes the char after the cursor (currently a dead key mid-edit). Existing plain-key arms gain the collapse behaviour from Task 001 by matching modifier-free presses exactly (`modifiers == default`), so Shift+X never falls into the plain arms.

**Acceptance criteria:**
- [ ] Shift+Left ×3 from cursor=5 (anchor None) → anchor=5, cursor=2; further Shift+Right ×1 → cursor=3, anchor still 5
- [ ] Plain Left with a live selection collapses (anchor=None) and moves
- [ ] Shift+Up / Shift+Down on multi-line text seed the anchor and move the cursor line-wise (via `cursor_up`/`cursor_down` on the cursor only)
- [ ] Shift+Home / Shift+End extend to line start / end (anchor seeded)
- [ ] Ctrl+A → anchor=0, cursor=len; Ctrl+A on empty draft is a no-op (anchor stays None)
- [ ] Delete with anchor=0/cursor=5 on "hello" → ""; Delete collapsed at cursor=1 → "hllo" (forward delete); Backspace/Enter/Esc/Shift+Enter chords behave exactly as before when modifiers match none of the new arms
- [ ] Every existing text-modal dispatcher test passes unchanged (spec: fresh-draft behaviour byte-for-byte)

**Verification:**
- [ ] `cargo test -p flintshot-core`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: `cargo run -- app`, place text, Shift+arrows / Ctrl+A — selection not yet visible (rendering lands Task 004); verify via statusbar/panel if trivially observable, else rely on tests

**Dependencies:** Task 001

**Files likely touched:**
- `crates/flintshot-core/src/editor.rs` (dispatch arms, `delete_text` forward-delete helper)

**Docs:**
- Doc comment on the new Delete helper; inline comment on the modifier-exactness gotcha (Shift+Enter vs Enter precedent).

**Estimated scope:** Small: 1 file
