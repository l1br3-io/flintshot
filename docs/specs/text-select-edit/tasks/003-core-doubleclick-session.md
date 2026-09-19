# Task 003: Core — DoubleClick + lift-out re-edit session

**Description:** Add `InputEvent::DoubleClick { pos }` and the
lift-out re-edit session. A `DoubleClick` (normal, non-modal path)
under `Tool::Text` that hit-tests a committed Text lifts it out of
`anns` into `pending` — the entire existing modal path (cursor ops,
key suppression, `is_editing_text`) then applies unchanged. A
session record `{ index, original }` supports exact-once history:
commit splices the edited text back at `index` and pushes one
snapshot containing the original; Esc reverts to the original with
no history touch. `DoubleClick` is a no-op in every other context
(non-Text tool, miss, or while already editing — the accompanying
`PointerDown` already did the normal work).

Key mechanics:

- `begin`: `anns.remove(index)` → `original`; `pending = clone`;
  `text_cursor` = char count (caret at end); `selected_index =
  None` for the session's duration (the slot is vacant in `anns`;
  pointing `delete_selected` at it would remove the wrong
  annotation). Restore `selected_index = Some(index)` on
  commit/revert.
- `commit_text_draft` (the public flush API shells already call on
  toolbar switches) gains the session branch: unchanged → splice
  back silently; changed → push `Snapshot::anns_only` of `anns`
  with `original` spliced at `index`, clear `redo`, splice edited
  back; edited-to-empty → remove entirely (fresh drafts never
  commit empty) + same single snapshot.
- Esc in the modal path: session → revert splice; fresh draft →
  existing `cancel_annotation`.
- Defensive guards: `crop` / `undo` / `redo` restore `original`
  first when a session is somehow active (shell-direct calls bypass
  the modal dispatcher); guards drop the session and selection.

**Acceptance criteria:**

- [ ] Session lifecycle: begin leaves `anns` without the text,
      `pending` holding it, caret at end; click-away commits then
      starts the new action (existing Flameshot flow preserved).
- [ ] Edit + Enter (or click-away): edited text back at its index,
      **exactly one** undo entry; `Ctrl+Z` restores original content
      *and* style. Esc: original restored, zero entries. No-op edit
      (open + commit without changes): zero entries.
- [ ] Edited-to-empty + commit: annotation removed, one entry,
      undo restores. `DoubleClick` no-ops under non-Text tools, on
      miss, and mid-edit.
- [ ] Direct `crop` / `undo` / `redo` calls mid-session restore the
      original safely (no lost text, no wrong-index splice).

**Verification:**

- [ ] `cargo test -p flintshot-core` (session lifecycle, undo
      exactness, guards, DoubleClick routing)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`

**Dependencies:** Task 001

**Files likely touched:**

- `crates/flintshot-core/src/input.rs` (variant + docs)
- `crates/flintshot-core/src/editor.rs` (session record, dispatch,
  `commit_text_draft`, guards, tests)

**Docs:**

- `InputEvent` variant doc must state the emission contract: shells
  emit it *in addition to* the second press's `PointerDown`, in the
  same frame, after it.

**Estimated scope:** Medium (2 files, test-heavy)
