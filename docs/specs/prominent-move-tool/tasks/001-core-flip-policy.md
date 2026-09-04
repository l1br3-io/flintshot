# Task 001: Core flip policy + Move default + config field

**Description:** Land the dispatcher-level policy: committing an
annotation switches the active tool to Move when (a) the policy
flag is on, (b) the committing tool owns the pending annotation,
and (c) the tool is an eligible draw tool. Add the
`UiConfig::flip_to_move` config field (default `true`) and the
matching `EditorState.flip_to_move` policy bool (default `true`).
Flip `Tool`'s `#[default]` from `Rect` to `Move`. After this task
alone, the flip is already live in **both** shells (they dispatch
through core; the field defaults on) — a complete vertical path.

**Acceptance criteria:**

- [ ] `EditorState` gains `pub flip_to_move: bool` (default `true`
      in `new()`); dispatcher consults it in `commit_annotation`
- [ ] Flip fires on commit for Rect / Ellipse / Line / Arrow /
      Marker / Pencil / Blur (pointer-up) and Text (modal commit)
      when `flip_to_move` is on
- [ ] **Ownership guard**: flip only when the pending annotation's
      owning tool equals the active tool. Specifically: a Text
      draft flushed because the user already switched to another
      tool (the `app.rs` toolbar-switch flush calls
      `commit_text_draft` after `state.tool` changed) must NOT
      flip — the explicit tool choice wins
- [ ] `flip_to_move = false` → tools stay sticky across commits
- [ ] Number placements never flip (`place_number` bypasses
      `commit_annotation`); Select / Move never flip
- [ ] Degenerate commits (cancelled zero-extent strokes) do not
      flip (cancel path returns before the commit)
- [ ] A flip adds no undo entry; undo after a flip restores the
      annotation and leaves the tool on Move
- [ ] `Tool::default() == Tool::Move`; doc comment on the enum
      updated (Rect rationale is now stale)
- [ ] `UiConfig.flip_to_move` (`[ui] flip-to-move`) with
      `#[serde(default = "default_flip_to_move")]` → `true`,
      `Default` impl entry, and a `merge_into_existing` /
      `set_value` entry so saves persist it
- [ ] Editor-window default-tool test updated:
      `plain_editor_keeps_default_tool_and_no_phase` (app.rs) now
      expects `Tool::Move`; audit any other test relying on the
      `Rect` default
- [ ] `persist_ui_config` round-trip: a user-set
      `flip-to-move = false` survives an editor session that
      persists last-used UI values (loaded-then-saved config keeps
      the field)

**Verification:**

- [ ] `cargo test -p flintshot-core` — new flip-policy tests (the
      case matrix above, modeled on the existing dispatcher tests)
      plus config tests: default-when-absent, `false` roundtrip,
      `merge_into_existing` preserves a comment on the key
- [ ] `cargo test -p flintshot-editor-egui` — default-tool test
      green
- [ ] Manual: `cargo run -- region` → press `R`, draw, release →
      rail shows Move selected; drag repositions; `Ctrl+Z` restores
      position with tool still Move

**Dependencies:** None

**Files likely touched:**

- `crates/flintshot-core/src/editor.rs` (policy field, flip in
  `commit_annotation`, owning-tool helper, `Tool` default, tests)
- `crates/flintshot-core/src/config.rs` (field, default fn,
  `Default` impl, `merge_into_existing`, tests)
- `crates/flintshot-editor-egui/src/app.rs` (default-tool test
  expectation only)

**Docs:**

- The ownership-guard rationale belongs in a code comment at the
  flip site (it is the "why" for a non-obvious condition); ADR
  lands in Task 005.

**Estimated scope:** Medium: 3 files (2 source + 1 test edit)
