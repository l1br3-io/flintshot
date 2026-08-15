# Task 002: `OverlayApp` fullscreen viewport + frozen-capture render + dispatcher wiring + Esc/Enter

**Description:** The overlay shell MVP (plan decision 1). New
`flintshot-editor-egui::overlay` module with `OverlayApp`, a new
eframe app (not a mode on `EditorApp`): a fullscreen, undecorated
viewport (the Phase 3 pin window's `ViewportBuilder` is the
precedent) that renders the frozen capture through the unchanged
shared `render()` + `EguiCanvas` path. Pointer and key events
translate to `InputEvent`s exactly like `EditorApp::logic` — no
second dispatcher (plan decision 5): the literal-duplicate
translation helpers (`translate_modifiers`, `tool_for_key`, the
text-input drain) are factored out of `app.rs` for reuse. The
overlay-specific deltas: it opens with `Tool::Select` (first drag
is a region selection), `Esc` closes as `Cancelled`, and `Enter` is
intercepted *before* dispatch (the dispatcher's Enter path crops via
`confirm_selection` — wrong for the overlay, where the selection
defines the output region). The overlay opens in selection mode;
single-key tool switching (S R E A L M P T B N) and Ctrl+Z/Y undo
chords work via the dispatcher from day one. The Enter=Copy /
Ctrl+S / Ctrl+C pipelines are Task 006: for now an accepted close
publishes `Cancelled` with a notice (same placeholder pattern Task
001 used for the missing shell). `EguiShell` grows a concrete
`run_overlay(state)` method (trait addition explicitly not required
per spec §Boundaries), and `run_capture_shell` in `cli.rs` replaces
the Task 001 fallback branch with the real dispatch.

**Acceptance criteria:**
- [x] `overlay` module (`crates/flintshot-editor-egui/src/overlay.rs`)
      with `OverlayApp` (`impl eframe::App`): owns the
      `EditorState`, `EguiCanvas`, bg texture cache, `BlurCache`,
      and the shared `OutcomeSlot`.
- [x] Viewport builder: `.with_fullscreen(true)` +
      `.with_decorations(false)`, title `"flintshot — overlay"`
      (WM-class convention). Unit-tested by inspecting the builder
      fields.
- [x] `ui()`: `CentralPanel` with `Frame::NONE` (no margin — the
      image must land at exact screen position 0,0), painter-bound
      shared `render()` call; panel rect cached for pointer
      hit-testing; render failure publishes `Err` + closes.
- [x] `logic()`: text-editing modal branch first (factored
      `dispatch_text_input`); then Esc → close-cancel / Enter →
      close-accept (`OverlayClose::{Cancel, Accept}`, pure
      decision fn, unit-tested); then dispatcher key dispatch
      (Ctrl+Z/Y) + single-key tool switching; then pointer
      dispatch with screen-logical → canvas-physical translation
      (pure fn, unit-tested) and the press-began-inside-canvas
      drag guard.
- [x] Enter while a text draft is active commits the draft
      (dispatcher), it does not close the overlay.
- [x] Accepted close publishes `Ok(EditorOutcome::Cancelled)` with
      an `info!` notice that the Enter→Copy output flow lands in
      Task 006; cancelled close publishes `Ok(Cancelled)`; X-close
      defaults to `Cancelled` via the empty slot.
- [x] `EguiShell::run_overlay(&self, state) ->
      Result<EditorOutcome>` concrete method (not on the
      `EditorShell` trait); blocks like `run_editor`/`run_pin`.
- [x] `cli.rs`: `run_capture_shell` dispatches resolved
      `Overlay` → overlay shell, `Editor` → editor window
      (Task 001's notice-and-fallback branch removed). State
      construction (config UI defaults) factored into
      `state_from_config`, shared by both paths; outcome logging
      factored and shared.
- [x] Editor-window mode untouched: no behavior change to
      `run_editor` paths beyond the factored helpers.

**Verification:**
- [x] `cargo fmt --all --check`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo test --workspace --all-targets` — incl. new overlay
      unit tests (close decisions, viewport flags, pointer
      translation, tool default) + `state_from_config` CLI test.
- [x] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual (X11, `$DISPLAY`): `cargo run -- region --overlay`
      freezes the screen, drag draws a selection, Esc exits clean
      (code 0, no file / clipboard / worker). **Blocked in the dev
      shell**: this session's Xwayland (`:1`, hybrid GPU) hangs in
      wgpu device creation for *both* the overlay and the untouched
      editor-window path, so the interactive checkpoint could not be
      driven headlessly (XTEST Esc script was prepared and the
      process bounded with `timeout`; nothing lingered). Run once
      from the real desktop session.

**Dependencies:** Task 001 (mode resolution + the fallback branch
this task replaces).

**Files likely touched:**
- `crates/flintshot-editor-egui/src/overlay.rs` — new module.
- `crates/flintshot-editor-egui/src/app.rs` — factor out
  `translate_modifiers` / `tool_for_key` / `dispatch_text_input`
  as `pub(crate)` reuse points.
- `crates/flintshot-editor-egui/src/shell.rs` — `run_overlay`
  blocking entry + `EguiShell` method.
- `crates/flintshot-editor-egui/src/lib.rs` — module list + docs.
- `crates/flintshot-app/src/cli.rs` — overlay dispatch branch +
  factored `state_from_config` / outcome logging.
- `CHANGELOG.md` — Unreleased entry.

**Estimated scope:** Medium (1 new module ~300 lines, 3 files
touched for wiring/reuse, no core changes).
