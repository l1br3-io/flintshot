# Task 007: Core+egui — TripleClick event + tracker counts + click gesture arms

**Description:** The gesture plumbing. Core: new `InputEvent::TripleClick { pos }` variant (doc-commented with the same in-addition-to-press ordering contract as `DoubleClick`, count-3 consecutive press); `dispatch_text_event` gains arms — `DoubleClick` while editing → `text_select_word()` (word around the cursor, which the intercepted press just collapsed onto the click point), `TripleClick` while editing → `text_select_line()`; both no-op when the draft is empty. The non-modal `DoubleClick` re-edit arm is untouched; `TripleClick` outside editing is a documented no-op. egui: `DoubleClickTracker::on_press` returns `u32` consecutive-press count (resets to 1 past 3 or on time/distance gap); shells push `DoubleClick` on 2, `TripleClick` on 3 — but NOT during an intercepted in-text press ordering change: the tracker is fed on every dispatched *or* intercepted press so consecutive in-text clicks count.

**Acceptance criteria:**
- [ ] Tracker: press,press,press → 1,2,3; fourth → 1; slow/distant pair → 1
- [ ] Core: DoubleClick while editing with text → word selection; with empty draft → no-op (no panic)
- [ ] Core: TripleClick while editing → clicked-line selection; TripleClick not editing → `Continue`, no state change
- [ ] Re-edit flow intact: double-click a committed text still lifts it out for editing (existing tests + the ordering contract preserved)
- [ ] `InputEvent` remains `#[non_exhaustive]`-compatible: no exhaustive-match breakage in shells (compile proves it)
- [ ] Existing dblclick tests revised to counts, same scenarios green

**Verification:**
- [ ] `cargo test -p flintshot-core` and `cargo test -p flintshot-editor-egui`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`

**Dependencies:** Task 005, Task 006

**Files likely touched:**
- `crates/flintshot-core/src/input.rs`
- `crates/flintshot-core/src/editor.rs` (two dispatch arms)
- `crates/flintshot-editor-egui/src/dblclick.rs`
- `crates/flintshot-editor-egui/src/app.rs`, `overlay.rs` (tracker call sites — count plumbing only; full gesture wiring is Tasks 9/10)

**Docs:**
- `TripleClick` variant doc carries the emission contract; tracker doc updated for counts.

**Estimated scope:** Medium: 3–5 files
