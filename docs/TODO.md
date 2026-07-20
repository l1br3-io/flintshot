# TODO — Phase 1: Capture + Editor Window

> Rolling task list for the current phase. Overwritten at the start of
> each phase; never appended across phases. Per-task detail for the
> plan in [`PLAN.md`](PLAN.md). Check off items by completing the
> acceptance criteria and verification steps, not by writing code.

Conventions:
- **Acceptance criteria** answer "did we build the right thing?" for
  this task. All must be checked off.
- **Verification** is the runtime proof. The standing gate (`fmt`,
  `clippy`, `cargo test --all-targets`, `cargo test --doc`) applies to
  every task; per-task verification is what's additionally run.
- **Dependencies** refer to task numbers in this file.
- **Size**: XS (1 file) · S (1–2 files) · M (3–5 files) · L (split).

Standing gate (run before marking any task done):

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo test --doc
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
- [ ] `Config::load()` reads `~/.config/flintshot/config.toml` (or
      platform equivalent via `dirs::config_dir()`); returns
      `Ok(Config::default())` when the file is missing.
- [ ] `Config::save()` writes the config back without clobbering
      comments (verified roundtrip on a fixture file).
- [ ] All public types have doc comments (`#![warn(missing_docs)]`).
- [ ] Unknown fields in the TOML are ignored, not errors.
- [ ] `Color` serializes as `"#rrggbb"` (or `"#rrggbbaa"`) hex string,
      not four integers. Add a `serde` helper module.

**Verification:**
- [ ] Standing gate passes.
- [ ] `cargo test -p flintshot-core config::` — unit tests cover:
      missing-file defaults, roundtrip with comment preservation,
      hex-color roundtrip, unknown-field tolerance.
- [ ] Manual: `cargo run -- config` (new CLI verb, see Task 3 wiring)
      prints the loaded config; or temporarily add a `--dump-config`
      debug flag.

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
- [ ] `capture_fullscreen(None)` returns the primary monitor's image
      with `.dimensions()` matching the monitor's physical size.
- [ ] `capture_region` crops the supplied `Rect` (physical px) out of
      the full capture; rejects empty / negative rects with
      `FlintshotError::InvalidRect` (new variant).
- [ ] `FlintshotError` gains `Capture { source: String }` and
      `InvalidRect` variants; both documented.
- [ ] No `unwrap`/`expect`; all `xcap` fallible calls mapped to
      `FlintshotError`.
- [ ] macOS permission failure surfaces as `FlintshotError::Capture`
      with a message containing "permission" (so Task 5 can detect and
      guide).

**Verification:**
- [ ] Standing gate passes.
- [ ] `#[ignore]`'d integration test `capture::smoke_can_grab_primary`
      in `crates/flintshot-core/tests/capture.rs` — requires a display;
      listed in `TEST_MATRIX.md`.
- [ ] Unit test `capture::invalid_rect_rejected` runs headless and
      verifies the error path without calling xcap.

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
- [ ] `flintshot region --out /tmp/foo.png` writes a real PNG of the
      primary monitor (or the selected region, if Phase 1.F's crop tool
      is being demoed early — out of scope here, just do fullscreen-crop
      to a sane default).
- [ ] `flintshot fullscreen --monitor 0 --out /tmp/foo.png` writes the
      PNG of monitor 0.
- [ ] `flintshot config` prints the loaded TOML to stdout.
- [ ] Failure paths (no display, write permission denied) return
      `color_eyre` reports with context, not raw panics.
- [ ] Default `--out` filename includes an ISO-8601 timestamp.

**Verification:**
- [ ] Standing gate passes.
- [ ] Manual: `cargo run -- region --out /tmp/x.png && file /tmp/x.png`
      reports `PNG image data, ...`.
- [ ] Manual: `cargo run -- fullscreen --monitor 99` fails cleanly
      with a readable error.
- [ ] Manual: `cargo run -- config` prints valid TOML.

**Dependencies:** Task 1, Task 2.

**Files likely touched:**
- `crates/flintshot-app/src/cli.rs` (real bodies; new `config` verb;
  `--out` flag)
- `crates/flintshot-app/Cargo.toml` (no new deps unless `time`/`chrono`
  for timestamps — prefer `std::time` + formatting helper)

**Size:** S

---

### Checkpoint A — capture stack proven end-to-end

- [ ] Standing gate green.
- [ ] `flintshot region --out /tmp/x.png` writes a valid PNG on at
      least one platform from `TEST_MATRIX.md` (L1 or M1).
- [ ] `flintshot config` round-trips a real TOML.
- [ ] No GUI code added (no `eframe`, `egui` calls yet).
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
- [ ] `EguiCanvas::upload_image` loads via `ctx.load_texture` with
      `TextureFilter::Nearest` (sharp pixels matter for screenshots).
- [ ] `EguiCanvas::draw_image` blits at the requested `Rect` (physical
      px → logical px conversion done in the canvas).
- [ ] `flintshot-core::render::render()` uploads once (cached),
      re-blits each frame, returns `Ok(())`.
- [ ] No GUI types leak: the `render()` free function is in core and
      takes `&mut impl Canvas`; it never names `egui`.

**Verification:**
- [ ] Standing gate passes (including the `no_gui_types_in_core`
      integration test).
- [ ] Unit test in `render.rs`: with an empty `EditorState`, the
      `MockCanvas` (added in Task 7) records exactly one
      `draw_image` call. *(If Task 7 hasn't landed yet, defer this
      assertion to Task 7 and instead assert via a temporary inline
      mock.)*

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
- [ ] `EguiShell::run_editor(state)` opens an `eframe::Frame` window
      sized to the screenshot dimensions (clamped to viewport).
- [ ] Screenshot displayed via the `render()` shared path; visible on
      open.
- [ ] `Esc` closes the window and the function returns
      `EditorOutcome::Cancelled`.
- [ ] Window-close button (X) also returns `Cancelled`.
- [ ] Window title is `"flintshot — annotate"` (or similar).

**Verification:**
- [ ] Standing gate passes.
- [ ] Manual (TEST_MATRIX row L1 or M1): `cargo run -- region` (after
      Task 6 wires it) opens the window with the screenshot visible;
      Esc closes it; terminal prints `EditorOutcome::Cancelled`.

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
- [ ] `flintshot region` opens the editor (no `--out`).
- [ ] `flintshot region --out PATH` skips the editor and writes the
      PNG directly (Task 3's path preserved).
- [ ] When the editor returns `EditorOutcome::Saved(p)`, the CLI logs
      the path.
- [ ] When the editor returns `Cancelled`, the CLI exits cleanly with
      no output file.
- [ ] `cfg(not(feature = "egui"))` build still works: `region` without
      `--out` errors clearly ("editor not built; pass --out").

**Verification:**
- [ ] Standing gate passes (including a `--no-default-features` build).
- [ ] Manual: `cargo run -- region` opens editor, Esc cancels, no file
      written, clean exit code 0.
- [ ] Manual: `cargo run --no-default-features -- region --out /tmp/x.png`
      still works.

**Dependencies:** Task 3, Task 5.

**Files likely touched:**
- `crates/flintshot-app/src/cli.rs`

**Size:** S

---

### Checkpoint B — editor opens over a real screenshot

- [ ] Standing gate green.
- [ ] `flintshot region` opens an egui window showing the captured
      screenshot on at least one platform from `TEST_MATRIX.md`.
- [ ] Esc closes cleanly; no panics, no leaked GPU resources.
- [ ] **Review with human before starting Phase 1.C.**

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
- [ ] `EditorState::pending: Option<Annotation>` field added; documented.
- [ ] `begin_annotation` / `update_annotation` / `commit_annotation`
      drive `pending` correctly; commit pushes a snapshot of `anns`
      onto `undo`, clears `redo`, appends to `anns`.
- [ ] `undo` / `redo` move `anns` snapshots correctly; `undo` with
      empty history is a no-op; `redo` after a new commit is a no-op.
- [ ] `cancel_annotation` drops `pending`.
- [ ] `render()` draws `pending` on top of `anns`.
- [ ] `MockCanvas` records every `Canvas` call into a public
      `Vec<DrawCall>`; `DrawCall` is an enum with one variant per
      Canvas method (with arguments).
- [ ] Hit-testing helper `EditorState::hit_test(point: Point) ->
      Option<usize>` returns the topmost annotation under the cursor
      (used in Phase 1.F's select tool).

**Verification:**
- [ ] Standing gate passes.
- [ ] Unit tests in `editor.rs`:
  - `undo_redo_snapshot_roundtrip` — push 3 anns, undo twice, redo
     once, verify exact `anns` contents at each step.
  - `commit_clears_redo` — committing after an undo wipes the redo
     stack.
  - `cancel_drops_pending` — pending is None after cancel.
- [ ] Unit test in `render.rs` using `MockCanvas`: render an
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
- [ ] With `Tool::Rect` active, mouse-down begins a rect annotation
      with the current color/stroke; mouse-drag updates the opposite
      corner; mouse-up commits.
- [ ] The in-progress rect is visible during drag (via `pending`).
- [ ] Committed rect persists in `anns` across frames; visible after
      next render.
- [ ] `Ctrl+Z` undoes the last commit (visible immediately).
- [ ] Toolbar shows at least one button (Rect) and clicking it sets
      `state.tool`.
- [ ] Clicking outside the canvas area doesn't begin an annotation.
- [ ] No `egui::Painter` calls in the egui crate for the rect — only
      `Canvas::draw_rect_*`.

**Verification:**
- [ ] Standing gate passes.
- [ ] `MockCanvas` test: render an `EditorState` with two committed
      `Annotation::Rect`s; assert recorded `draw_rect_outline` count
      and arguments.
- [ ] Manual: `cargo run -- region`, draw a red rectangle, see it
      persist, press Ctrl+Z, see it disappear.

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

- [ ] Standing gate green.
- [ ] User can draw a red rectangle over a screenshot, see it persist
      across frames, and undo it with `Ctrl+Z`.
- [ ] `MockCanvas` snapshot tests lock in the render output.
- [ ] **Review with human before starting Phase 1.D.**

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
- [ ] New core function `compose_final(state: &EditorState) ->
      RgbaImage` renders `anns` into a clone of `state.image` (this is
      the source of truth for save/copy — not a screenshot of the
      egui canvas).
- [ ] `flintshot-editor-egui` exposes Save and Copy buttons (no
      keyboard wiring yet — Task 10).
- [ ] Save opens `rfd` async dialog; PNG written to chosen path.
- [ ] Copy puts the image on the clipboard and is pasteable in another
      app (verified manually).
- [ ] `EguiShell::run_editor` returns the correct `EditorOutcome`
      variant on each action and closes the window.

**Verification:**
- [ ] Standing gate passes.
- [ ] Unit test: `compose_final` on a known `EditorState` produces a
      deterministic image (compare to a fixture PNG checked into the
      repo).
- [ ] Manual: draw a rect, click Save, choose a path, verify file
      opens in an image viewer and shows the rect baked in.
- [ ] Manual: draw a rect, click Copy, paste in GIMP / Preview /
      mspaint; verify rect is visible.

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
- [ ] `Esc` cancels: drops pending, returns `EditorOutcome::Cancelled`.
- [ ] `Ctrl+S` triggers Save (same path as Task 9's button).
- [ ] `Ctrl+C` triggers Copy (same path as Task 9's button).
- [ ] `Ctrl+Z` undoes last commit.
- [ ] `Ctrl+Y` (and `Ctrl+Shift+Z`) redoes last undo.
- [ ] Shortcuts are documented in `docs/TEST_MATRIX.md` Phase 1 table
      for manual verification.

**Verification:**
- [ ] Standing gate passes.
- [ ] Manual: each shortcut tested end-to-end on L1 or M1.
- [ ] Unit test in egui crate: feed synthetic `Key` events through
      the shell's dispatcher; assert state machine transitions.

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

<!-- Example entry format (commented out):
### Task 8 — Rectangle tool end-to-end
**Deviation.** Discovered egui 0.35's `Painter` API doesn't expose
stroke-line-join the way the ARCHITECTURE doc assumed.
**Change.** Added a `StrokeJoin` field to the core `Stroke` struct;
updated MockCanvas assertions accordingly.
**Downstream impact.** Task 11 (Arrow) inherits the new field
automatically; Task 12 (Text) unaffected.
-->
