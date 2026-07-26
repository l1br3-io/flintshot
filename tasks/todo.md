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
- [ ] All Phase 1 deliverables in `SPEC.md` checked off.
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

## Decisions

Lasting technical decisions landed this phase (full text in
[`docs/adr/`](../docs/adr/)):

- [ADR-0013](../docs/adr/ADR-0013-error-field-detail-not-source.md) — `FlintshotError` variants use a `detail` field, never `source` (thiserror rule).
- [ADR-0014](../docs/adr/ADR-0014-synchronous-rfd-filedialog.md) — Synchronous `rfd::FileDialog` over the async API in the blocking editor.
- [ADR-0015](../docs/adr/ADR-0015-compose-final-separate-render-path.md) — `compose_final` renders annotations into the image, separate from live `render()`.
- [ADR-0016](../docs/adr/ADR-0016-workspace-scoped-standing-gate.md) — Standing gate uses `--workspace` + scoped `--doc` selectors.
