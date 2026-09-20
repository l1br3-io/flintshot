# Task 004: Save graduation + per-commit annotation flush

**Description:** The removal and crash-safety paths. In
`PageBridge::report_outcome` (`app_window.rs:1799`), `Saved(path)`
additionally removes the active session entry (it graduated to
recents — the existing `recents::record` call stays). `Uploaded`,
`CopiedToClipboard`, and `Cancelled` deliberately keep the entry (spec
lifecycle table). Per-commit flush: new default-no-op
`EditorPageBridge::annotations_committed(&mut self, _anns: &[Annotation])`
— the impl resolves the target via the tracked active session — called
from the egui editor's commit sites (wherever `state.anns` mutates:
tool commit, delete, move, crop confirm, history-driven restores). The
app impl rewrites the JSON via `SessionStore::update_anns`.

**Acceptance criteria:**
- [ ] Saving from the editor removes the session entry (test: arm → save outcome → `list()` empty)
- [ ] Upload/copy/Esc keep the entry (test pins the distinction)
- [ ] Each committed annotation change rewrites `<id>.json` within the same frame (test double on the bridge records calls; store round-trip asserts content)
- [ ] Default no-op keeps all existing test doubles compiling (trait-default pattern per `HomeBridge` precedent)

**Verification:**
- [ ] `cargo test -p flintshot-app --lib` + `cargo test -p flintshot-editor-egui --lib`
- [ ] Full gate commands

**Dependencies:** Task 003 (needs `active_session` + id plumbing)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/shell.rs` (`EditorPageBridge` method)
- `crates/flintshot-editor-egui/src/app.rs` / `canvas.rs` (commit call sites)
- `crates/flintshot-app/src/app_window.rs` (`PageBridge` impl)

**Docs:**
- None beyond doc comments on the new trait method

**Estimated scope:** Medium
