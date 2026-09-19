# Task 009: egui — app-window pump wiring (press/drag/dblclick/triple)

**Description:** Wire the editor-window pump (`app.rs` pointer section) to the new machinery while editing text: when `state.is_editing_text()` and the press resolves inside the pending text (Task 008 resolver via ctx+view), the pump calls `text_press_at_char(idx)` INSTEAD of pushing `PointerDown` (the commit path is bypassed — this is the press-interception contract), feeds the tracker, and marks the drag as in-text so subsequent `PointerDrag`s call `text_drag_to_char(resolved)` instead of dispatching; the release just clears the in-text flag (selection persists). Presses resolving outside behave exactly as today. DoubleClick (count 2) and TripleClick (count 3) fire after the intercepted press exactly like the dispatched case.

**Acceptance criteria:**
- [ ] Type "hello", press between 'l' and 'l', release → caret moved there (anchor None); no commit, no new draft
- [ ] Press + drag across "hello" → selection grows live (highlight from Task 004); release keeps it; type 'X' → replaces selection
- [ ] Press inside text, drag OUTSIDE the block → clamps to the boundary char (resolver edge), selection stable
- [ ] Press outside the text → commits + fresh caret / new action exactly as before (byte-for-byte existing behavior; existing app tests prove it)
- [ ] Double-click inside text → word selected; triple-click → line; double-click on a *committed* text (not editing) still re-edits
- [ ] Toolbar/chip presses, Hand pan, region gating untouched — existing app pump tests green
- [ ] While editing with tool switched via toolbar, the draft still flushes (commit_text_draft path) — no interception happens post-flush

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui` (extend the synthetic-event app tests around app.rs:2700 with in-text press/drag sequences)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: `cargo run -- app` — full gesture pass: drag/dblclick/triple/keyboard selection with visible highlight; drag-select in a re-edit session too

**Dependencies:** Tasks 006, 007, 008

**Files likely touched:**
- `crates/flintshot-editor-egui/src/app.rs`
- `crates/flintshot-editor-egui/src/texthit.rs` (real measure closure constructor, if it lives here rather than per-pump)

**Docs:**
- Inline comment at the interception block naming the contract (no PointerDown for in-text presses) and why (commit bypass).

**Estimated scope:** Medium: 1–2 files
