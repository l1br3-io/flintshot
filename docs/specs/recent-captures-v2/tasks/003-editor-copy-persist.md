# Task 003: Editor Copy persists + save-dialog prefill

**Description:** Wire `save-clipboard-copies` into the editor's Copy
button (`EditorApp::handle_copy`): after a successful clipboard write,
when the flag is on, persist the composed image via core's
`clipboard_copy_path` + `save_image`, then record it into recents at the
configured limit. A persist failure never fails the copy — the clipboard
write already succeeded: log + status/toast, move on. Prefill the plain
Save dialog (`prompt_png_path`) with the resolved save directory
(`resolve_save_dir`), so dialog-first saves land somewhere sane too.

**Acceptance criteria:**
- [ ] With `save-clipboard-copies = true`, a Copy writes a file to the resolved dir with template-or-builtin naming and collision suffixing, and records it into `recents.toml`
- [ ] With `false`, Copy writes no file (tempdir probe) and still succeeds
- [ ] A persist failure (unwritable dir) leaves the copy reported as successful; the failure is logged, not published as an error outcome
- [ ] `prompt_png_path` composes with the resolved directory (pure path-composition seam tested; the dialog itself is manual)

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui` (RED first)
- [ ] fmt + clippy gates; `cargo test --workspace --all-targets` for regressions

**Dependencies:** Tasks 001, 002

**Files likely touched:**
- `crates/flintshot-editor-egui/src/app.rs` (`handle_copy`, `prompt_png_path`, `handle_save` prefill)
- `crates/flintshot-editor-egui/src/overlay.rs` only if its Copy shares the seam cheaply (optional; overlay copies are out of spec scope — skip unless trivial)

**Docs:**
- None

**Estimated scope:** Medium
