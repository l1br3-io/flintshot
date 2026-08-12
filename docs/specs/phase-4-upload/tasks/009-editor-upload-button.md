# Task 009: Editor Upload button

**Description:** Add an **Upload** button to the editor toolbar
(`flintshot-editor-egui/src/app.rs`). Mirrors the existing Pin button:
compose the final image (selection-aware via `compose_final` /
`compose_final_region`), write it to a temp PNG, spawn `flintshot upload
<tmpfile>` as a detached child (resolving the binary via `current_exe`),
publish `EditorOutcome::Uploaded(url)` and close the editor.

The spawned worker reports the URL back to the daemon (Task 008), so
subscribers see `Event::Uploaded`; the editor itself closes immediately
after spawn confirmation (the URL arrives asynchronously via the worker;
the editor outcome is `Uploaded` to signal "upload was initiated").

**Acceptance criteria:**
- [ ] `ToolbarAction::Upload` variant added
- [ ] Upload button rendered in the toolbar (next to Pin)
- [ ] `handle_upload` composes the image, writes a temp PNG, spawns
      `flintshot upload <tmpfile>`, publishes `Uploaded` outcome, closes
      editor
- [ ] Compose is selection-aware (mirrors Pin/Copy selection logic)
- [ ] Spawn failure is recoverable (editor stays open, logged at
      `error`) — mirrors `handle_pin` error policy
- [ ] Compose/temp-write failure is terminal (publish `Err`, close) —
      mirrors `handle_pin`
- [ ] `run_editor_for_capture` in `cli.rs` handles
      `EditorOutcome::Uploaded(url)` (already has the arm from Phase 0;
      ensure it logs correctly)
- [ ] Headless test: `compose_and_write_upload` writes a decodable PNG
      (mirrors the pin compose-and-write test)

**Verification:**
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo test --doc -p flintshot-editor-egui`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo fmt --all --check`

**Dependencies:** Task 005 (CLI upload exists to spawn)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/app.rs`

**Estimated scope:** Small-Medium: 1 file (large file, but the changes
are localized — toolbar enum + handler + compose helper)

**Docs:**
- Gotcha: the editor closes immediately after spawn; the URL is not
  available synchronously. `EditorOutcome::Uploaded(String)` carries an
  empty or placeholder string if the worker hasn't reported yet. The
  real URL arrives via `Event::Uploaded` from the daemon. If this
  causes issues, the outcome variant may need reworking — flag in the
  task review.
