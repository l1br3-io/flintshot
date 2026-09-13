# Task 001: Remove Pin from editor + overlay toolbars

**Description:** Strip the Pin button and its handler from both
editor surfaces — the editor window (`app.rs`) and the live overlay
(`overlay.rs`) — including the shared toolbar action
(`toolbar.rs`). `EditorOutcome::Pinned` production ends here; the
variant itself stays in core until task 003.

**Acceptance criteria:**
- [ ] `ToolbarAction::Pin` variant, "Pin to desktop" buttons (editor
      rail + overlay chip `overlay_pin`), and `g_pin` glyph gone from
      `toolbar.rs`
- [ ] `handle_pin` removed from both `app.rs` and `overlay.rs`
- [ ] `compose_and_write_pin` + its tests deleted from `app.rs`
      (`compose_and_write_upload` stays)
- [ ] `spawn_pin_worker` deleted; the shared detached-spawn helper it
      wraps stays for `spawn_upload_worker`
- [ ] Overlay imports (`compose_and_write_pin`, `spawn_pin_worker`)
      and doc-comment mentions ("Save / Pin / Upload" lists) rewritten
      without Pin
- [ ] `EditorOutcome::Pinned` no longer constructed anywhere

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui` green
- [ ] `cargo clippy -p flintshot-editor-egui --all-targets -- -D warnings` green
- [ ] `rg -n "ToolbarAction::Pin|handle_pin|compose_and_write_pin|spawn_pin_worker" crates/flintshot-editor-egui/src/` empty

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-editor-egui/src/toolbar.rs`
- `crates/flintshot-editor-egui/src/app.rs`
- `crates/flintshot-editor-egui/src/overlay.rs`

**Docs:**
- Gotchas → inline `/* GOTCHA */` comment if the shared spawn helper
  has non-obvious constraints after the split.

**Estimated scope:** Medium (3 files, heavy test edits in app.rs)
