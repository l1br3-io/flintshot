# Task 2: Pin window end-to-end — `run_pin` real impl + `PinApp`

**Description:** Implement the actual pin window. Replace
`EguiShell::run_pin`'s `FlintshotError::NotImplemented` stub
(`crates/flintshot-editor-egui/src/shell.rs:42`) with a real borderless,
always-on-top, transparent eframe viewport that blits the image 1:1 and
implements all five UX features the spec confirmed: (1) always-on-top +
drag-to-move, (2) close on Esc / double-click, (3) hover-opacity
(instant on enter/leave), (4) aspect-locked resize handle, (5)
copy-on-close (gated on `PinConfig.copy_on_close`). Add a new
`PinApp` (`eframe::App`) in `crates/flintshot-editor-egui/src/pin.rs`,
mirroring how `EditorApp` was structured in Phase 1 Task 5. This is the
de-risk slice for pin: the window lifecycle, the viewport flags, and
the input handling all land here.

`run_pin` takes `(image: RgbaImage, rect: Rect)` where `rect.min` is
the window position and `rect.max - rect.min` is the size (both physical
px, per plan Q6). The whole image is shown scaled to that size.

**Acceptance criteria:**
- [ ] `EguiShell::run_pin` opens an eframe `ViewportBuilder` with
      `.with_decorations(false)`, `.with_always_on_top()`,
      `.with_transparent(true)`, `.with_resizable(true)`,
      `.with_inner_size(logical)`, `.with_position(position)`; blocks
      until the window closes (mirror `run_editor_blocking` at
      `shell.rs:54`).
- [ ] New `crates/flintshot-editor-egui/src/pin.rs` with `PinApp`
      implementing `eframe::App`: uploads the image as a texture once
      (cache like the editor's bg cache), blits it each frame filling
      the window, and handles the five UX features below.
- [ ] **Always-on-top + drag:** the window floats above others
      (viewport flag). Dragging anywhere on the image moves the window
      (track a pointer-down + drag; move via `ViewportCommand::OuterPosition`
      or egui's drag, whichever is reliable).
- [ ] **Close on Esc / double-click:** `Esc` or a double-click anywhere
      on the image ends the event loop (publish a close signal like
      `EditorApp`'s outcome slot).
- [ ] **Hover-opacity:** when no pointer is over the window, the image
      is drawn at `PinConfig.default_opacity`; on `PointerButton`/hover
      it snaps to fully opaque. Instant transitions.
- [ ] **Resize handle:** a corner (e.g. bottom-right) drag handle scales
      the window, aspect-ratio locked (image proportions preserved).
      Uses `ViewportCommand::InnerSize` to resize.
- [ ] **Copy-on-close:** when `PinConfig.copy_on_close` is true, the
      image is placed on the clipboard via `arboard` as the window
      closes. The `arboard::Clipboard` handle is kept alive until the
      event loop drains (mirror the editor Copy fix at `app.rs:86` —
      X11's clipboard is owner-based).
- [ ] `flintshot pin` CLI flags from Task 1 (`--opacity`, `--no-clipboard`)
      flow through to the `PinApp` and override `PinConfig` defaults.
- [ ] No `egui::Painter` calls bypass the image blit path for *content*
      (the image is drawn via the texture; hover-opacity is a paint
      alpha, not a re-render — acceptable since a pin shows one image,
      no annotations, so ADR-0012's "route through render()" is
      satisfied by the single blit).

**Verification:**
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-targets` — incl. a `PinApp`
      construction test (given image + rect + `PinConfig`, builds
      without panic) and a close-logic test (Esc/double-click set the
      close flag).
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual (Linux): `cargo run -- pin <png>` → borderless
      always-on-top window appears; drag it; hover (opaque) / unhover
      (translucent); Esc closes. `cargo run -- pin <png> --opacity 0.7`
      → translucent at 0.7. With `copy_on_close = true` in config,
      close → paste elsewhere yields the image. Resize handle scales
      without distortion.
- [ ] Manual (macOS or Windows, if available): same checks; confirm
      always-on-top + borderless work (the cross-platform risk).

**Dependencies:** Task 1 (needs `PinConfig` + the CLI plumbing that
calls `run_pin`).

**Files likely touched:**
- `crates/flintshot-editor-egui/src/shell.rs` — real `run_pin` (replace stub at `:41`); add `run_pin_blocking` helper mirroring `run_editor_blocking`.
- `crates/flintshot-editor-egui/src/pin.rs` — NEW: `PinApp` (`eframe::App`), the five UX features.
- `crates/flintshot-editor-egui/src/lib.rs` — `pub use pin::PinApp;` (if public) or keep internal.
- `crates/flintshot-app/src/cli.rs` — pass resolved `PinConfig` + overrides into `run_pin` (may need `EguiShell::run_pin_with_config` or a small struct arg; keep the trait signature `(image, rect)` and pass config via a side channel or a builder).

**Docs:**
- If eframe's `always_on_top` / `transparent` differs per-OS in a
  non-obvious way, a `// GOTCHA:` comment at the `ViewportBuilder`
  site. Otherwise no separate doc.

**Estimated scope:** Large (3–4 files; `PinApp` with 5 UX features is the substantive work). If it exceeds one session, split hover-opacity + resize + copy-on-close into a Task 2b — but try to land as one cohesive window.
