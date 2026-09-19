# Task 005: Core — restyle API + gesture bracketing

**Description:** Add the gesture-bracketed restyle API to
`EditorState`:

- `begin_style_gesture()` — stage a pre-gesture `anns` snapshot
  (first-wins: a second begin inside one gesture is a no-op; no-op
  entirely while a re-edit session is active).
- `restyle_selected_text(style)` — write the full `TextStyle` field
  set onto the target (the session's `pending` draft when a session
  is active, else `anns[selected_index]`, which must be a Text) and
  **always** sync the session-default fields
  (`font_size`, `text_color`, `font_family`, `font_weight`,
  `font_style`, `text_align`, `line_spacing`, `text_outline`,
  `text_background`). Never pushes history itself.
- `end_style_gesture()` — push the staged snapshot onto `undo`,
  clear `redo`, drop the staging. No-op when nothing is staged.

Mid-session restyles absorb into the session's single commit entry
(the text is absent from `anns`, so an independent snapshot would
undo-delete it — see spec Decisions). Anchor `pos` is never touched
by restyle; the bbox grows with font size.

**Acceptance criteria:**

- [ ] Selected-not-editing: one gesture (any number of `restyle`
      calls between begin/end) restyles the annotation, syncs
      defaults (the next placed text inherits them), and pushes
      **exactly one** undo entry; `Ctrl+Z` restores the prior
      style.
- [ ] Mid-session: restyle retargets the draft, stages nothing;
      session commit still pushes exactly one entry covering
      content + style.
- [ ] No selection (and no session): all three calls are no-ops.

**Verification:**

- [ ] `cargo test -p flintshot-core` (gesture semantics, defaults
      sync, mid-session absorption, no-op guards)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`

**Dependencies:** Task 003

**Files likely touched:**

- `crates/flintshot-core/src/editor.rs` (API + tests)

**Docs:**

- Doc comments must spell out the begin/end pairing contract and
  the mid-session absorption invariant.

**Estimated scope:** Small–Medium (1 file, test-heavy)
