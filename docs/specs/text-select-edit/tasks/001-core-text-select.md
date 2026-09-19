# Task 001: Core — Text-tool click-to-select state machine

**Description:** With `Tool::Text` active, a `PointerDown` that
hit-tests a committed `Annotation::Text` selects it
(`selected_index = Some(i)`, no draft started) instead of beginning
a new caret on top of it. A miss — or a hit on a non-Text
annotation — clears any selection and starts a fresh draft exactly
as today. Leaving the Text tool via `set_tool` clears the selection
(mirroring the Move rule). `Del` / `Backspace` then delete the
selected text through the existing `delete_selected` path.

**Acceptance criteria:**

- [x] Text-tool press on a committed Text: `selected_index` set,
      `pending` stays `None`. Press on empty canvas or a non-Text
      annotation: selection cleared, empty Text draft begins
      (current behavior preserved).
- [x] `set_tool` away from `Tool::Text` clears a Text-tool
      selection; staying on Text never clears. `undo` / `redo` /
      `crop` still clear it (existing generic paths, pinned by
      tests).
- [x] `Del` with a Text-tool selection removes exactly that
      annotation with one undo entry; `Ctrl+Z` restores it.

**Verification:**

- [x] `cargo test -p flintshot-core` (new dispatcher tests +
      existing suite green)
- [x] `cargo clippy --workspace --all-targets -- -D warnings`

**Dependencies:** None

**Files likely touched:**

- `crates/flintshot-core/src/editor.rs` (dispatcher PointerDown
  branch, `set_tool`, tests)

**Docs:**

- None expected; dispatcher branch comments carry the why (Text
  selects before it drafts).

**Estimated scope:** Small (1 file)
