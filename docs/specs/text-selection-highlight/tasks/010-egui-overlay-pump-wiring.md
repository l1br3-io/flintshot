# Task 010: egui — overlay pump wiring (press/drag/dblclick/triple)

**Description:** Mirror Task 009 in the fullscreen overlay pump (`overlay.rs`), respecting its extra gating: the interception slot sits where `annotation_press_allowed` currently admits the press — an in-text press must also survive `gesture_for_press` (region handles/edges win, as they do today) and the outside-region swallow (Task 005 of native-capture-ux). `drag_active` bookkeeping must cover the intercepted press so its drags/release route to the selection path, not the dispatcher's stale `_` arm. Same contract: in-text press never pushes `PointerDown`; drags resolve via Task 008; count-2/3 clicks fire word/line selection.

**Acceptance criteria:**
- [ ] Same gesture matrix as Task 009 (press-move-caret, drag-select, dblclick word, triple line, outside-press commits) inside the overlay
- [ ] A press on a region handle/edge still starts the resize/move gesture even when it overlaps the pending text (gesture precedence preserved)
- [ ] A press outside the crop region is still swallowed entirely (dimmed-area contract, native-capture-ux Task 005 regression)
- [ ] `drag_active` never leaks: release after an in-text drag leaves `gesture == None`, `drag_active == false`
- [ ] Re-edit flow in overlay: double-click committed text lifts out; selection gestures work during the re-edit session
- [ ] Existing overlay pump tests green; new synthetic tests cover the intercepted-press path

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: `cargo run -- capture` → editor overlay path (or the overlay route from the app), full gesture pass

**Dependencies:** Task 009 (established interception pattern; can start in parallel after 008 but verify both together)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/overlay.rs`
- `crates/flintshot-editor-egui/src/texthit.rs` (shared closure, adjustments)

**Docs:**
- Inline comments at the gating interactions (gesture precedence, region swallow) — the "why" is non-obvious there.

**Estimated scope:** Medium: 1–2 files
