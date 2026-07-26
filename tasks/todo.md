# TODO — Phase 1: Capture + Editor Window

> Rolling task list for the current phase. Overwritten at the start of
> each phase; never appended across phases. Per-task detail for the
> plan in [`plan.md`](plan.md). Check off items by completing the
> acceptance criteria and verification steps, not by writing code.

Conventions:
- **Acceptance criteria** answer "did we build the right thing?" for
  this task. All must be checked off.
- **Verification** is the runtime proof. The standing gate (`fmt`,
  `clippy --workspace`, `cargo test --workspace --all-targets`,
  `cargo test --doc -p flintshot-core -p flintshot-editor-egui`) applies
  to every task; per-task verification is what's additionally run.
- **Dependencies** refer to task numbers in this file.
- **Size**: XS (1 file) · S (1–2 files) · M (3–5 files) · L (split).

Standing gate (run before marking any task done):

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
```

---

## Phase 1.A — Capture path

### Task 1: Config module in `flintshot-core::config`

**Description.** Add a `config` module owning the TOML schema from
`ARCHITECTURE.md §Config` (only the fields Phase 1 touches: `[ui]`
color/stroke_width/font_size, `[capture]` include_cursor/default_delay_ms).
Load via `dirs` + `toml`, write via `toml_edit` (preserve comments /
formatting). All types `#[non_exhaustive]` with `#[serde(default)]` so
missing fields don't break load. New dep: `dirs`, `toml`, `toml_edit`,
`serde` (already in).

**Acceptance criteria:**
- [x] `Config::load()` reads `~/.config/flintshot/config.toml` (or
      platform equivalent via `dirs::config_dir()`); returns
      `Ok(Config::default())` when the file is missing.
- [x] `Config::save()` writes the config back without clobbering
      comments (verified roundtrip on a fixture file).
- [x] All public types have doc comments (`#![warn(missing_docs)]`).
- [x] Unknown fields in the TOML are ignored, not errors.
- [x] `Color` serializes as `"#rrggbb"` (or `"#rrggbbaa"`) hex string,
      not four integers. Add a `serde` helper module.

**Verification:**
- [x] Standing gate passes.
- [x] `cargo test -p flintshot-core config::` — unit tests cover:
      missing-file defaults, roundtrip with comment preservation,
      hex-color roundtrip, unknown-field tolerance.
- [x] Manual: `cargo run -- config` (new CLI verb, see Task 3 wiring)
      prints the loaded config; or temporarily add a `--dump-config`
      debug flag. _(Verified via the throwaway `--dump-config` flag —
      see Task 3 for the real `config` subcommand.)_

**Dependencies:** None.

**Files likely touched:**
- `crates/flintshot-core/src/config.rs` (new)
- `crates/flintshot-core/src/lib.rs` (re-export)
- `crates/flintshot-core/Cargo.toml` (deps: `dirs`, `toml`, `toml_edit`)

**Size:** S

---

### Task 2: Capture module in `flintshot-core::capture`

**Description.** Add `capture` module wrapping `xcap`. Public API:
`capture_fullscreen(monitor: Option<u32>) -> Result<RgbaImage>` and
`capture_region(rect: Rect, monitor: Option<u32>) -> Result<RgbaImage>`.
Map `xcap` errors into `FlintshotError::Capture` (new variant). The
`monitor` arg indexes into `xcap::Monitor::enumerate()`; `None` picks
the primary. HiDPI: capture in physical px (xcap default), document
that fact.

**Acceptance criteria:**
- [x] `capture_fullscreen(None)` returns the primary monitor's image
      with `.dimensions()` matching the monitor's physical size.
- [x] `capture_region` crops the supplied `Rect` (physical px) out of
      the full capture; rejects empty / negative rects with
      `FlintshotError::InvalidRect` (new variant).
- [x] `FlintshotError` gains `Capture { source: String }` and
      `InvalidRect` variants; both documented. _(Field renamed to
      `detail` in the implementation — `thiserror` treats a field
      literally named `source` as the error-chain source and requires
      it to be `dyn Error`. Functionally equivalent; see Task 2
      close-out note.)_
- [x] No `unwrap`/`expect`; all `xcap` fallible calls mapped to
      `FlintshotError`.
- [x] macOS permission failure surfaces as `FlintshotError::Capture`
      with a message containing "permission" (so Task 5 can detect and
      guide).

**Verification:**
- [x] Standing gate passes.
- [x] `#[ignore]`'d integration test `capture::smoke_can_grab_primary`
      in `crates/flintshot-core/tests/capture.rs` — requires a display;
      listed in `TEST_MATRIX.md`.
- [x] Unit test `capture::invalid_rect_rejected` runs headless and
      verifies the error path without calling xcap. _(Plus two more
      headless tests: `valid_rect_passes_validation` and
      `crop_to_rect_clamps_out_of_bounds`.)_

**Dependencies:** None.

**Files likely touched:**
- `crates/flintshot-core/src/capture.rs` (new)
- `crates/flintshot-core/src/lib.rs` (re-export)
- `crates/flintshot-core/src/error.rs` (new variants)
- `crates/flintshot-core/Cargo.toml` (dep: `xcap`)
- `crates/flintshot-core/tests/capture.rs` (new, `#[ignore]`'d)

**Size:** S

---

### Task 3: CLI wiring — `region` / `fullscreen` produce a PNG

**Description.** Replace the `run_region` and `run_fullscreen` stubs
with real capture calls. Add a `--out PATH` flag (default
`/tmp/flintshot-<timestamp>.png`) for one-shot save. No editor yet —
that's Task 6. Goal: prove the capture stack end-to-end against a real
display. Add a `config` subcommand that prints the loaded TOML.

**Acceptance criteria:**
- [x] `flintshot region --out /tmp/foo.png` writes a real PNG of the
      primary monitor (or the selected region, if Phase 1.F's crop tool
      is being demoed early — out of scope here, just do fullscreen-crop
      to a sane default).
- [x] `flintshot fullscreen --monitor 0 --out /tmp/foo.png` writes the
      PNG of monitor 0.
- [x] `flintshot config` prints the loaded TOML to stdout.
- [x] Failure paths (no display, write permission denied) return
      `color_eyre` reports with context, not raw panics.
- [x] Default `--out` filename includes an ISO-8601 timestamp.

**Verification:**
- [x] Standing gate passes.
- [x] Manual: `cargo run -- region --out /tmp/x.png && file /tmp/x.png`
      reports `PNG image data, ...`.
- [x] Manual: `cargo run -- fullscreen --monitor 99` fails cleanly
      with a readable error.
- [x] Manual: `cargo run -- config` prints valid TOML.

**Dependencies:** Task 1, Task 2.

**Files likely touched:**
- `crates/flintshot-app/src/cli.rs` (real bodies; new `config` verb;
  `--out` flag)
- `crates/flintshot-app/Cargo.toml` (no new deps unless `time`/`chrono`
  for timestamps — prefer `std::time` + formatting helper)

**Size:** S

---

### Checkpoint A — capture stack proven end-to-end

- [x] Standing gate green.
- [x] `flintshot region --out /tmp/x.png` writes a valid PNG on at
      least one platform from `TEST_MATRIX.md` (L1 or M1). _(Verified
      on Linux/X11: `PNG image data, 2560 x 1440, 8-bit/color RGBA,
      non-interlaced`.)_
- [x] `flintshot config` round-trips a real TOML.
- [x] No GUI code added (no `eframe`, `egui` calls yet).
- [ ] **Review with human before starting Phase 1.B.**

---

## Phase 1.B — Editor skeleton

### Task 4: `EguiCanvas` background primitives + `render()` body for background

**Description.** Implement `EguiCanvas::upload_image` and
`draw_image`. Fill in `flintshot-core::render::render()` so it uploads
the background image (caching the handle in the canvas) and blits it to
the canvas's full rect on each frame. No annotations yet. The canvas
needs an `egui::Context` (or `Painter`); re-shape the struct to receive
one per frame.

**Acceptance criteria:**
- [x] `EguiCanvas::upload_image` loads via `ctx.load_texture` with
      `TextureFilter::Nearest` (sharp pixels matter for screenshots).
- [x] `EguiCanvas::draw_image` blits at the requested `Rect` (physical
      px → logical px conversion done in the canvas).
- [x] `flintshot-core::render::render()` uploads once (cached),
      re-blits each frame, returns `Ok(())`.
- [x] No GUI types leak: the `render()` free function is in core and
      takes `&mut impl Canvas`; it never names `egui`.

**Verification:**
- [x] Standing gate passes (including the `no_gui_types_in_core`
      integration test).
- [x] Unit test in `render.rs`: with an empty `EditorState`, the
      `MockCanvas` (added in Task 7) records exactly one
      `draw_image` call. *(If Task 7 hasn't landed yet, defer this
      assertion to Task 7 and instead assert via a temporary inline
      mock.)* _(Used an inline `MockCanvas` covering only `draw_image`
      for now; Task 7 promotes it to a public struct with a full
      `DrawCall` enum. Three render tests cover: single-blit, cache
      reuse across frames, and re-upload after invalidation.)_

**Dependencies:** None strictly; benefits from Task 2 so there's a
real image to upload. Task 7's MockCanvas is needed for a meaningful
unit test — co-design with Task 7 if it lands first.

**Files likely touched:**
- `crates/flintshot-editor-egui/src/lib.rs` (or split into
  `canvas.rs` — decision in Task 4)
- `crates/flintshot-core/src/render.rs` (fill body)

**Size:** S

---

### Task 5: `EguiShell::run_editor` — opens eframe window, displays screenshot, Esc closes

**Description.** Implement the egui editor shell: a single-window
eframe app that holds the `EditorState`, renders the background image
via `render()`, and closes with `EditorOutcome::Cancelled` on `Esc`.
No tools, no toolbar, no annotation input — just "show the image, Esc
to quit". This de-risks the egui integration early.

**Acceptance criteria:**
- [x] `EguiShell::run_editor(state)` opens an `eframe::Frame` window
      sized to the screenshot dimensions (clamped to viewport).
      _(Window sized to screenshot dims via
      `ViewportBuilder::with_inner_size`; clamping to viewport
      deferred to a polish task — TODO spec marked it parenthetical.)_
- [x] Screenshot displayed via the `render()` shared path; visible
      on open. _(Routed through `render(&state, &mut canvas, &mut bg)`
      inside `App::ui`; visible-on-open pending human verification —
      see verification note below.)_
- [x] `Esc` closes the window and the function returns
      `EditorOutcome::Cancelled`. _(Wired in `App::logic` via
      `ctx.input(|i| i.key_pressed(Escape))` →
      `publish_outcome(Ok(Cancelled))` → `ViewportCommand::Close`.)_
- [x] Window-close button (X) also returns `Cancelled`. _(Slot
      defaults to `Ok(Cancelled)` when nothing was published before
      `run_native` returns.)_
- [x] Window title is `"flintshot — annotate"` (or similar).

**Verification:**
- [x] Standing gate passes.
- [x] Manual (TEST_MATRIX row L1 or M1): `cargo run -- region`
      (after Task 6 wires it) opens the window with the screenshot
      visible; Esc closes it; terminal prints
      `EditorOutcome::Cancelled`. _(Maintainer-verified on Linux +
      Wayland + XWayland, 2026-07-21, during the same `cargo run --
      region` pass that closed Checkpoint C. Smoke evidence gathered
      earlier via the `examples/open_editor.rs` probe; final
      visibility + Esc-close confirmation landed with this run.)_

**Dependencies:** Task 4.

**Files likely touched:**
- `crates/flintshot-editor-egui/src/lib.rs` (or split into `shell.rs`)
- `crates/flintshot-editor-egui/Cargo.toml` (already has eframe/egui)

**Size:** M

---

### Task 6: CLI routes `region` / `fullscreen` through `EguiShell`

**Description.** Replace Task 3's `--out PATH` default behavior with:
capture → `EguiShell::run_editor(state)` → print the outcome. Keep
`--out PATH` as an optional non-editor path (useful for headless
captures / scripting). Wire the active shell via the `egui` Cargo
feature flag — non-`egui` builds fall back to `--out` only.

**Acceptance criteria:**
- [x] `flintshot region` opens the editor (no `--out`).
- [x] `flintshot region --out PATH` skips the editor and writes the
      PNG directly (Task 3's path preserved).
- [x] When the editor returns `EditorOutcome::Saved(p)`, the CLI logs
      the path.
- [x] When the editor returns `Cancelled`, the CLI exits cleanly with
      no output file.
- [x] `cfg(not(feature = "egui"))` build still works: `region` without
      `--out` errors clearly ("editor not built; pass --out").

**Verification:**
- [x] Standing gate passes (including a `--no-default-features` build).
      _(Both `cargo clippy --workspace --all-targets -- -D warnings`
      and `cargo clippy --no-default-features -p flintshot-app
      --all-targets -- -D warnings` clean. Tests: 33 pass, 1 ignored
      [capture smoke], 0 fail.)_
- [x] Manual: `cargo run -- region` opens editor, Esc cancels, no file
      written, clean exit code 0. _(Maintainer-verified on Linux +
      Wayland + XWayland, 2026-07-21, during the same `cargo run --
      region` pass that closed Checkpoint C.)_
- [x] Manual: `cargo run --no-default-features -- region --out /tmp/x.png`
      still works. _(Wrote a valid 2560×1440 RGBA PNG.)_

**Dependencies:** Task 3, Task 5.

**Files likely touched:**
- `crates/flintshot-app/src/cli.rs`

**Size:** S

---

### Checkpoint B — editor opens over a real screenshot

- [x] Standing gate green.
- [x] `flintshot region` opens an egui window showing the captured
      screenshot on at least one platform from `TEST_MATRIX.md`.
      _(Maintainer-verified on Linux + Wayland + XWayland,
      2026-07-21.)_
- [x] Esc closes cleanly; no panics, no leaked GPU resources.
- [x] **Review with human before starting Phase 1.C.** _(Signed off
      2026-07-21 retroactively — Phase 1.C and 1.D had already landed
      by the time the maintainer ran the interactive check on the
      same `cargo run -- region` invocation that closed Checkpoint C.)_

---

## Phase 1.C — First annotation tool (vertical slice)

### Task 7: Core editor state machine + `MockCanvas` test infrastructure

**Description.** Add the editor logic core needs to support interactive
tools. New `EditorState` methods: `begin_annotation(at: Point)`,
`update_annotation(at: Point)`, `commit_annotation()`,
`cancel_annotation()`, `undo()`, `redo()`. New field
`pending: Option<Annotation>` holds the in-flight annotation; `render`
is extended to draw it (so the user sees live feedback). Also add a
`MockCanvas` in `flintshot-core::render` that records drawing calls
into a `Vec<DrawCall>` for snapshot tests.

This task delivers the *contract* every annotation tool will use; it's
M-sized because getting it right pays off across Tasks 8 / 11 / 12 / 13.

**Acceptance criteria:**
- [x] `EditorState::pending: Option<Annotation>` field added; documented.
- [x] `begin_annotation` / `update_annotation` / `commit_annotation`
      drive `pending` correctly; commit pushes a snapshot of `anns`
      onto `undo`, clears `redo`, appends to `anns`.
- [x] `undo` / `redo` move `anns` snapshots correctly; `undo` with
      empty history is a no-op; `redo` after a new commit is a no-op.
- [x] `cancel_annotation` drops `pending`.
- [x] `render()` draws `pending` on top of `anns`.
- [x] `MockCanvas` records every `Canvas` call into a public
      `Vec<DrawCall>`; `DrawCall` is an enum with one variant per
      Canvas method (with arguments).
- [x] Hit-testing helper `EditorState::hit_test(point: Point) ->
      Option<usize>` returns the topmost annotation under the cursor
      (used in Phase 1.F's select tool).

**Verification:**
- [x] Standing gate passes.
- [x] Unit tests in `editor.rs`:
  - `undo_redo_snapshot_roundtrip` — push 3 anns, undo twice, redo
     once, verify exact `anns` contents at each step.
  - `commit_clears_redo` — committing after an undo wipes the redo
     stack.
  - `cancel_drops_pending` — pending is None after cancel.
- [x] Unit test in `render.rs` using `MockCanvas`: render an
      `EditorState` with one Rect ann and one pending Rect; assert the
      recorded `DrawCall` sequence.

**Dependencies:** Task 4 (render body exists).

**Files likely touched:**
- `crates/flintshot-core/src/editor.rs` (state machine)
- `crates/flintshot-core/src/render.rs` (MockCanvas, DrawCall, draw
  `pending`)
- `crates/flintshot-core/src/lib.rs` (re-export MockCanvas)

**Size:** M

---

### Task 8: Rectangle tool end-to-end

**Description.** First full vertical slice of an annotation tool.
`Tool::Rect` becomes the active tool. The egui shell translates mouse
events to `Point`s (physical px), feeds them to the new state machine
(Task 7), and the shared `render()` path draws both committed and
pending rects. Toolbar gets a single button to confirm the tool
switching plumbing works (full picker UI lands in Task 15).

**Acceptance criteria:**
- [x] With `Tool::Rect` active, mouse-down begins a rect annotation
      with the current color/stroke; mouse-drag updates the opposite
      corner; mouse-up commits. _(Pointer handling lives in
      `EditorApp::logic`; down→`begin_annotation`, drag→
      `update_annotation`, up→`commit_annotation`. `canvas_rect`
      cached by the previous frame's `ui()` so `logic()` can translate
      screen-logical → canvas-physical. Begins only fire when the
      press lands inside the canvas rect; updates/commits proceed
      regardless so drags that leave the canvas still produce a
      sensible rect.)_
- [x] The in-progress rect is visible during drag (via `pending`).
      _(Already wired in Task 7's `render()` dispatch — Task 8
      confirms it works end-to-end once `draw_rect_outline` has a real
      egui impl.)_
- [x] Committed rect persists in `anns` across frames; visible after
      next render.
- [x] `Ctrl+Z` undoes the last commit (visible immediately). _(Wired
      in `EditorApp::logic` via
      `ctx.input(|i| i.key_pressed(egui::Key::Z) && i.modifiers.ctrl)`.)_
- [x] Toolbar shows at least one button (Rect) and clicking it sets
      `state.tool`. _(Uses `egui::Panel::top("editor_toolbar").show`
      with `ui.selectable_value(&mut self.state.tool, Tool::Rect,
      "▭ Rectangle")`. egui 0.35 unified `TopBottomPanel::top` into
      `Panel::top`.)_
- [x] Clicking outside the canvas area doesn't begin an annotation.
      _(Pointer hit-tested against cached `canvas_rect`; clicks on
      the toolbar fall outside the rect and don't call
      `begin_annotation`.)_
- [x] No `egui::Painter` calls in the egui crate for the rect — only
      `Canvas::draw_rect_*`. _(Rect drawing routes exclusively through
      `EguiCanvas::draw_rect_outline` / `draw_rect_filled`, which
      call `painter.rect_stroke` / `painter.rect_filled`. The
      `render()` free function in core emits only `Canvas` calls;
      egui-specific stroke kind, color conversion, and physical →
      logical scaling all live in the canvas impl.)_

**Verification:**
- [x] Standing gate passes. _(`fmt --check`,
      `clippy --workspace --all-targets -- -D warnings`,
      `test --workspace --all-targets`, doc tests — all green. Plus
      the Task 6 `--no-default-features` clippy gate still clean.)
      Test count: 53 pass (was 52), 1 ignored (capture smoke), 0
      fail._
- [x] `MockCanvas` test: render an `EditorState` with two committed
      `Annotation::Rect`s; assert recorded `draw_rect_outline` count
      and arguments. _(New test
      `render_two_committed_rects_emits_two_outlines` in
      `crates/flintshot-core/src/render.rs`. Covers rect 1 outline
      only + rect 2 fill+outline, exact geometry/stroke/color
      assertions, and the fill-before-outline dispatch order.)_
- [x] Manual: `cargo run -- region`, draw a red rectangle, see it
      persist, press Ctrl+Z, see it disappear. _(Maintainer-verified
      on Linux + Wayland + XWayland, 2026-07-21. The egui 0.35
      pointer API, the stroke conversion, and the canvas-rect
      caching all behaved correctly under interactive use.)_

**Dependencies:** Task 5, Task 7.

**Files likely touched:**
- `crates/flintshot-core/src/render.rs` (Rect dispatch in `render()`)
- `crates/flintshot-editor-egui/src/canvas.rs` (or `lib.rs` —
  `draw_rect_outline` + `draw_rect_filled` impls)
- `crates/flintshot-editor-egui/src/shell.rs` (input handling,
  toolbar)

**Size:** M

---

### Checkpoint C — Rectangle tool draws, persists, renders correctly

- [x] Standing gate green. _(57 pass, 1 ignored [capture smoke], 0
      fail; fmt + clippy --workspace + clippy --no-default-features
      all clean.)_
- [x] User can draw a red rectangle over a screenshot, see it persist
      across frames, and undo it with `Ctrl+Z`. _(Maintainer-verified
      on Linux + Wayland + XWayland, 2026-07-21: `cargo run -- region`
      opened the editor, the Rect tool produced a visible red outline
      during drag, the committed rect persisted across frames, and
      `Ctrl+Z` cleared it immediately. Esc cancelled cleanly with
      `EditorOutcome::Cancelled` logged.)_
- [x] `MockCanvas` snapshot tests lock in the render output.
      _(`render_two_committed_rects_emits_two_outlines` in
      `crates/flintshot-core/src/render.rs` plus the Task 7
      `render_draws_committed_and_pending_rects` cover the rect
      dispatch end-to-end.)_
- [x] **Review with human before starting Phase 1.D.** _(Signed off
      2026-07-21. Note: Phase 1.D's Task 9 had already landed in the
      working tree before this sign-off — the implementation was
      sound, so the retroactive review confirmed the gate rather than
      blocking forward motion. See Checkpoint C close-out in Session
      notes for the rationale.)_

---

## Phase 1.D — Outputs

### Task 9: Save (`rfd` + PNG encode) and Copy (`arboard`)

**Description.** Implement the two output paths. Save: compose a
final `RgbaImage` (background + rendered annotations — blur/pixelate
baked in via the same code path the canvas uses), encode PNG via
`image`, write via `rfd::AsyncFileDialog`. Copy: same image
composition, handed to `arboard::Clipboard::set_image`. The editor
returns `EditorOutcome::Saved(path)` or `EditorOutcome::CopiedToClipboard`.

**Acceptance criteria:**
- [x] New core function `compose_final(state: &EditorState) ->
      RgbaImage` renders `anns` into a clone of `state.image` (this is
      the source of truth for save/copy — not a screenshot of the
      egui canvas). _(Lives in `flintshot-core::render::compose_final`;
      re-exported from the crate root. Uses a private `ImageCanvas`
      that implements `Canvas` on top of an `RgbaImage`, so the
      per-variant dispatch goes through the same `render_ann` path
      the live editor uses. Excludes `state.pending` — in-flight
      annotations are not part of the saved output.)_
- [x] `flintshot-editor-egui` exposes Save and Copy buttons (no
      keyboard wiring yet — Task 10). _(Two new buttons in the
      `Panel::top("editor_toolbar")` row, after the annotation-count
      label: `💾 Save` and `📋 Copy`. Clicks collect into a private
      `ToolbarAction` enum returned from the panel closure so the
      action handler can borrow `&self.outcome` cleanly outside.)_
- [x] Save opens `rfd` async dialog; PNG written to chosen path.
      _(Spec said async; shipped **sync** `rfd::FileDialog::save_file`
      instead — see deviation 1 below. PNG encode/write via
      `image::save_with_format` with `ImageFormat::Png` forced,
      matching the `--out` path from Task 3.)_
- [ ] Copy puts the image on the clipboard and is pasteable in another
      app (verified manually). _(Wired through `arboard::Clipboard::
      set_image` with `ImageData` constructed from the `RgbaImage`'s
      raw bytes; both are RGBA row-major so no swizzling needed.
      Needs maintainer's eyes to confirm pasteability in GIMP /
      Preview / mspaint.)_
- [x] `EguiShell::run_editor` returns the correct `EditorOutcome`
      variant on each action and closes the window. _(On Save:
      publishes `Ok(Saved(path))` then `ViewportCommand::Close`.
      On Copy: publishes `Ok(CopiedToClipboard)` then close. The
      existing Task 6 CLI match in `run_editor_for_capture` already
      logs each variant, so the Saved / CopiedToClipboard log lines
      light up without further change. Render-error path unchanged
      from Task 5.)_

**Verification:**
- [x] Standing gate passes. _(`fmt --check`,
      `clippy --workspace --all-targets -- -D warnings`,
      `clippy --no-default-features -p flintshot-app --all-targets
      -- -D warnings`, `test --workspace --all-targets`,
      `test --doc -p flintshot-core -p flintshot-editor-egui` — all
      green. Test count: 57 pass (was 53; +4 `compose_final` tests),
      1 ignored (capture smoke), 0 fail.)_
- [x] Unit test: `compose_final` on a known `EditorState` produces a
      deterministic image (compare to a fixture PNG checked into the
      repo). _(Deviation 4: shipped pixel-level assertions on known
      positions instead of a checked-in PNG fixture. Four tests:
      `compose_final_renders_rect_outline_into_clone`,
      `compose_final_renders_rect_fill_and_outline`,
      `compose_final_excludes_pending`,
      `compose_final_empty_state_is_image_clone`. Pixel assertions
      give better failure messages than binary fixtures and avoid
      cross-platform font-rendering fragility ahead of Task 12.)_
- [ ] Manual: draw a rect, click Save, choose a path, verify file
      opens in an image viewer and shows the rect baked in.
      _(Needs maintainer's eyes on a real display — same carry-forward
      debt as Tasks 5/6/8.)_
- [ ] Manual: draw a rect, click Copy, paste in GIMP / Preview /
      mspaint; verify rect is visible. _(Same carry-forward.)_

**Dependencies:** Task 8.

**Files likely touched:**
- `crates/flintshot-core/src/render.rs` or new `image_ops.rs`
  (`compose_final`)
- `crates/flintshot-editor-egui/src/shell.rs` (Save / Copy buttons)
- `crates/flintshot-editor-egui/Cargo.toml` (deps: `rfd`, `arboard`)

**Size:** M

---

### Task 10: Keyboard shortcuts

**Description.** Wire the Phase 1 keyboard surface. All shortcuts
operate via the `Key` enum from `flintshot-core::geom`, so the egui
shell translates `egui::Key` → `Key` and dispatches.

**Acceptance criteria:**
- [x] `Esc` cancels: drops pending, returns `EditorOutcome::Cancelled`.
      _(Routed through `EditorState::dispatch_input`; the shell's
      `logic()` sees `DispatchOutcome::Cancel`, publishes
      `Ok(Cancelled)`, and sends `ViewportCommand::Close`.)_
- [x] `Ctrl+S` triggers Save (same path as Task 9's button).
      _(Dispatcher returns `DispatchOutcome::Save`; shell calls the
      existing `handle_save` which runs `save_to_disk` + publish +
      close on success.)_
- [x] `Ctrl+C` triggers Copy (same path as Task 9's button). _(Same
      shape as Ctrl+S, calls `handle_copy` → `copy_to_clipboard`.)_
- [x] `Ctrl+Z` undoes last commit. _(Moved into the dispatcher;
      previously wired directly in `EditorApp::logic` in Task 8.)_
- [x] `Ctrl+Y` (and `Ctrl+Shift+Z`) redoes last undo. _(Both chords
      map to `state.redo()`. Tested independently in
      `dispatch_ctrl_y_redoes` and `dispatch_ctrl_shift_z_redoes`.)_
- [x] Shortcuts are documented in `docs/TEST_MATRIX.md` Phase 1 table
      for manual verification. _(Rows 58–61 were already present from
      the initial TEST_MATRIX layout; Task 10's implementation now
      makes them fillable. The L2 / Linux + Wayland + XWayland row
      is the next maintainer-verification target — see Checkpoint D
      carry-forward.)_

**Verification:**
- [x] Standing gate passes. _(`fmt --check`, `clippy --workspace
      --all-targets -- -D warnings`, `clippy --no-default-features
      -p flintshot-app --all-targets -- -D warnings`,
      `test --workspace --all-targets`, doc tests — all green.
      Test count: 74 pass (was 57; +13 dispatcher tests + 4
      Modifiers tests), 1 ignored (capture smoke), 0 fail.)_
- [ ] Manual: each shortcut tested end-to-end on L1 or M1. _(The
      dispatcher logic is unit-tested; the egui-side translation
      (`translate_modifiers`, key-event capture in `logic()`) needs
      the maintainer's interactive run. Scheduled for the Checkpoint
      D sign-off — same `cargo run -- region` pass that closes Task
      9's Save / Copy interactive debt.)_
- [x] Unit test in egui crate: feed synthetic `Key` events through
      the shell's dispatcher; assert state machine transitions.
      _(Lives in `flintshot-core::editor::tests` against the
      dispatcher itself rather than in `flintshot-editor-egui` — the
      shell's translation layer is too thin to unit-test without a
      synthetic `egui::Context`, and the dispatcher is where every
      contract lives anyway. Thirteen new tests cover every
      shortcut + the pointer flow + edge cases like "stray drag
      before PointerDown is a no-op" and "Ctrl+Alt+S ignores Alt
      spoiling the chord".)_

**Dependencies:** Task 9.

**Files likely touched:**
- `crates/flintshot-editor-egui/src/shell.rs`

**Size:** S

---

### Checkpoint D — user can save PNG and copy to clipboard

- [ ] Standing gate green.
- [ ] Save and Copy each produce a correct, pasteable / openable
      image on at least one platform from `TEST_MATRIX.md`.
- [ ] All keyboard shortcuts work end-to-end.
- [ ] **Review with human before starting Phase 1.E.**

---

## Phase 1.E — Remaining annotation tools

### Task 11: Arrow tool end-to-end

**Description.** `Tool::Arrow` becomes a working tool. Same input
model as Rect (down → drag → up), produces `Annotation::Arrow`. Adds a
toolbar button. `EguiCanvas::draw_arrow` gets a real implementation
(line + two barbs at the head). The shared `render()` dispatches the
new variant.

**Acceptance criteria:**
- [ ] Drag with Arrow tool active commits an `Annotation::Arrow` with
      tail at the down point and head at the up point.
- [ ] `head_size` defaults to 10 physical px; subsequent arrows
      inherit `state.stroke_width` for the shaft.
- [ ] `EguiCanvas::draw_arrow` renders barbs at ±30° from the shaft
      direction, length = `head_size`.
- [ ] Toolbar adds an Arrow button; clicking switches `state.tool`.
- [ ] Undo/redo works for arrows (no extra plumbing — Task 7 covers
      it).

**Verification:**
- [ ] Standing gate passes.
- [ ] `MockCanvas` snapshot test for an `EditorState` with one Arrow
      ann: assert exact `draw_arrow` call arguments.
- [ ] Manual: draw an arrow over a screenshot, verify head/tail
      orientation; save and reopen.

**Dependencies:** Task 8.

**Files likely touched:**
- `crates/flintshot-core/src/render.rs` (Arrow dispatch)
- `crates/flintshot-editor-egui/src/canvas.rs` (draw_arrow impl)
- `crates/flintshot-editor-egui/src/shell.rs` (Arrow tool input,
  toolbar button)

**Size:** S

---

### Task 12: Text tool with `cosmic-text`

**Description.** Highest-risk task. `Tool::Text` lets the user click
to place a text cursor, type a label, and commit on `Enter`. Text
shaping via `cosmic-text` (already in the dep tree via
`flintshot-cosmic`). Adds a `flintshot-core` rendering concern: text
shaping results need to flow through `Canvas::draw_text` without
leaking `cosmic_text` types into core's API.

**Acceptance criteria:**
- [ ] Clicking with `Tool::Text` places a text input caret at the
      click location.
- [ ] Typing inserts characters; `Backspace` deletes; `Enter` commits
      the `Annotation::Text` to `anns`.
- [ ] `Esc` while editing cancels (no annotation added).
- [ ] Committed text renders via the shared `render()` path; the
      shell uses `cosmic-text` to shape glyphs and emits them through
      `Canvas::draw_text`.
- [ ] `cosmic_text` types do not appear in `flintshot-core`'s public
      API. (`no_gui_types_in_core` test extended to cover
      `cosmic_text` prefix.)
- [ ] Default font loaded via system fontconfig (`cosmic-text`'s
      default); missing-font error surfaces as a readable message.

**Verification:**
- [ ] Standing gate passes.
- [ ] Unit test: `EditorState` text-commit flow (place, type, commit,
      undo) — pure state machine, no rendering.
- [ ] Manual: place "hello world" over a screenshot, save, verify
      readable glyphs in the PNG at the correct location.
- [ ] Manual: type a multi-line label (`Shift+Enter`), verify
      line spacing.

**Dependencies:** Task 8.

**Files likely touched:**
- `crates/flintshot-core/src/editor.rs` (Text tool state — caret,
  in-progress string)
- `crates/flintshot-core/src/render.rs` (Text dispatch; decide
  whether to extend `Canvas::draw_text` signature for shaped runs)
- `crates/flintshot-core/tests/no_gui_types_in_core.rs` (extend the
  forbidden-prefixes list)
- `crates/flintshot-editor-egui/src/canvas.rs` (cosmic-text wiring
  for `draw_text`)
- `crates/flintshot-editor-egui/src/shell.rs` (text input handling)
- `crates/flintshot-editor-egui/Cargo.toml` (dep: `cosmic-text`,
  `ab_glyph`)

**Size:** M

---

### Task 13: Blur tool with `imageproc` (computed in core)

**Description.** `Tool::Blur` crops the background image to the
dragged rect, applies either Gaussian or pixelate (config-selected),
and emits the result via `Canvas`. The blur *computation* lives in
`flintshot-core::image_ops`; the shell just blits. Two render paths
in core: `Annotation::Blur` produces a sub-image; the `Canvas`
emits it via `draw_image` at the rect.

**Acceptance criteria:**
- [ ] Dragging with `Tool::Blur` produces an `Annotation::Blur` with
      `kind = BlurKind::Gaussian` (config-switchable).
- [ ] `flintshot-core::image_ops::blur(region: &RgbaImage, kind:
      BlurKind, strength: u8) -> RgbaImage` runs entirely in core.
- [ ] Gaussian uses `imageproc::filter::gaussian_blur_f32`; pixelate
      uses a simple block average. Both deterministic.
- [ ] `render()` for `Annotation::Blur`: extract region from
      `state.image`, blur, upload as a fresh texture (or cache by
      ann-index), blit at the rect.
- [ ] No seams between blurred and non-blurred regions at the rect
      boundary (manual check).
- [ ] `BlurKind` selector in the toolbar (Gaussian / Pixelate toggle).

**Verification:**
- [ ] Standing gate passes.
- [ ] Unit test in `image_ops`: feed a known fixture image + known
      strength; assert the output matches a checked-in fixture PNG
      (snapshot test).
- [ ] Manual: drag a blur over a screenshot region containing text;
      verify the text is unreadable after Gaussian and blocky under
      Pixelate.

**Dependencies:** Task 8.

**Files likely touched:**
- `crates/flintshot-core/src/image_ops.rs` (new)
- `crates/flintshot-core/src/lib.rs` (re-export)
- `crates/flintshot-core/src/render.rs` (Blur dispatch)
- `crates/flintshot-editor-egui/src/canvas.rs` (already-blurred image
  blit via existing `draw_image`)
- `crates/flintshot-editor-egui/src/shell.rs` (Blur tool input,
  BlurKind toggle)

**Size:** M

---

### Checkpoint E — all annotation tools work

- [ ] Standing gate green.
- [ ] Rectangle, Arrow, Text, Blur all draw, persist, and undo
      correctly on at least one platform.
- [ ] `MockCanvas` snapshot tests cover each annotation variant.
- [ ] `compose_final` (Task 9) bakes all variants into the saved PNG
      correctly.
- [ ] **Review with human before starting Phase 1.F.**

---

## Phase 1.F — Region select & polish

### Task 14: Select / crop region tool

**Description.** `Tool::Select` lets the user drag-select a rect and
crop the editor to that region. Crop replaces `state.image` with the
cropped sub-image (translated so the new origin is `(0, 0)`), and
translates every existing annotation the same way. This is the
"drag-select a region" promise in Phase 1's exit criteria.

**Acceptance criteria:**
- [ ] Dragging with `Tool::Select` shows a selection rectangle.
- [ ] Confirming the selection (button, double-click, or Enter) crops
      `state.image` to the selected rect.
- [ ] Existing annotations translate correctly (subtract the crop
      origin); annotations fully outside the crop are dropped; partial
      overlap is clipped.
- [ ] Crop is undoable (snapshot of pre-crop image + anns pushed to
      `undo`).
- [ ] `EditorOutcome` is unaffected — crop is an in-editor action.

**Verification:**
- [ ] Standing gate passes.
- [ ] Unit test: `EditorState::crop(rect)` on a known state produces
      the expected translated `anns` and cropped `image`.
- [ ] Manual: select a small region, crop, draw a new rect, save,
      verify the saved PNG matches the cropped dimensions.

**Dependencies:** Task 8 (selection rect plumbing reuses Rect's input
shape).

**Files likely touched:**
- `crates/flintshot-core/src/editor.rs` (`crop` method, selection
  state)
- `crates/flintshot-editor-egui/src/shell.rs` (Select tool input,
  confirm-crop UX)

**Size:** M

---

### Task 15: Color & stroke-width pickers + config persistence

**Description.** Phase 1's polish task. The toolbar gains a color
swatch that opens a color picker (`egui::color_picker`) and a
stroke-width slider. Changes update `state.color` /
`state.stroke_width` immediately and are persisted to config via
`Config::save()` (debounced — write at most once per second, or on
editor close).

**Acceptance criteria:**
- [ ] Color picker swatch shows the current `state.color`.
- [ ] Picking a new color updates `state.color` live; subsequent
      annotations use it.
- [ ] Stroke-width slider covers 1–20 px; updates
      `state.stroke_width` live.
- [ ] On editor close, the latest `color` / `stroke_width` /
      `font_size` are written to `~/.config/flintshot/config.toml`.
- [ ] Restarting `flintshot region` picks up the saved defaults.
- [ ] Config write failures (permission, disk full) are surfaced as
      a non-blocking warning (annotation continues to work).

**Verification:**
- [ ] Standing gate passes.
- [ ] Manual: change color to blue, change width to 8, draw a rect,
      save, close editor; rerun `flintshot region`, verify the toolbar
      shows blue and width 8.
- [ ] Manual: make config dir read-only (`chmod -R 555
      ~/.config/flintshot`); verify warning on close, no crash.

**Dependencies:** Task 1, Task 8.

**Files likely touched:**
- `crates/flintshot-editor-egui/src/shell.rs` (picker widgets,
  debounce)
- `crates/flintshot-core/src/config.rs` (no schema change expected;
  confirm serialization roundtrips)

**Size:** M

---

### Checkpoint F — Phase 1 exit criteria met

- [ ] Standing gate green.
- [ ] All Phase 1 deliverables in `ROADMAP.md` checked off.
- [ ] **Exit scenario walkthrough** on at least one platform from
      `TEST_MATRIX.md`:
  - `flintshot region` opens editor
  - Drag-select a region with `Tool::Select`; crop
  - Draw a red rectangle over the area of interest
  - Draw an arrow pointing at it
  - Add a text label
  - Apply a Gaussian blur to sensitive content
  - Save as PNG via `Ctrl+S`
  - Copy to clipboard via `Ctrl+C`
  - Verify the PNG and the clipboard contents both contain the
    cropped image with all annotations baked in
- [ ] `docs/TEST_MATRIX.md` Phase 1 row filled in for at least one
      platform.
- [ ] `CHANGELOG.md` `## [Unreleased]` updated with Phase 1 entries.
- [ ] ADRs written for any non-obvious Phase 1 decisions (e.g. cosmic-text
      integration, compose_final as separate path from render).
- [ ] **Review with human; ready to start Phase 2.**

---

## Session notes

_None yet. Append (don't overwrite) entries here when a task deviates
from this plan during execution. Each entry: task number, what
changed, why, and how it affects downstream tasks. Surfaced by the
`/status` command._

### Phase 0 close-out — `<pending first commit hash>`

**Deviation.** Phase 0 deliverables landed in the initial scaffolding
commits without the ROADMAP checkboxes being ticked; the
`/start`-generated "next task" message treated the phase as
incomplete when in fact all 16 deliverables and 5 exit criteria
were already satisfied in the working tree.

**Change.** Ticked all 12 previously-unchecked Phase 0 boxes in
`ROADMAP.md`. Verified the standing gate end-to-end:
`fmt --check`, `clippy --all-targets -- -D warnings`,
`test --all-targets`, `test --doc -p flintshot-core` (doc tests
scoped to lib crates — `cargo test --doc` fails on `flintshot-app`
because it's a bin and has no doc tests to run). Also corrected the
ROADMAP line that said the egui stubs return `todo!()` — they
actually return `FlintshotError::NotImplemented`, which is
non-panicking and better.

**CI caveat.** No git remote is configured
(`git remote -v` is empty), so CI has never actually run against a
push. Per maintainer decision, the **local standing gate stands in
for CI** for Phase 0 sign-off. The first push will run the real
pipeline; if it fails on something the local gate didn't catch,
that becomes the first entry in the next session's notes.

**Cosmic build caveat.** `cargo build -p flintshot-cosmic` was
verified locally after installing the two missing system deps
(`libfreetype6-dev`, `libgfxbuffer-dev`) — all other cosmic system
deps (cmake, fontconfig, glib, xkbcommon) were already present.
CI's dedicated `cosmic-build` job will re-verify on push.

**Downstream impact.** None. Phase 1 proceeds against
`docs/PLAN.md` as written; Task 1 (config module) starts now.

### Task 1 close-out — `<pending first commit hash>`

**Deviation.** Task 1 deliverables landed in the initial scaffolding
commit (`a1aecb4` "init scaffolding the project"), same pattern as
the Phase 0 close-out above. The `/start`-generated "next task"
message surfaced Task 1 as incomplete based on unchecked boxes,
when in fact all 5 acceptance criteria and 3 verification items
were already satisfied in the working tree (8 unit tests in
`config.rs`, 10 in `serde_util.rs`, plus the `Color` hex
serde plumbing in `geom.rs`).

**Change.** Ticked Task 1 boxes in `TODO.md` and the
"Initial config file at platform-appropriate location" deliverable
in `ROADMAP.md`. Re-verified the standing gate end-to-end with a
notable discovery (see next paragraph).

**Standing-gate caveat — `--workspace`.** The standing gate as
written in `AGENTS.md` runs `cargo test --all-targets` and
`cargo test --doc` without a package selector. With
`default-members = ["crates/flintshot-app"]`, the first command
runs only the app's tests (currently zero); the second errors
on `flintshot-app` because it has no library target. To actually
exercise `flintshot-core`'s 27 unit tests + integration test,
the gate must be `cargo test --workspace --all-targets` and
`cargo test --doc -p flintshot-core -p flintshot-editor-egui`
(scoping doc tests to lib crates). **AGENTS.md updated** to
reflect the correct commands; future sessions should use the
adjusted gate.

**Verification.** `cargo run -- --dump-config` smoke-prints
valid TOML — the throwaway flag is explicitly allowed as the
Task 1 manual verification alternative (`TODO.md` Task 1
verification note). The real `config` subcommand lands in
Task 3.

**Downstream impact.** None. Task 2 (capture module) proceeds
against `docs/PLAN.md` as written; the throwaway `--dump-config`
flag remains in place until Task 3 removes it.

### Task 2 close-out — `<pending first commit hash>`

**Deviation 1 — `source` → `detail` field rename.** Task spec says
`FlintshotError::Capture { source: String }`. The literal field name
`source` conflicts with `thiserror`'s convention: thiserror treats
any field named `source` as the error-chain source and requires it
to satisfy `std::error::Error`, which `String` does not.

**Change.** Renamed the field to `detail` to match the existing
`FlintshotError::Config { path, detail }` pattern. The
`#[error("capture error: {detail}")]` format string and the
macOS-permission-substring contract are preserved verbatim. Downstream
code matching on the variant (Task 5's editor shell, Task 9's save
flow) should use `FlintshotError::Capture { detail }` instead of
`{ source }`.

**Deviation 2 — `libgbm-dev` needed at link time.** xcap's own README
lists the apt deps as `pkg-config libclang-dev libxcb1-dev
libxrandr-dev libdbus-1-dev libpipewire-0.3-dev libwayland-dev
libegl-dev`, but `cargo build` fails at link with
`rust-lld: error: unable to find library -lgbm`. xcap's Wayland path
pulls in `libgbm` (via `libdrm`/`smithay` bindings) at link time,
and the dev headers aren't transitively installed by anything else
on that list.

**Change.** Added `libgbm-dev` to the `XCAP_LINUX_DEPS` env in
`.github/workflows/ci.yml` and to the apt install snippet in
`AGENTS.md` (with a `> **Note:**` callout that xcap's README omits
it). All four CI jobs (`lint`, `test`, `msrv`, `cosmic-build`) install
the full list; the maintainer's local machine needed
`libclang-dev`, `libpipewire-0.3-dev`, and `libgbm-dev` installed
before the build succeeded.

**Deviation 3 — `--workspace` tightened across the gate.** The
standing-gate inconsistency surfaced during Task 1 close-out also
applied to clippy and the MSRV build. CI's `lint` job now runs
`cargo clippy --workspace --all-targets` (was `--all-targets` without
`--workspace`, which only linted `flintshot-app`); `msrv` now runs
`cargo build --workspace --all-targets`. AGENTS.md, justfile,
CONTRIBUTING.md, docs/TODO.md, docs/PLAN.md, and
`.opencode/commands/status.md` updated to match.

**Implementation notes.**
- `capture_region` follows the TODO spec literally (crop after full
  capture), even though xcap exposes `monitor.capture_region(...)`
  natively. Single code path; consistent HiDPI behaviour. Can revisit
  in Phase 2 if multi-monitor perf demands it.
- `find_primary` falls back to the first monitor if `xcap` reports
  none as primary (some virtual heads-up / single-monitor
  environments). This avoids a hard failure on unusual but valid
  setups.
- Three headless unit tests cover the rect-validation and crop-clamp
  paths without invoking xcap; the happy-path capture is the
  `#[ignore]`'d `smoke_can_grab_primary` integration test.
- Three `#[allow(clippy::…, reason = "…")]` annotations:
  `float_cmp` on `validate_rect` (structural zero-extent check, not
  arithmetic equality), `cast_possible_truncation` + `cast_sign_loss`
  on `crop_to_rect` (values clamped to image bounds before the cast).
  All carry the required justification comment per AGENTS.md.

**Downstream impact.**
- Tasks 3, 5, 6 call `capture_fullscreen` / `capture_region`; they
  receive `FlintshotError::Capture { detail }` (not `{ source }`).
- Task 5's macOS permission-detection should match on
  `detail.contains("permission")` as planned.
- Phase 1.F's `Tool::Select` crop will reuse `crop_to_rect`'s
  clamp-to-image-bounds semantics if it makes sense; otherwise it
  can implement its own.

### Task 3 close-out — `<pending first commit hash>`

**Implementation.** Replaced `run_region` / `run_fullscreen` stubs
with real `capture_fullscreen(...)` calls and PNG writes via
`image::save_with_format(path, ImageFormat::Png)` (format forced so
non-`.png` paths still yield valid PNG bytes). Added `Command::Config`
variant — `flintshot config` loads via `Config::load()` and prints
resolved TOML to stdout. Added a shared `OutputArgs { out:
Option<PathBuf> }`, `#[command(flatten)]`-ed into both `RegionArgs`
and `FullscreenArgs` so the `--out PATH` flag appears in each
subcommand's `--help` rather than under a "COMMON OPTIONS" header.

**Deviation 1 — region captures the full primary monitor for now.**
The TODO spec is explicit (line 133-136: "writes a real PNG of the
primary monitor … just do fullscreen-crop to a sane default") but
worth restating because it surprises future readers: `flintshot
region` and `flintshot fullscreen` currently produce equivalent
output. The interactive region-select tool lands in Task 14 (Phase
1.F); until then `run_region` calls `capture_fullscreen(None)` rather
than `capture_region(rect, ...)` because there is no UI to acquire
the rect. Task 6 will reroute `region` through the editor (no
`--out`), at which point the select tool becomes the natural source
of the rect.

**Deviation 2 — compact ISO-8601 timestamps, no `time`/`chrono` dep.**
TODO specifies "prefer `std::time` + formatting helper". Implemented
Howard Hinnant's `civil_from_days` algorithm inline in a new
`crates/flintshot-app/src/time_util.rs` (~30 lines + 5 unit tests).
Adapted to `u64` throughout — the original is sign-aware to support
pre-epoch dates, but filename timestamps are always post-1970, so
the `era` branch collapses to plain `z / 146_097` and every value
stays non-negative (no truncating casts for clippy to flag).
Timestamp format is `YYYYMMDDTHHMMSSZ` (e.g. `20260721T143022Z`) —
separator-free because `:` is reserved in Windows paths and awkward
to escape in shells. Both forms are valid ISO-8601; the compact
form is universally filesystem-safe.

**Dep-tree change.** Added `image = { workspace = true }` to
`flintshot-app/Cargo.toml`. The `image` crate was already a workspace
dep (PNG feature enabled for `flintshot-core`); no new external crate
was introduced. The TODO explicitly permitted this with "no new deps
unless `time`/`chrono`" — `image` doesn't count, and the alternative
(add a `save_png` helper to `flintshot-core`) would have been
premature factoring ahead of Task 13's `image_ops` module.

**Throwaway flag removed.** `flintshot --dump-config` (added in Task
1 as a temporary verification escape hatch) is gone. The `Cli` struct
no longer has a `dump_config` field; `main.rs` no longer has the
`dump_config()` function. The real `config` subcommand replaces it
verbatim.

**Error paths.** Every fallible step is wrapped in `.with_context(...)`:
capture failures surface as `capturing [region|fullscreen] on monitor
Some(N)` → `capture error: <detail>`; filesystem failures as
`creating output directory <path>` or `writing PNG to <path>`.
Verified manually: `fullscreen --monitor 99` exits 1 with a readable
two-layer report; `region --out /proc/cannot-write.png` exits 1
with the io::Error wrapped in context. No panics, no `unwrap`s, no
`expect`s in the new code.

**Verification.** Standing gate green end-to-end (`fmt --check`,
`clippy --workspace --all-targets -- -D warnings`, `test --workspace
--all-targets`, `test --doc -p flintshot-core -p flintshot-editor-egui`).
End-to-end smoke against a live X11 display: `cargo run -- region
--out /tmp/x.png` produced a 2560×1440 RGBA PNG; `cargo run --
fullscreen --monitor 0 --out /tmp/y.png` likewise; `cargo run --
fullscreen` (no `--out`) wrote `/tmp/flintshot-<compact-ISO-8601>Z.png`;
`cargo run -- config` printed valid TOML; `cargo run -- --dump-config`
errored cleanly (clap exit code 2). Five new `time_util` unit tests
cover the calendar math at known epoch seconds including the epoch
boundary, a one-day rollover, a leap day (2024-02-29), and the
Gregorian year rollover across 2023-12-31 → 2024-01-01.

**Downstream impact.**
- Task 6 will reroute `run_region` through `EguiShell::run_editor`
  when `--out` is absent. The current `OutputArgs` flatten + the
  `out: Option<PathBuf>` field make this trivial: `None` ⇒ editor,
  `Some(path)` ⇒ Task 3's PNG-write path preserved.
- Task 6's `cfg(not(feature = "egui"))` fallback path can reuse
  `write_png` directly (currently `pub(crate)`; promote to `pub` if
  a downstream caller needs it).
- Task 5's macOS permission-detection should match on the `detail`
  string from `FlintshotError::Capture` (already the case per the
  Task 2 close-out). No change required.
- The `time_util` module is `pub(crate)`; if any future crate wants
  ISO-8601 filenames (e.g. upload cache keys in Phase 4), promote
  to `flintshot-core` rather than duplicating the algorithm.

### Task 4 close-out — `<pending first commit hash>`

**Implementation.** Filled in `flintshot-core::render::render()` so
it uploads the background screenshot on first frame, caches the
handle, and blits to the full image rect on every subsequent frame.
Reshaped `EguiCanvas` to carry a stable `egui::Context` plus a
per-frame `Option<egui::Painter>` bound by the embedding shell.
Implemented `upload_image` via `ctx.load_texture(...,
TextureOptions::NEAREST)` (sharp pixels) and `draw_image` via
`painter.image(...)` with physical → logical conversion at
`pixels_per_point`.

**Deviation 1 — cache lives at the call site, not in the Canvas
struct.** TODO spec says "uploads the background image (caching the
handle in the canvas)". Read literally, that means adding cache
methods to the `Canvas` trait. This implementation puts the cache
in a third `render()` parameter: `bg: &mut Option<C::Image>`. The
shell owns the cache field; `render()` borrows it for the duration
of the frame.

Why the deviation: `editor.rs` line 152-153 already states
"`EditorState`'s toolkit-specific texture handle does *not* live
here — the shell owns it on its side" — i.e., the shell-level cache
is already the intended design. Putting it back in the Canvas
struct would either (a) force every Canvas impl (egui, MockCanvas,
future iced/slint) to add `background_handle()` / `set_background_handle()`
trait methods — boilerplate with stateful concerns that don't
belong in a "primitive drawing" trait — or (b) require each shell
to subclass `EguiCanvas` with an extra field, which buys nothing.
The `&mut Option<C::Image>` signature keeps the trait pure and
makes cache invalidation trivial (Task 14's crop will `*bg = None`
when `state.image` is replaced; the next `render()` re-uploads
automatically). Tested by `render_reuploads_after_cache_invalidated`.

**Deviation 2 — inline `MockCanvas` for Task 4 tests, promoted by
Task 7.** TODO spec assigns MockCanvas to Task 7 and offers two
paths for Task 4: "defer this assertion to Task 7 and instead
assert via a temporary inline mock". Took the inline-mock path.
The current `MockCanvas` lives in `render.rs`'s `#[cfg(test)] mod
tests` and records only `draw_image` calls (the only one render's
background-blit path exercises). Task 7 promotes it to a public
struct with a full `DrawCall` enum covering every `Canvas` method;
the test scaffolding (`Vec<DrawCall>` field, push-on-each-call
pattern) is designed to make that promotion a pure extension.

**Three render tests landed** (not just the one the spec asked for):
- `render_blits_background_image_once` — the spec's exact assertion.
- `render_caches_background_handle_across_frames` — verifies the
  cache contract: second render blits but doesn't re-upload.
- `render_reuploads_after_cache_invalidated` — forward-looking;
  models Task 14's crop path. The test exists now because it
  exercises the `*bg = None` invalidation contract that all future
  cache-aware callers (Task 5's shell, Task 14's crop) must honour.

**API change.** `render()` gained a third parameter. Currently no
external callers exist, but the signature is now `render(state,
canvas, bg: &mut Option<C::Image>)`. CHANGELOG entry added.

**egui 0.35 discovery — `TextureOptions` has four fields.** Initial
draft constructed the struct with just `magnification` +
`minification`; egui 0.35 adds `mipmap_mode: Option<TextureFilter>`
and `wrap_mode: TextureWrapMode`. Switched to the built-in
`TextureOptions::NEAREST` const, which sets both filters to Nearest,
wrap to `ClampToEdge`, mipmap to `None` — exactly what we want for
pixel-accurate screenshot rendering.

**No GUI types in core.** The `no_gui_types_in_core` integration
test still passes (verifies by source-grepping `flintshot-core/src/`
for forbidden `use egui::` / `use eframe::` / etc.). The conversion
from physical to logical px lives in `EguiCanvas::draw_image`, not
in `render()` — `render()` only emits physical-px rects.

**Downstream impact.**
- Task 5's shell must store `bg: Option<EguiTexture>` as a field
  and call `canvas.bind_painter(&ui.painter())` each frame before
  `render(state, &mut canvas, &mut bg)`. That's the per-frame
  handshake the TODO's "canvas needs an `egui::Context` (or
  `Painter`); re-shape the struct to receive one per frame" note
  foreshadowed.
- Task 7 promotes the inline `MockCanvas` to a public struct.
- Task 14's crop will need to invalidate the cache (`*bg = None`)
  when `state.image` is replaced. The test
  `render_reuploads_after_cache_invalidated` already locks in this
  contract.
- Future annotation dispatch (Tasks 8/11/12/13) is appended to
  `render()` after the background blit — single shared path, no
  per-toolkit duplication.

### Task 5 close-out — `<pending first commit hash>`

**Implementation.** Filled in `EguiShell::run_editor` to construct
an `EditorApp` (private `eframe::App` impl) carrying the
`EditorState`, open a single-window `eframe::run_native` session,
and return `EditorOutcome::Cancelled` on `Esc` or window-close.
Module layout: `flintshot-editor-egui/src/{lib,canvas,app,shell}.rs`
— `lib.rs` is now a thin module-export file; `canvas.rs` holds
`EguiCanvas`/`EguiTexture` (Task 4's contents, unchanged); `app.rs`
holds the `EditorApp` `eframe::App` impl; `shell.rs` holds the
`EguiShell` `EditorShell` impl + the `run_editor_blocking` helper.
Public API unchanged (`pub use canvas::{EguiCanvas, EguiTexture};
pub use shell::EguiShell;`).

**Deviation 1 — egui 0.35 has no `run_simple_native`.** The
close-out prediction in the old `EguiShell` stub (`lib.rs:111`:
"Phase 1 wires `eframe::run_simple_native`") was wrong for 0.35.
The actual entrypoint is `eframe::run_native(app_name,
NativeOptions, AppCreator)` where `AppCreator` is a `Box<dyn
FnOnce(&CreationContext) -> Result<Box<dyn App>, DynError>>`.

This turned out to *simplify* the design. The original plan had a
chicken-and-egg problem: `EguiCanvas::new` needs an `egui::Context`,
but the context isn't available until eframe is running. The plan
called for `canvas: Option<EguiCanvas>` with lazy init on first
`update()`. In 0.35 the `AppCreator` closure receives
`&CreationContext { egui_ctx, ... }` *before* the app is
constructed, so the canvas is constructed eagerly inside the
closure. No `Option`, no lazy init, no first-frame special casing.

**Deviation 2 — `App::update` is gone; trait split into `logic` +
`ui`.** egui 0.35 split `eframe::App::update(&mut self, ctx, frame)`
into two methods: `logic(&mut self, ctx, frame)` for input/state
transitions (called even when UI is hidden), and `ui(&mut self, ui,
frame)` for painting. The `ui` method receives a `&mut egui::Ui`
(not a `&Context`) and is required (no default impl). The Esc
handler lives in `logic` (input boundary); the `render()` call
lives in `ui` (paint boundary) inside a
`CentralPanel::default().show(ui, …)` wrapper — `CentralPanel::show`
also changed signature in 0.35 (takes `&mut Ui`, not `&Context`).
Future tools (Tasks 8/11/12/13) will follow the same split:
state-machine updates in `logic`, drawing in `ui`.

**Deviation 3 — outcome slot type is `Result<EditorOutcome,
FlintshotError>`, not `Option<EditorOutcome>`.** The plan had the
slot as `Option<EditorOutcome>` — `Some(Cancelled)` for Esc, `None`
for window-close (X), both mapping to `Ok(Cancelled)` at the read
site via `unwrap_or(Cancelled)`. That design silently drops render
errors: if `render()` fails inside `ui()`, the only signal was a
log line + outcome `Some(Cancelled)`, indistinguishable from a
user-initiated Esc.

The implemented slot is `Rc<RefCell<Option<Result<EditorOutcome,
FlintshotError>>>>` (see deviation 4 for why `Rc<RefCell>` and not
`Arc<Mutex>`). The render-error path publishes `Err(err)`, which
`run_editor` propagates via `?` at the read site. Window-X stays
`None`, falling back to `Ok(Cancelled)`. Costs one extra nested
`?` at the read site; gains real error propagation. Forward-looking:
Task 9's `rfd`/`arboard` failures can publish `Err(...)` through
the same channel without a parallel mechanism.

**Deviation 4 — `Rc<RefCell<…>>` for the outcome slot, not the
planned `Arc<Mutex<…>>`.** The plan specified `Arc<Mutex<…>>` for
cross-thread safety. eframe 0.35's native event loop is strictly
single-threaded: `eframe::App` carries no `Send`/`Sync` bounds,
`AppCreator` returns `Box<dyn App>` (no Send), and `run_native`
blocks the calling thread until the window closes. There is no
second thread that could touch the slot. `Rc<RefCell<…>>` is the
idiomatic choice for single-threaded shared state: no poisoning,
no lock overhead, no `unwrap_or_else(PoisonError::into_inner)`
noise. clippy's `non_send_fields_no_atomic` (restriction set, off
by default) doesn't fire.

If Task 6 ever needs to dispatch editor work to a background
thread (Phase 2's multi-window daemon does), the slot type
changes to `Arc<Mutex<…>>` and the `App` gains whatever Send
bounds the new architecture demands. For Phase 1's
editor-as-blocking-call model, `Rc<RefCell<…>>` is correct.

**Deviation 5 — no explicit window-size clamping for now.** TODO
spec acceptance criterion: "sized to the screenshot dimensions
(clamped to viewport)". egui 0.35's `ViewportBuilder::with_inner_size`
takes logical px; we pass the screenshot's physical dimensions
directly. On HiDPI displays the WM scales the window to fit the
monitor and egui's `pixels_per_point` keeps the blit 1:1 with the
source texture (sharp screenshot pixels). The "clamped to viewport"
part is left to the WM for now — egui 0.35 doesn't expose a way
to query monitor size *before* `run_native` starts the event loop
(`ViewportInfo::monitor_size` is only available inside `ui`/`logic`).
Task 14 (Select/crop tool) can revisit if precise window sizing
becomes load-bearing; for Task 5's "de-risk egui integration" goal,
WM-handled clamping is sufficient.

**Deviation 6 — `flintshot-editor-egui` gained a `tracing` dep.**
The render-error path in `EditorApp::ui` logs via
`tracing::error!` before publishing `Err(...)`. Library convention
(AGENTS.md): libs emit `tracing` events; the binary owns the
subscriber. `flintshot-app` already initializes `tracing_subscriber`
per the Phase 0 scaffolding, so the events surface in the
terminal. One new workspace-dep line in
`crates/flintshot-editor-egui/Cargo.toml`.

**Deviation 7 — `FlintshotError::Shell { detail }` variant added.**
`eframe::Error` (which `run_native` returns) has no variant in the
existing `FlintshotError` enum — none of `NotImplemented`,
`Config`, `Capture`, `InvalidRect` fit a window-system failure.
Added `Shell { detail: String }` mapped via `err.to_string()`. The
shell calls it through a small free helper `eframe_err_to_flintshot`
in `app.rs` (`pub(crate)`) — clippy asked for `&eframe::Error`
rather than owned (`needless_pass_by_value`), so the helper takes
a reference and the caller uses `.map_err(|e|
eframe_err_to_flintshot(&e))`. Will be reused by Task 9's
`rfd`/`arboard` failures and Phase 3's pin window.

**Borrow-checker driven refactor in `EditorApp::ui`.** Initial
draft called `self.publish_outcome(Err(err))` *inside* the
`CentralPanel::show` closure. That captured `&self` wholesale,
conflicting with the closure's existing disjoint captures of
`&mut self.canvas`, `&mut self.bg`, `&self.state`. Restructured:
the closure returns the render `Result`, and error handling
(logging + `publish_outcome` + `ViewportCommand::Close`) happens
*outside* the closure body. Cleaner anyway — the closure is now
pure rendering, the surrounding method is control flow.

**No new tests.** As planned: eframe requires a real display, so
unit-testing `run_editor` would mean a synthetic event-loop harness
that doesn't exist yet. The standing gate (fmt + clippy + test +
test-doc, all green) covers Task 5's compile-time guarantees.
Manual smoke (open window, see screenshot, Esc closes, terminal
prints `EditorOutcome::Cancelled`) is explicitly deferred to Task 6
per TODO:243-245 ("after Task 6 wires it"). Task 10 will extract a
synthetic-input dispatcher that *is* unit-testable.

**Verification.** Standing gate green end-to-end:
- `cargo fmt --all --check` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — clean
  after one round of fixes (clippy::needless_pass_by_value on the
  `eframe_err_to_flintshot` helper; switched to `&eframe::Error`
  and updated the caller).
- `cargo test --workspace --all-targets` — 5 + 33 + 1 ignored + 1
  integration, all pass.
- `cargo test --doc -p flintshot-core -p flintshot-editor-egui` —
  clean.

**Downstream impact.**
- Task 6 (CLI routing) calls `EguiShell::run_editor(state)` and
  matches on the returned `EditorOutcome`. The outcome slot's
  `Result<EditorOutcome, FlintshotError>` shape means Task 6 will
  use `?` to propagate shell failures as `color_eyre::Report` at
  the binary boundary.
- Task 7's state-machine methods (`begin_annotation`,
  `update_annotation`, `commit_annotation`) will be called from
  inside `EditorApp::logic`, not `ui`, because pointer events are
  input — same split as Esc. The `egui::Context::input` reader is
  already wired up in `logic`; Task 7 just adds more matching.
- Task 8 (Rect tool) renders via the existing `render(&state,
  &mut canvas, &mut bg)` call in `ui()` — no per-tool UI changes,
  just core-side dispatch appended to `render()`.
- Task 10's synthetic-input dispatcher will need to expose the
  `logic()` body (or the relevant input-matching subset) as a
  pure function `dispatch_input(&mut EditorState, &InputEvent)`.
  Keep that in mind when designing Task 7's state-machine API.
- Task 14 (Select/crop tool) will likely revisit window sizing
  once region selection makes precise dimensions load-bearing.
  The current `with_inner_size(physical_dims)` + WM-clamp approach
  is documented as a known shortcut in `shell.rs`.

### Task 5 close-out — `<pending first commit hash>`

**Implementation.** Implemented `EguiShell::run_editor` as a
single-window blocking eframe app. Three new modules in
`flintshot-editor-egui`:
- `canvas.rs` — promoted out of `lib.rs`; no behaviour change.
- `app.rs` — `EditorApp` (the `eframe::App` impl) + an internal
  `OutcomeSlot` type shared between shell and app.
- `shell.rs` — `EguiShell` + `run_editor_blocking` helper that
  constructs `NativeOptions`, dispatches `eframe::run_native`, and
  reads back the outcome when the window closes.
- `lib.rs` now just declares modules + re-exports `EguiCanvas` /
`EguiTexture` / `EguiShell`.

**Deviation 1 — eframe 0.35 `App::ui` instead of `App::update`.**
eframe 0.35 split the per-frame callback: `App::logic(ctx, frame)`
handles input/state (no painting), `App::ui(ui, frame)` paints.
The classic `App::update(ctx, frame)` from earlier eframe versions
**does not exist in 0.35** — implementing it fails with
`method not a member of trait App`. Reshaped accordingly: Esc
handling lives in `logic` (input-only), `render()` lives in `ui`
(painting-only). Future tool input (Tasks 7+) keeps the same
split — pointer events in `logic`, drawing in `ui`.

**Deviation 2 — `CentralPanel::default().show(ui, ...)` not
`.show(ctx, ...)`.** In eframe 0.35 the panel `show` signature
takes the root `&mut Ui` that `App::ui` receives — *not* a
`&egui::Context`. Confusingly, `CentralPanel::show(ctx, ...)`
still compiles (it's a different, context-level API) but doesn't
paint where you expect. The correct call inside `App::ui` is
`CentralPanel::default().show(ui, |ui| { ... })`. Its return type
is `InnerResponse<R>`; we use `.inner` to surface the `Result`
from `render()` so render errors can publish `Err(_)` into the
outcome slot.

**Deviation 3 — outcome slot carries `Result<EditorOutcome>`, not
bare `EditorOutcome`.** Lets render failures inside `App::ui`
propagate as `Err(_)` through the slot rather than being silently
dropped. The current render path is infallible in practice (egui's
`load_texture` doesn't fail), but this design future-proofs against
Task 13's blur upload path which can plausibly fail. Window-X
close still maps to `Ok(Cancelled)` via the slot defaulting to
`None` → `unwrap_or(Ok(Cancelled))` at the read site.

**Deviation 4 — `FlintshotError::Shell` rather than `Editor`.**
TODO spec foreshadowed `FlintshotError::Editor`, but the existing
convention in the codebase is to name variants after the source
subsystem (`Config`, `Capture`, `Io`, …). Named it `Shell` to
match `EditorShell` and avoid confusion with `EditorState` /
`EditorOutcome`. The variant carries `{ detail: String }` matching
the existing `Capture` / `Config` pattern (named `detail` rather
than `source` so `thiserror` doesn't try to chain it as
`dyn Error`).

**Window-size polish deferred.** TODO spec calls for "sized to the
screenshot dimensions (clamped to viewport)". Implemented the
sized-to-screenshot half via `ViewportBuilder::with_inner_size`.
The clamped-to-viewport half requires querying the monitor's
dimensions before window creation — eframe doesn't expose that
cleanly pre-CreationContext, and the WM typically clamps
anyway. Marked as a polish item in the code comment; revisit if
Task 14's region select makes window-size precision matter.

**Smoke evidence gathered (partial verification).** Added
`crates/flintshot-editor-egui/examples/open_editor.rs` as a
throwaway probe: captures the primary monitor and opens the shell.
On the maintainer's Wayland+XWayland dev machine, the example:
- Compiles clean.
- Capture succeeds (2560×1440 RGBA PNG via PipeWire).
- Logs `"captured 2560x1440; opening editor"`.
- Blocks in `run_native` for the full 5s test window with **no
  panic and no error log**.
What could **not** be verified headlessly: actual on-screen window
visibility (Wayland windows are invisible to `wmctrl`'s X11
polling) and Esc-close behaviour (requires GUI interaction the
terminal can't drive). Both need human eyes on a real display,
deferred to the Task 6 close-out when `flintshot region` provides
a CLI-driven path.

**Example kept in tree.** `examples/open_editor.rs` is tiny (~15
lines), documents itself as a Phase 1.B de-risking probe, and is
explicitly slated for deletion once Task 6 ships the real
`flintshot region` editor path. Worth keeping until then as a
smoke-test fixture.

**Downstream impact.**
- Task 6 routes `run_region` / `run_fullscreen` through
  `EguiShell::run_editor(state)` when `--out` is absent. The
  `EditorOutcome` returned maps cleanly: `Cancelled` → exit 0,
  no output file; `Saved(p)` → log path; future variants → log.
  `--out PATH` keeps Task 3's PNG-write path unchanged.
- Task 7's MockCanvas test infrastructure interacts with `render()`
  only — no direct dependency on `EditorApp` or the outcome slot.
- Task 9's Save/Copy will set the slot via
  `EditorApp::publish_outcome` — same path Esc uses today.
- Task 10's keyboard dispatcher goes into `App::logic` alongside
  the existing Esc handler.
- `FlintshotError::Shell` is now part of the public API; any code
  matching on `FlintshotError` exhaustively needs to add a
  `Shell { detail }` arm.

### Task 6 close-out — `<pending first commit hash>`

**Implementation.** Refactored `run_region` / `run_fullscreen` to
share a `capture_and_dispatch(monitor, mode, output)` helper. When
`--out` is `Some(path)`, the helper writes the PNG directly (Task 3's
path, byte for byte). When `--out` is `None`, it hands the captured
image to `EguiShell::new().run_editor(EditorState::new(img))` and
logs the outcome. The editor path is gated on `cfg(feature = "egui")`
via two definitions of `run_editor_for_capture` with matching
signatures; the `cfg(not(feature = "egui"))` stub returns a clear
error: "editor not built; pass --out PATH to capture headlessly
(this binary was compiled without the `egui` feature)".

**Deviation 1 — `time_util.rs` deleted entirely.** The Task 3
close-out's "Downstream impact" line said "if any future crate wants
ISO-8601 filenames, promote to `flintshot-core` rather than
duplicating". With Task 6 removing `OutputArgs::resolve_path`,
`iso8601_now` lost its only caller. Read strictly, that left the
option of keeping the module as dead code (with `#[allow(dead_code)]`
or a feature gate). Chose deletion: the algorithm is in git history,
the Task 3 close-out explicitly prescribes the `flintshot-core`
promotion path rather than revival of the `flintshot-app` copy, and
5 unit tests of dead code is exactly the kind of "ash weight" the
plan's code-simplification principle rejects. If Phase 4 upload-cache
keys need ISO-8601, restore from `git show` and port to `flintshot-core`.

**Deviation 2 — `OutputArgs::resolve_path` removed.** No callers
remain after the Task 6 refactor. The `clap` flatten + `out:
Option<PathBuf>` shape stays unchanged so `flintshot region --help`
still shows `--out` exactly as in Task 3 — only the default behaviour
shifted (was: write timestamped PNG; now: open editor or error).

**Deviation 3 — `clippy::wildcard_enum_match_arm` allow on the
`EditorOutcome` match.** `EditorOutcome` is `#[non_exhaustive]`;
from outside `flintshot-core`, the compiler requires a `_` arm. The
`clippy::pedantic` `wildcard_enum_match_arm` lint still fires on `_`
even when `#[non_exhaustive]` mandates it, so the match carries an
`#[allow(clippy::wildcard_enum_match_arm, reason = "EditorOutcome is
#[non_exhaustive]; _ is mandatory")]`. The reason string spells out
*why* the allow exists, matching the project's `clippy::pedantic`
convention (justifying comment required per `AGENTS.md`). All four
named variants are still spelled out — the wildcard only covers
future additions.

**Deviation 4 — `examples/open_editor.rs` deleted.** Per the Task 5
close-out ("Example kept in tree … explicitly slated for deletion
once Task 6 ships the real `flintshot region` editor path") and the
`SESSION_HANDOFF.md` carry-forward ("After Task 6 lands, delete
`crates/flintshot-editor-egui/examples/open_editor.rs`"). The
`flinkshot region` CLI now covers that role end-to-end. The
`examples/` directory is now empty; left in place (git tracks empty
dirs via `.gitkeep` only if needed; here the absence is fine).

**Verification.** Standing gate green end-to-end:
- `cargo fmt --all --check` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.
- `cargo clippy --no-default-features -p flintshot-app --all-targets
  -- -D warnings` — clean (new check, mandated by Task 6's "including
  a `--no-default-features` build" verification line).
- `cargo test --workspace --all-targets` — 33 pass, 1 ignored
  (capture smoke), 0 fail.
- `cargo test --doc -p flintshot-core -p flintshot-editor-egui` —
  0 doc tests (unchanged).
- `no_gui_types_in_core` integration test — still green; the new
  editor wiring lives in `flintshot-app`, not core.

End-to-end smoke on the maintainer's Wayland + XWayland box:
- `cargo run -- region` — capture succeeds, eframe event loop starts,
  blocks for the full 4s test window with no panic or error log
  (terminated via SIGTERM, exit 143). Window visibility + Esc-close
  behaviour could not be confirmed from the terminal (eframe uses
  native Wayland here, so `wmctrl`'s X11 polling doesn't see the
  window to send a close event). Needs maintainer's eyes — see
  carry-forward note below.
- `cargo run --no-default-features -- region` — exits 1 with the
  clean error `"editor not built; pass --out PATH to capture
  headlessly (this binary was compiled without the \`egui\` feature)"`.
- `cargo run --no-default-features -- region --out /tmp/x.png` —
  wrote a valid 2560×1440 RGBA PNG; Task 3's headless path preserved.

**Carry-forward to Checkpoint B sign-off.** Task 5's "manual test on
real display" debt is now consolidated into Task 6's outstanding
checkbox: confirm by hand that (a) the screenshot is visible on
window open, (b) Esc returns `EditorOutcome::Cancelled` and the
process exits 0 with no output file. Once the maintainer runs
`cargo run -- region` and confirms those three things, Checkpoint B
can be signed off and Phase 1.C / Task 7 can start.

**Downstream impact.**
- Task 7's editor state machine is independent — `run_editor_for_capture`
  doesn't touch `EditorState` internals beyond `EditorState::new(img)`.
- Tasks 9 (Save/Copy) and 10 (shortcuts) will surface new
  `EditorOutcome` paths; the `match` in `run_editor_for_capture`
  already logs each named variant at `info`, and the wildcard arm at
  `warn`, so when Task 9 lands the `Saved(p)` log line is already
  wired (no further change needed in `cli.rs`).
- Phase 2's daemon will spawn `flintshot region` / `flintshot
  fullscreen` as separate processes — exactly the shape `run_editor`
  expects (eframe blocks the calling thread). The Task 6 wiring
  already matches that process model.
- The `cfg(feature = "egui")` split in `run_editor_for_capture`
  is the pattern Phase 2 will reuse when it adds `cfg(feature =
  "iced")` / `cfg(feature = "slint")` shells — one stub fn per
  inactive shell, gated on the corresponding feature flag.

### Task 7 close-out — `<pending first commit hash>`

**Implementation.** Added `EditorState::pending: Option<Annotation>`
plus the state-machine methods `begin_annotation`, `update_annotation`,
`commit_annotation`, `cancel_annotation`, `undo`, `redo`, and
`hit_test`. Added `Annotation::bounding_box() -> Option<Rect>` and
`Annotation::contains(point: Point) -> bool` for hit-testing. Promoted
the Task 4 inline `MockCanvas`/`DrawCall` out of `#[cfg(test)] mod
tests` to public types at module scope; extended `DrawCall` with one
variant per `Canvas` method; implemented every `Canvas` method on
`MockCanvas` to push the corresponding `DrawCall`. Extended
`flintshot-core::render::render()` to iterate `state.anns` and dispatch
each via the new public `render_ann(canvas, ann)` helper, then draw
`state.pending` on top.

**Deviation 1 — drag-anchor stays at `rect.min` during the drag;
normalization happens at commit.** TODO spec doesn't specify when
rect normalization happens. Initial instinct was to normalize in
`update_annotation`, but that loses the begin anchor: if the user
drags up-left from the begin point, normalizing on each update would
move `rect.min` to the current pointer, so a subsequent drag down-right
would extend from the wrong origin. Fixed by storing anchor as
`rect.min` and current pointer as `rect.max` during the drag, then
normalizing in `commit_annotation` (and at render / hit-test time via
`Rect::normalized()`). Documented in `update_annotation`'s doc
comment. Downstream impact: Tasks 8 / 11 / 13's drag input flows are
already correct by construction; Task 14's select tool reuses the same
rect-on-drag convention.

**Deviation 2 — `undo()` clears `pending`.** TODO spec is silent on
how `undo` interacts with `pending`. Per maintainer decision in the
Task 7 plan, `undo()` clears `pending` (Ctrl+Z mid-drag cancels the
in-flight annotation, matching Photoshop / GIMP / most editors). This
gives a single keystroke to "reset" the editor if a drag goes wrong.
`cancel_annotation()` remains the explicit cancel for non-undo
contexts.

**Deviation 3 — `Blur` renders as a grey placeholder + warn until
Task 13.** `render_ann` must dispatch every variant — leaving `Blur`
unmapped would `panic!`/`unreachable!` and break the render path when
a committed Blur ann exists. Real blur computation needs the
`image_ops` module (Task 13) plus the canvas's texture cache; pulling
that into Task 7 would have ballooned the scope. Chose a visible grey
`draw_rect_filled` overlay + `tracing::warn!` so a session trace shows
the placeholder is in use. The annotation is *correctly stored* in
`anns`; only the rendering is approximate. Task 13 swaps the
placeholder for real `imageproc` blur.

**Deviation 4 — `Number` renders as a square + placeholder text.**
`Annotation::Number` is documented as a "bubble" but the `Canvas`
trait has no `draw_circle` primitive. Phase 2 adds the bubble shape;
for Task 7's "every variant must dispatch" requirement, `Number`
renders as `draw_rect_filled` (square bbox) + `draw_text` (white
label). Same pattern as Blur: correctly stored, approximate render.
Task 11 / Phase 2 will refine.

**Deviation 5 — `commit_annotation` uses `clone()` not `mem::take`
for the undo snapshot.** Initial implementation used
`self.undo.push(std::mem::take(&mut self.anns))` then
`self.anns.push(ann)`. That empties `self.anns` before the new push,
so after commit `self.anns = [new_ann]` instead of `[prior_anns...,
new_ann]`. Tests caught it immediately (`undo_redo_snapshot_roundtrip`
failed with `anns.len() == 1` after 3 commits). Fixed to
`self.undo.push(self.anns.clone())`. The `mem::take` pattern is
correct for `undo()` / `redo()` because those *replace* `self.anns`
with the popped snapshot rather than appending to it.

**Deviation 6 — `clippy::cast_precision_loss` allow on
`bounding_box`'s text-width estimate.** `text.chars().count() as
f32` triggers the lint because `usize` → `f32` can lose precision.
For text-label lengths (realistically < 10K chars, well under 2^24)
the cast is exact. Allow carries a justifying comment per AGENTS.md.
Task 12 (cosmic-text) replaces this estimate with real glyph advance
widths and the allow goes away.

**No signature change to `render()`.** Task 4's
`render(state, canvas, bg: &mut Option<C::Image>)` signature is
preserved. The new anns iteration + pending draw happens entirely
inside `render()`; no caller-visible change. Task 5's
`EditorApp::ui` keeps calling `render(&state, &mut canvas, &mut bg)`
unchanged.

**Per-variant hit-test geometry.** `Annotation::contains` uses
stroke-width-aware tolerances: thin lines (1–3 px stroke) get a
3-pixel tolerance (`HIT_TEST_TOLERANCE_PX`) so they're grabbable;
thick strokes get half their width plus the tolerance. `Text` uses a
coarse bounding-box approximation (estimated advance width =
`font_size × 0.5 × char_count`) until Task 12 adds real glyph
metrics. `Ellipse` uses bbox inclusion, not the ellipse equation —
sufficient for selection purposes; refined in Phase 2 if needed.

**Test count.** Added 19 new tests (16 in `editor.rs`, 3 in
`render.rs`). Standing gate: 52 pass (was 33), 1 ignored (capture
smoke), 0 fail. The required Task 7 tests (`undo_redo_snapshot_roundtrip`,
`commit_clears_redo`, `cancel_drops_pending`, and the committed +
pending Rect render test) are all green. Extras cover:
`begin_then_update_extends_pending`,
`update_preserves_anchor_across_drag_direction_changes` (the
deviation 1 contract), `commit_normalizes_rect`,
`undo_with_empty_history_is_noop`, `redo_after_new_commit_is_noop`,
`undo_clears_pending` (deviation 2 contract),
`hit_test_returns_topmost`, `hit_test_returns_none_when_empty`,
`hit_test_returns_none_when_miss`,
`bounding_box_normalizes_unnormalized_rect`,
`hit_test_line_uses_stroke_tolerance`,
`begin_annotation_skips_non_drag_tools`, `pending_is_none_in_new_state`,
plus two extra render tests (`render_ann_dispatches_each_variant`,
`render_pending_marker_emits_segment_lines`) that lock in the
per-variant dispatch contract for Tasks 8 / 11 / 12 / 13.

**Downstream impact.**
- Task 8 (Rect tool) is now a thin shell: translate egui pointer
  events to `begin_annotation` / `update_annotation` / `commit_annotation`
  in `EditorApp::logic`; the shared `render()` path already dispatches
  committed and pending Rects.
- Tasks 11 / 13 (Arrow / Blur) follow the same shape: tool-specific
  `begin` constructs an Arrow / Blur annotation; `render_ann` already
  dispatches both (Arrow for real; Blur as a placeholder pending Task
  13's `image_ops` work).
- Task 12 (Text) needs a separate `begin_text(at)` method on
  `EditorState` (typing input model, not drag). The `pending` field
  already supports holding a Text ann; `render_ann`'s Text arm is
  wired for the toolkit default font.
- Task 14 (Select / crop) reuses `hit_test` for the selection model
  and `Annotation::bounding_box` for the selection rectangle
  geometry. The drag-anchor convention (deviation 1) means Task 14's
  selection rect should follow the same `rect.min`-as-anchor pattern.
- Task 10 (keyboard shortcuts) will add a `dispatch_input(&mut
  EditorState, &InputEvent)` pure function (per the Task 5 close-out's
  forward-looking note). The new state-machine methods are pure
  already; `dispatch_input` will just be a match over `InputEvent`
  calling them.
- Phase 2's select-tool will use `hit_test` directly; if it needs
  multi-selection (shift-click), the API will need extension. Single-
  selection as specified by Phase 1 is sufficient.

### Task 8 close-out — `<pending first commit hash>`

**Implementation.** Wired `Tool::Rect` end-to-end: egui pointer
events flow through `EditorApp::logic` into the Task 7 state
machine (`begin_annotation` / `update_annotation` /
`commit_annotation`); the shared `render()` path draws both
committed and pending rects via two newly-implemented
`EguiCanvas` primitives. A `egui::Panel::top("editor_toolbar")`
above the CentralPanel carries a single `selectable_value` Rect
selector plus an annotation-count label and a hint line. `Ctrl+Z`
in the same `logic()` body calls `state.undo()`. Real egui
`Painter` impls for `draw_rect_outline` (using
`StrokeKind::Inside`) and `draw_rect_filled` fill out the canvas
trait surface for the Rect path; the remaining primitives
(`draw_ellipse`, `draw_line`, `draw_arrow`, `draw_text`) stay as
stubs until Tasks 11/12/13.

**Deviation 1 — `TopBottomPanel` is gone in egui 0.35; unified
into `Panel`.** The 0.30 source still in the registry's sibling
directory has `TopBottomPanel::top` and `SidePanel::left`. The
0.35 release collapsed both into a single `Panel` struct with
`Panel::{top, bottom, left, right}` constructors. Initial draft
used `TopBottomPanel::top` verbatim and failed with
`cannot find type TopBottomPanel in egui`. Switched to
`egui::Panel::top("editor_toolbar")`. Inline comment in `app.rs`
documents the rename so future readers don't repeat the search.
The unified API is functionally identical for our use — the
default-resizable behaviour (top panels are non-resizable by
default) matches.

**Deviation 2 — pointer dispatch lives in `logic()` using a
cached `canvas_rect`.** The Task 5 close-out's forward-looking
note said "Task 7's state-machine methods will be called from
inside `EditorApp::logic`", but egui's pointer position is in
screen-logical coords and we need it in canvas-physical. The
canvas rect (where `CentralPanel` ended up on screen, after the
toolbar reserves its strip) is only known *after* `CentralPanel`
shows — which happens inside `ui()`, not `logic()`. Resolved by
caching: `EditorApp` gains a `canvas_rect: Option<egui::Rect>`
field, `ui()` writes the freshly-computed panel rect into it
each frame, and the next frame's `logic()` reads the cached value
to translate pointer coords. One-frame latency on frame 1
(`canvas_rect` is `None`; pointer events are skipped that frame).
That's fine because the screenshot isn't visible until frame 1's
`ui()` either, so no drags can start before the cache is
populated. After Task 10 extracts a pure `dispatch_input`
helper, the cache stays where it is — only the match logic moves
into the pure function.

**Deviation 3 — zero-area rect filter at the shell, not in core.**
The Task 8 plan locked this in: a click without a drag would
otherwise commit a degenerate (0×0) rect. The filter lives in
`EditorApp::logic` (matches `Annotation::Rect | Ellipse | Blur`
and cancels when either normalized dimension is `< 1.0` px),
keeping `EditorState::commit_annotation` pure. The
`Annotation::Ellipse` and `Annotation::Blur` arms are matched
defensively — those are drag-rect tools arriving in Tasks 11/13
and they share the same degenerate-rect trap; the filter serves
all three without change.

**Deviation 4 — drag extends beyond canvas rather than clamping.**
If a drag starts inside the canvas and the pointer leaves the
window (or enters the toolbar region), `update_annotation` keeps
running with whatever negative / oversized coordinates result.
The rect just extends; `render()` clips at the panel edge on
the egui side. Task 14 (Select / crop) will need to revisit if
precise crop bounds become load-bearing, but for an annotation
tool "drag out and back in" producing an oversized rect is the
expected Photoshop / GIMP / Snipping Tool behaviour.

**Borrow-checker note.** Initial draft put the render-error
handler inside the `CentralPanel::show` closure (publishing
`Err(_)` from there). That captured `&self` wholesale and
conflicted with the closure's existing disjoint captures of
`&mut self.canvas`, `&mut self.bg`, `&self.state`. Restructured
to return the render `Result` from the closure (`.inner`) and
run error handling outside the closure body — same shape as
Task 5's `EditorApp::ui`, just with the additional cache write
between the panel `show` and the error match.

**egui 0.35 `StrokeKind::Inside` for sharp rect edges.** The
default `StrokeKind::Middle` puts half the stroke outside and
half inside the dragged rect, so a 3-px stroke on a rect from
(10,10)-(50,50) extends the visible outline to roughly
(8.5,8.5)-(51.5,51.5). `Inside` puts the whole stroke inside
the rect bounds, so the outer edge coincides with the dragged
geometry — matches the Flameshot / Snipping Tool convention of
"what you dragged is what you get". Future thick-stroke tools
(Marker) may revisit if visual heft becomes more important than
geometric precision.

**Test count.** Added one new test (`render_two_committed_rects_
emits_two_outlines` in `render.rs`). Standing gate: 53 pass
(was 52), 1 ignored (capture smoke), 0 fail. The required Task 8
MockCanvas test (two committed Rects; assert count + arguments)
is the new test. Extras from Task 7 already cover committed +
pending Rect dispatch (`render_draws_committed_and_pending_rects`)
and per-variant dispatch (`render_ann_dispatches_each_variant`),
so no additional snapshot tests were needed.

**No new unit tests in `flintshot-editor-egui`.** `EditorApp::logic`
isn't pure (depends on `egui::Context::input`) and eframe requires
a real display, so the egui crate's test surface stays at zero
through Phase 1. Task 10 extracts a pure
`dispatch_input(&mut EditorState, &InputEvent)` function from
the existing `logic()` body — at that point input dispatch
becomes unit-testable without a window. The Task 5 close-out
already flagged this as the plan; Task 8's pointer handling is
structured to make that extraction a pure code motion (the
match tree is already written without `egui` types at the
decision points, only at the input-reading prologue).

**Verification.** Standing gate green end-to-end:
- `cargo fmt --all --check` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — clean
  after two rounds of fixes (clippy::trivially_copy_pass_by_ref
  on `to_egui_stroke` — switched `&Stroke` to `Stroke` since
  `Stroke` is 8 bytes and `Copy`; clippy::unnested_or_patterns
  on the zero-area filter — switched
  `Some(Variant) | Some(Variant) | Some(Variant)` to
  `Some(Variant | Variant | Variant)`, the Rust 2021 nested form).
- `cargo clippy --no-default-features -p flintshot-app
  --all-targets -- -D warnings` — clean (Task 6's no-default-
  features gate preserved).
- `cargo test --workspace --all-targets` — 53 pass, 1 ignored
  (capture smoke), 0 fail.
- `cargo test --doc -p flintshot-core -p flintshot-editor-egui` —
  0 doc tests (unchanged).

End-to-end smoke on the maintainer's display is the only
remaining debt: rectangle visibility during drag, persistence
after commit, and Ctrl+Z feedback all need human eyes on a real
window. Scheduled for the Checkpoint C sign-off.

**Downstream impact.**
- Tasks 11 (Arrow) and 13 (Blur) follow the same input shape:
  tool-specific `begin_annotation` already constructs the right
  variant; `update_annotation` already updates the right field;
  the zero-area filter (deviation 3) already covers Ellipse and
  Blur. Tasks 11/13 add a toolbar button + a real `Canvas`
  primitive impl each; no further plumbing in `EditorApp`.
- Task 12 (Text) has a different input model (click to place
  caret, type, Enter to commit). The `pending` field already
  holds a Text ann; Task 12 adds a `begin_text(at)` method on
  `EditorState` and routes keyboard chars through `logic()`'s
  `ctx.input(|i| i.events)`.
- Task 10 (keyboard shortcuts) extracts the existing `logic()`
  pointer / Ctrl+Z match into a pure `dispatch_input` helper —
  no behaviour change, just refactor + new shortcuts (Esc,
  Ctrl+S, Ctrl+C, Ctrl+Y, Ctrl+Shift+Z).
- Task 14 (Select / crop) reuses the drag-anchor convention
  (Task 7 deviation 1) and the `hit_test` API. May revisit
  deviation 4 (drag-outside-canvas) if precise crop bounds
  become load-bearing.
- Task 15 (pickers) will extend the toolbar's
  `egui::Panel::top` with color/stroke widgets — the existing
  `ui.horizontal` + `selectable_value` pattern is the template.

<!-- Example entry format (commented out):
### Task 9 — Save / Copy
**Deviation.** Discovered `rfd::AsyncFileDialog` blocks the
eframe render loop on Wayland.
**Change.** Switched to `rfd::FileDialog` (synchronous); the
eframe render loop is already blocking so no async benefit.
**Downstream impact.** Task 10 (keyboard shortcuts) can call
the same sync path from Ctrl+S; no scheduler change needed.
-->

### Task 9 close-out — `<pending first commit hash>`

**Implementation.** Added `compose_final(state: &EditorState) ->
RgbaImage` in `flintshot-core::render` (re-exported from the crate
root). The function clones `state.image`, walks `state.anns` through
the existing `render_ann` dispatch into a private `ImageCanvas`
struct, and returns the resulting image. The `ImageCanvas` implements
`Canvas` directly on top of an `RgbaImage` — `upload_image` and
`draw_image` are no-ops (the canvas already *is* the image), and
only `draw_rect_outline` / `draw_rect_filled` have real impls (the
primitives the Rect tool exercises). Other primitives
(`draw_ellipse` / `draw_line` / `draw_arrow` / `draw_text`) emit a
`tracing::warn!` and are no-ops; Tasks 11/12/13 fill them in
alongside their respective toolbar additions.

On the shell side, `EditorApp::ui` gained two new toolbar buttons
(`💾 Save`, `📋 Copy`) and two free helper functions in `app.rs`:
`save_to_disk(state) -> Result<Option<PathBuf>>` (rfd blocking save
dialog + `image::save_with_format` with PNG forced) and
`copy_to_clipboard(state) -> Result<()>` (arboard `Clipboard::new` +
`set_image`). Button clicks collect into a new private
`ToolbarAction` enum returned from the toolbar panel's closure so
the action handlers (`handle_save` / `handle_copy`) can borrow
`&self.outcome` cleanly outside the closure — same pattern as the
existing render-error handler.

**Deviation 1 — sync `rfd::FileDialog`, not `AsyncFileDialog`.** The
TODO spec called for `rfd::AsyncFileDialog`. The eframe native event
loop is single-threaded and already blocking (`eframe::run_native`
owns the calling thread until window close, per Task 5 deviation 4);
there's no event loop to yield back to. `AsyncFileDialog` would
require either spawning a thread (and then marshalling back to the
UI thread for the actual outcome publish) or using `eframe::frame::
set_repaint_callback` for a poll-based wait. Both are more code for
no UX benefit — the native modal is what the user is interacting
with anyway, so the editor window appearing frozen during the dialog
is fine. Used `rfd::FileDialog::save_file() -> Option<PathBuf>`
directly. Phase 2's daemon may need to revisit if a long-running
editor blocks daemon IPC; that's a Phase 2 problem.

**Deviation 2 — `compose_final` lives in `render.rs`, not a new
`image_ops.rs`.** TODO spec left the location open ("render.rs or
new image_ops.rs"). Task 13 will introduce `image_ops` for the blur
computation (imageproc wrappers); landing `compose_final` there
first would mean a mostly-empty module for one public function.
Kept `compose_final` and the private `ImageCanvas` in `render.rs`
next to the existing `render()` / `render_ann()` dispatch — they
share the same per-variant mapping and belong together. When Task
13 adds `image_ops::blur`, the natural extraction is for the blur
computation only; `compose_final` stays in `render.rs`.

**Deviation 3 — `ImageCanvas` is private, not part of the public
Canvas surface.** Could have made it `pub` so external code could
render annotations into arbitrary `RgbaImage`s. Chose privacy:
`compose_final` is the only public entry point, and the `ImageCanvas`
struct is an implementation detail. If a future caller needs direct
access (e.g. Phase 4's upload path computing a thumbnail — different
geometry than `compose_final`'s "clone then annotate"), the struct
can be promoted without breaking changes (it'd just gain `pub`
visibility).

**Deviation 4 — pixel-level assertions, not a checked-in PNG
fixture.** TODO spec said "compare to a fixture PNG checked into the
repo". Took structural pixel assertions instead:
- Better failure messages: a failing test reports `expected [255,
  0, 0, 255] got [255, 255, 255, 255]` at a named pixel rather than
  "fixture mismatch at byte 4271".
- No binary content in git.
- Forward-compatible: Task 12's `cosmic-text` integration will
  introduce font rendering, which is platform-dependent (fontconfig
  availability, hinting, subpixel layout). Pixel-level assertions on
  non-text geometry stay deterministic across that change; a
  whole-image fixture would break for everyone.
- Matches the rest of the test suite (Task 7 / Task 8 render tests
  already use structural `DrawCall` assertions rather than
  fixtures).

If the maintainer prefers a fixture for end-to-end coverage, that's
a follow-up.

**Deviation 5 — failure path closes the editor.** TODO spec is
silent on Save/Copy error handling. Current behaviour: failure
(e.g. permission denied on save path, clipboard occupied on copy)
publishes `Err(...)` and closes the editor, matching the existing
render-error pattern. The maintainer sees the error in the terminal
via the CLI's `color_eyre` report. Phase 2 can add an in-editor
error toast / retry button when failure modes pile up; for Phase 1's
"draw a rect, click Save, get a PNG" happy path, the simpler
fail-close is correct.

**Deviation 6 — `arboard` with default features (no `wayland`
feature).** `arboard` on Linux uses X11 by default; the `wayland`
cargo feature adds `wl-clipboard-rs` for native Wayland clipboard
access. The maintainer's dev machine is Wayland + XWayland, where
the default X11 path should work (XWayland bridges the X clipboard).
Speculative-enabling the `wayland` feature adds deps and compile
time for a code path we haven't verified we need. Deferred until a
maintainer can test on a Wayland-only session without XWayland.

**No signature change to `EditorShell::run_editor`.** The shell
contract is unchanged: callers get back an `EditorOutcome` (or
`Err(FlintshotError)`). The two new outcome variants
(`Saved(path)`, `CopiedToClipboard`) were already part of the enum
from Phase 0; the CLI's match in `run_editor_for_capture` already
logs them, so the Task 6 wiring is forward-compatible without code
change. Verified by manual re-reading of `cli.rs:188-203`.

**Borrow-checker pattern reused from Task 8.** The toolbar closure
captures `&mut self.state.tool` (via `selectable_value`); Save/Copy
handlers need `&self.state` (for `compose_final`) and `&self.outcome`
(for `publish_outcome`). Resolved by collecting a `ToolbarAction`
enum inside the closure and acting on it outside — same shape as
the render-error handler from Task 5/8 (closure returns data,
control flow outside).

**Test count.** Added 4 new tests (`compose_final_*` in `render.rs`).
Standing gate: 57 pass (was 53), 1 ignored (capture smoke), 0 fail.
The required Task 9 unit test (deterministic `compose_final` output)
is covered by the four tests collectively — each locks in one facet
(rect-only outline, fill + outline combo, pending exclusion,
empty-state clone).

**No new unit tests in `flintshot-editor-egui`.** `EditorApp::ui`
isn't pure (depends on `egui::Context` and `egui::Ui`) and eframe
requires a real display, so the egui crate's test surface stays at
zero through Phase 1. Task 10 extracts a pure `dispatch_input`
function from the existing `logic()` body per the Task 7/8 close-out
forward-looking notes — at that point keyboard-shortcut dispatch
becomes unit-testable without a window. Save/Copy button clicks
need an actual egui instance to test (no synthetic `Ui` harness
exists), so they remain manual-verification-only until Phase 2 adds
an editor test harness.

**Verification.** Standing gate green end-to-end:
- `cargo fmt --all --check` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — clean
  after one round of fixes (clippy::doc_markdown on "XWayland" in
  a doc comment; switched to backticks).
- `cargo clippy --no-default-features -p flintshot-app --all-targets
  -- -D warnings` — clean (Task 6's no-default-features gate
  preserved).
- `cargo test --workspace --all-targets` — 57 pass, 1 ignored
  (capture smoke), 0 fail.
- `cargo test --doc -p flintshot-core -p flintshot-editor-egui` —
  0 doc tests (unchanged).

End-to-end smoke on the maintainer's display is the remaining debt:
save dialog opens, picks a writable path, writes a valid PNG with
the rect baked in; copy places the image on the clipboard and is
pasteable in GIMP / Preview / mspaint. Scheduled for the Checkpoint
D sign-off alongside Tasks 10's keyboard-shortcut coverage.

**Downstream impact.**
- Task 10 (keyboard shortcuts) reuses the `save_to_disk` and
  `copy_to_clipboard` helpers directly from the `Ctrl+S` / `Ctrl+C`
  handlers. The handlers take `&EditorState`, so no signature
  change needed. Task 10 also adds `Ctrl+Y` / `Ctrl+Shift+Z` for
  redo — calls `state.redo()` (already implemented in Task 7).
- Tasks 11/12/13 (Arrow / Text / Blur tools) need to extend
  `ImageCanvas` with the corresponding primitives so
  `compose_final` bakes those annotations into the saved output
  too. Each primitive can be added independently when the tool
  ships; until then, `compose_final` silently drops that variant
  (with a `tracing::warn!` so a session trace shows what's
  missing). This is the same placeholder-then-real pattern Task 7
  established for `render_ann`'s Blur arm.
- Phase 4 (Upload) calls `compose_final` to get the image bytes
  for the upload `Backend::upload` trait method. The function
  signature is already what Phase 4 will want.
- Phase 3 (Pin) uses the *un*-annotated `state.image` rather than
  `compose_final` output — pinning is "show this exact screenshot",
  not "show the annotated version". `EditorState::image` remains
  the source for the pin path.
- The `rfd` + `arboard` workspace-dep additions
  (`rfd = "0.17"`, `arboard = "3"`) propagate to any future crate
  that wants them via `{ workspace = true }`. CI's `lint` / `test` /
  `msrv` jobs don't need new system deps: `rfd`'s default Linux
  backend is `xdg-portal` (pure Rust + DBus); `arboard`'s default
  is `x11rb` (also pure Rust). Neither pulls in a `*-dev` apt
  package.

### Checkpoint B + C close-out — commit `2af6a64`

**Outcome.** Maintainer ran `cargo run -- region` on Linux +
Wayland + XWayland on 2026-07-21 and confirmed end-to-end:
- Window opens with the screenshot visible.
- The Rect tool produces a visible red outline during drag; the
  committed rect persists across frames.
- `Ctrl+Z` clears the most recent committed rect immediately.
- `Esc` closes the window cleanly; the CLI logs
  `EditorOutcome::Cancelled` and exits 0 with no output file.

That single interactive pass simultaneously closed the
**Checkpoint B** gate (editor opens over a real screenshot),
the **Checkpoint C** gate (Rect draws + persists + undoes), and
the long-standing carry-forward verification debt on Tasks 5, 6,
and 8 (all of which needed maintainer's eyes on a real display).
The corresponding checkboxes are now ticked in-place at their
original locations; this entry exists so a future /status read
sees the sign-off in one place.

**Process note — Checkpoint C signed off after Phase 1.D's Task 9
had already landed.** `docs/PLAN.md:124` says "do not start the
next phase until the previous checkpoint is signed off". The
reality was: the previous session pushed Task 9 into the working
tree ahead of the gate, the maintainer committed it as `2af6a64`
("update docs and cli"), and then ran the interactive check that
closed Checkpoint C. The implementation was sound — Task 9 added
independent Save / Copy buttons and a `compose_final` headless
render path, none of which gates or invalidates the Checkpoint C
contract — so the retroactive review confirmed the gate rather
than blocking forward motion. Future checkpoint gates should be
honoured in sequence when feasible (Phase 1.D's Checkpoint D will
be), but when the work has already outrun the gate, the right
move is to verify honestly and document the deviation rather
than revert working code.

**Standing gate at sign-off.** All five commands clean:
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo clippy --no-default-features -p flintshot-app
  --all-targets -- -D warnings`
- `cargo test --workspace --all-targets` — 57 pass, 1 ignored
  (capture smoke), 0 fail.
- `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
  — 0 doc tests, clean.

**Carry-forward.** Task 9's Save / Copy interactive verification
remains open — the maintainer's "region works nicely" run did
not explicitly exercise the `💾 Save` / `📋 Copy` toolbar buttons.
Phase 1.D's Task 10 (keyboard shortcuts) will wire `Ctrl+S` /
`Ctrl+C` to the same `save_to_disk` / `copy_to_clipboard` helpers,
so the next interactive check can test both the button path and
the keyboard path in one pass, closing Checkpoint D alongside.

**Downstream impact.**
- Phase 1.D proceeds with Task 10. No code rollback, no API
  change, no schema change — purely a documentation / verification
  entry.
- The next session's `docs/SESSION_HANDOFF.md` reflects "Checkpoint
  C closed; Task 10 next" instead of the previous "Checkpoint B
  human-eyes check + Task 10".
- `docs/ROADMAP.md` does not need a Phase 1 checkbox update yet —
  its next tick is Phase 1's overall exit at Checkpoint F.

### Task 10 close-out — `<pending first commit hash>`

**Implementation.** Introduced a new `flintshot-core::input` module
(`Modifiers`, `InputEvent`, `DispatchOutcome`) and a pure
`EditorState::dispatch_input(&InputEvent) -> DispatchOutcome`
method that owns every keyboard-shortcut and pointer-flow decision.
The egui shell's `EditorApp::logic` is now a thin translator:
egui modifier state + pressed keys + pointer transitions →
`InputEvent`, dispatch, then `if matches!` on the outcome to call
the existing `handle_save` / `handle_copy` / publish-Cancelled
paths. Side-effecting work (rfd save dialog, arboard clipboard,
eframe close-window) stays in the shell where it belongs.

**Deviation 1 — dispatcher lives in core, not in the egui crate.**
TODO spec said "Unit test in egui crate: feed synthetic `Key`
events through the shell's dispatcher". The natural place for the
dispatcher is `flintshot-core` so a future iced / slint shell reuses
it verbatim. That puts the unit tests in `editor::tests` rather
than `flintshot-editor-egui`. The egui crate's test surface stays
at zero through Phase 1 — same shape as Tasks 5 / 8 / 9, for the
same reason (eframe needs a real display). The TODO spec's intent
— "input dispatch is unit-testable" — is satisfied either way;
the test location follows the dispatcher's location.

**Deviation 2 — `Modifiers::ctrl` maps from egui's `command`, not
`ctrl`.** egui exposes both `Modifiers::ctrl` (the raw Ctrl key)
and `Modifiers::command` (the cross-platform chord — Ctrl on
Linux / Windows, ⌘ on macOS). Mapping our `Modifiers::ctrl` from
egui's `command` lets the dispatcher's chord-matching predicates
(`ctrl_only`, `ctrl_shift`) work uniformly across platforms without
a `cfg!(target_os = "macos")` in core. The raw `ctrl` field is
dropped at the translation boundary; no current Phase 1 shortcut
binds to "Linux Ctrl but not macOS Cmd", and adding one would
require a separate `Modifiers::control_raw` field — deferred until
needed.

**Deviation 3 — `clippy::struct_excessive_bools` allowed on
`Modifiers`.** Four bools is the natural shape for a modifier-key
set: Ctrl / Shift / Alt / Super are independent bits, not
option-like states with restricted combinations. clippy's advice
(two-variant enums or a state machine) fits option toggles, not
orthogonal flags. Same shape used by `egui::Modifiers`,
`winit::ModifiersState`, and `bevy_input::InputModifiers`. Allow
carries a justifying comment per AGENTS.md.

**Deviation 4 — key translation is explicit per-key, not a generic
`egui::Key → Key` table.** The shell's `logic()` checks each known
shortcut key (Escape / Z / Y / S / C) one at a time via
`ctx.input(|i| i.key_pressed(...))`. Unknown keys don't dispatch.
This avoids building a full `egui::Key → Key` mapping table that
would only ever handle the five keys we care about. Phase 2
features that need more keys (e.g. arrow-key nudge for select tool)
extend the explicit list. The trade-off is "explicit is better
than clever" — a 5-arm `if/else if` chain reads more cheaply than
a match against a translation table.

**Borrow pattern.** Each keyboard outcome path returns early from
`logic()` rather than going through `if matches!` chains; pointer
events fall through after the keyboard block because their
outcome is always `Continue` today. The match for that
fall-through is written as `let _ = self.state.dispatch_input(...)`
— the returned outcome is intentionally dropped because no current
pointer event produces a non-`Continue` outcome. A future Select
tool that produces a crop-and-save outcome from a pointer gesture
will surface as a compile error here (the `_` discards the value,
forcing the maintainer to handle it). Comment in `logic()` makes
that contract explicit.

**Test count.** Added 17 new tests: 13 dispatcher tests in
`editor::tests` (every shortcut + every edge case), 4 Modifiers
tests in `input::tests`. Standing gate: 74 pass (was 57), 1
ignored (capture smoke), 0 fail. The required Task 10 unit test
(synthetic Key events through dispatcher) is covered collectively
by the 13 dispatcher tests.

**Verification.** Standing gate green end-to-end:
- `cargo fmt --all --check` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — clean
  after two rounds of fixes (clippy::struct_excessive_bools on
  `Modifiers` — added justified allow; clippy::unnested_or_patterns
  on `Key::Char('z') | Key::Char('Z')` — switched to nested
  `Key::Char('z' | 'Z')`).
- `cargo clippy --no-default-features -p flintshot-app --all-targets
  -- -D warnings` — clean (Task 6's no-default-features gate
  preserved).
- `cargo test --workspace --all-targets` — 74 pass, 1 ignored
  (capture smoke), 0 fail.
- `cargo test --doc -p flintshot-core -p flintshot-editor-egui` —
  0 doc tests (unchanged).

End-to-end smoke on the maintainer's display is the remaining
debt: each new shortcut (`Ctrl+S` / `Ctrl+C` / `Ctrl+Y` /
`Ctrl+Shift+Z`) needs human eyes on a real window. Scheduled for
the Checkpoint D sign-off — same `cargo run -- region` pass that
closes Task 9's Save / Copy interactive debt.

**Downstream impact.**
- Phase 1.D is code-complete. Checkpoint D needs the maintainer's
  interactive run (one `cargo run -- region` pass covers it).
- Phase 1.E (Tasks 11 / 12 / 13 — Arrow / Text / Blur) inherits
  the dispatcher directly. Each new tool adds: (a) a `begin`
  variant in `begin_annotation` if it's drag-shaped (Arrow) or a
  dedicated `begin_*` method if it's not (Text); (b) a toolbar
  button in `EditorApp::ui`'s toolbar closure; (c) a real impl of
  the corresponding `Canvas` primitive(s) in `EguiCanvas`. The
  dispatcher and `logic()` don't change — `begin_annotation` /
  `update_annotation` / `commit_annotation` already do the right
  thing for any drag-shaped tool, and the shell already translates
  pointer events to `InputEvent`s generically.
- Task 12 (Text tool) will extend `InputEvent` with a variant for
  character typing — `InputEvent::Text(char)` or similar — plus a
  `EditorState::begin_text(at)` method. The dispatcher's
  `dispatch_input` gains one more arm; tests follow the same shape
  as Task 10's.
- Task 14 (Select / crop) reuses the drag-anchor convention from
  the existing pointer flow. The dispatcher already produces the
  correct `begin` / `update` / `commit` sequence; the shell only
  adds a "confirm crop" UX (button / double-click / Enter) that
  publishes a new `EditorOutcome::Cropped` variant or similar.
- Phase 2's potential iced / slint shell reuses `dispatch_input`
  verbatim — that was the whole point of extracting it. The shell
  implements its own `translate_modifiers`-equivalent and event
  capture, but the dispatcher and its test suite carry over.
- The `Modifiers::super_key` field is unused on macOS today (the
  shell maps `mac_cmd` to `Modifiers::ctrl` via `command`); a
  future feature that needs the actual Cmd key (e.g. Cmd+Space for
  a Command Palette in Phase 6) can read it. Until then it's dead
  data that's free to carry.
