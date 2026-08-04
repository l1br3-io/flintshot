# Task 001: Pin primitive — `EguiShell::run_pin` + `flintshot pin <path>`

**Description:** Build the pin primitive end-to-end. `EditorShell::run_pin(image, rect)` is already declared (`flintshot-core/src/shell.rs:44`) but stubbed (`EguiShell::run_pin` at `crates/flintshot-editor-egui/src/shell.rs:41` returns `Err(FlintshotError::NotImplemented(...))`). The `flintshot pin <path>` CLI subcommand exists (`cli.rs:322`) but only validates the path exists. Fill both in: a borderless always-on-top eframe window that blits the image 1:1, lets the user drag the OS window by clicking anywhere, and closes on `Esc` / window-close. This is the foundation — the editor Pin button (Task 003) and `PinLast` over IPC (Task 004) both route through `flintshot pin <path>`.

egui 0.35 (the workspace version) has the full API surface: `ViewportBuilder::with_always_on_top()` → `WindowLevel::AlwaysOnTop`, `with_decorations(false)`, `with_position(Pos2)`, `with_resizable(bool)`; `ViewportCommand::StartDrag` moves the OS window with the left mouse button until released (the borderless-drag primitive). See `egui-0.35.0/src/viewport.rs` (`ViewportCommand` at `:1080`, `StartDrag` at `:1103`).

**Acceptance criteria:**
- [ ] New module `crates/flintshot-editor-egui/src/pin_window.rs` with a `PinApp` implementing `eframe::App`. It owns the `image::RgbaImage`, a cached `EguiTexture` (uploaded once, reused every frame — same pattern as `EditorApp`'s `bg` cache at `app.rs:58`), and the outcome slot pattern is **not** needed (run_pin returns `Ok(())` on close, not an `EditorOutcome`).
- [ ] `PinApp` blits the image at `(0,0)` covering the full inner size every frame via `EguiCanvas::draw_image` (or a direct `Painter::image`) — no annotations, no toolbar, no input state machine. Just the blit.
- [ ] `PinApp::logic` (egui 0.35 split — see `EditorApp::logic` at `app.rs:510` for the pattern): on `Esc`, send `ViewportCommand::Close`. No other key handling.
- [ ] `PinApp::ui`: on a primary-pointer-down + drag that starts on the image area, send `ViewportCommand::StartDrag` so the OS window follows the mouse. The pin has no other pointer interactions.
- [ ] `EguiShell::run_pin` real body (`shell.rs:41`): build a `ViewportBuilder` with `with_title("flintshot — pin")` (WM class hint, not rendered — decorations off), `with_inner_size([img_w, img_h])` (logical px, same convention as `run_editor_blocking` at `shell.rs:67`), `with_decorations(false)`, `with_always_on_top()`, `with_resizable(true)`, `with_position(rect.min.x, rect.min.y)` when `rect` is non-default. Run `eframe::run_native` blocking until close; return `Ok(())`.
- [ ] `flintshot-app::cli::run_pin` real body (`cli.rs:322`): load the image from `args.path` via `image::open(...)?.to_rgba8()` (the `image` crate is already a workspace dep). Build a default `Rect` (origin `(0.0, 0.0)`, dims from the image) — see [Dependencies / spec Q2](#dependencies). Call `EguiShell::new().run_pin(image, rect)`. Gate on the `egui` feature with a `#[cfg(not(feature = "egui"))]` fallback mirroring `run_editor_for_capture`'s (`cli.rs:286`).
- [ ] `flintshot pin --help` no longer says "not implemented yet (Phase 3)" — the doc comment on the `Pin` variant at `cli.rs:46-47` is updated.
- [ ] The `#[cfg(not(feature = "egui"))]` path returns a clear error ("pin requires the `egui` feature; rebuild with default features").
- [ ] The pin window opens at the image's exact pixel dimensions (no scaling, no letterboxing) — `pixels_per_point` keeps the blit 1:1 with the source texture, matching the editor's HiDPI handling.

**Verification:**
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-targets` — incl. new `pin_window` unit tests (construction doesn't panic; viewport builder carries `decorations(false)` + `always_on_top`; second `update` doesn't re-upload the texture).
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] `cargo build -p flintshot-cosmic` — the opt-in crate still builds (it's untouched, but the gate confirms no workspace-level breakage).
- [ ] Manual (needs display): `cargo run -- pin <some.png>` → borderless always-on-top window appears at the image size; drag from anywhere moves it; `Esc` closes it; the process exits 0. Run on X11 first; note Wayland behaviour for Task 005.

**Dependencies:** None (foundation task).

**Files likely touched:**
- `crates/flintshot-editor-egui/src/pin_window.rs` — **NEW**. `PinApp` (`impl eframe::App`): owns image + cached texture, blits each frame, drag-to-move, Esc-to-close.
- `crates/flintshot-editor-egui/src/shell.rs` — `EguiShell::run_pin` real body (replace stub at `:41-44`). New `run_pin_blocking` helper mirroring `run_editor_blocking` (`:54`).
- `crates/flintshot-editor-egui/src/lib.rs` — `mod pin_window;` and `pub use pin_window::PinApp;` (only if the app crate or tests need it externally; otherwise keep `PinApp` crate-private).
- `crates/flintshot-app/src/cli.rs` — `run_pin` real body (replace stub at `:322-336`); update the `Pin(PinArgs)` doc comment at `:46-47`. Add `#[cfg(not(feature = "egui"))]` fallback.

**Docs:**
- The `run_pin` trait doc at `shell.rs:38-44` currently says "Phase 3: borderless always-on-top viewport." — update to describe the real behaviour and the `rect` semantics (spec Q2: honoured when caller has a real screen position; both Phase 3 callers pass a default).
- ADR-0023 (pin-as-separate-process) — draft during this task; full text lands in Task 005. Record the deferred-viewport-doesn't-work rationale here as a `// RATIONALE:` comment pointing to ADR-0023.
- If `ViewportCommand::StartDrag` misbehaves on a platform during manual verification, record it as a `// GOTCHA:` comment at the call site.

**Estimated scope:** Medium (3-4 files; `pin_window.rs` is the substantive new code, the rest are wiring).

**Notes for the implementer:**
- The pin is a leaf use of egui — no `EditorState`, no `EditorApp`, no `Canvas` trait dispatch (one image, one blit). Resist generalising `EditorApp` to cover both; a separate `PinApp` is clearer and keeps the editor's complexity out of the pin.
- `eframe::run_native` blocks the calling thread until the window closes (same as `run_editor_blocking`). `flintshot pin` is a leaf CLI subcommand; blocking is correct.
- The `image::open` path accepts any format `image` decodes (PNG, JPEG, WebP, etc.) and `to_rgba8()` normalises to the `RgbaImage` the canvas expects. A decode failure surfaces as a `color_eyre` error with the path.
