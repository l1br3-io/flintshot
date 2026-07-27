# Changelog

All notable changes to `flintshot` are recorded here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Version `0.x.y` is in effect during development — no API stability
guarantees until `1.0.0` (see `docs/ROADMAP.md`).

Categories used below:

- **Added** — new features, crates, subcommands, public types
- **Changed** — changes to existing functionality
- **Deprecated** — soon-to-be-removed features
- **Removed** — removed features
- **Fixed** — bug fixes
- **Security** — vulnerability fixes

## [Unreleased]

### Added

- Workspace scaffolding with four crates: `flintshot-app` (bin, default),
  `flintshot-core` (lib), `flintshot-editor-egui` (lib),
  `flintshot-cosmic` (bin, opt-in).
- `flintshot` CLI surface (`clap`) with subcommands `daemon`, `region`,
  `fullscreen`, `pin`, `upload` — all stubs, tagged with their target
  phase in `--help`.
- `flintshot-core` public API: geometry types (`Point`, `Rect`, `Color`,
  `Stroke`, `Key`), `Annotation` enum, `EditorState`, `Tool`, `BlurKind`,
  the `Canvas` and `EditorShell` traits, and the `UploadBackend` trait
  with supporting types (`ImageMime`, `UploadContext`, `UploadOutcome`,
  `UploadError`).
- `flintshot-core::config`: `Config`, `UiConfig`, `CaptureConfig` types
  with `Config::load()` / `Config::save()` reading and writing
  `<config_dir>/flintshot/config.toml`. Comment-preserving writes via
  `toml_edit`; partial files fall back to documented defaults.
- `flintshot-core::capture`: `capture_fullscreen(monitor)` and
  `capture_region(rect, monitor)` wrapping `xcap`. Captures return
  physical-pixel `RgbaImage`s; HiDPI conversion happens at the shell
  boundary. macOS permission denials surface as
  `FlintshotError::Capture { source }` whose `source` contains the
  substring `"permission"` so callers can detect & guide.
- `flintshot-core::serde_util::color_hex` — `Color` now serializes as
  `#rrggbb` (opaque) or `#rrggbbaa` (with alpha) everywhere it appears
  (config, IPC, annotations). Reading accepts both forms with optional
  leading `#`, any case.
- New `FlintshotError::Config { path, detail }` variant for config
  parse / serialize failures.
- New `FlintshotError::Capture { source }` variant for `xcap` failures
  (permission denied, monitor out of range, no monitors available).
- New `FlintshotError::InvalidRect { rect, reason }` variant for
  degenerate rects passed to `capture_region` (`reason` is `"empty"`
  or `"negative"`).
- Hidden throwaway `flintshot --dump-config` flag that prints the
  resolved TOML (removed in Phase 1 Task 3 when the real `config`
  subcommand lands).
- `flintshot config` subcommand: prints the resolved configuration
  as TOML to stdout (Phase 1 Task 3). Replaces the throwaway
  `--dump-config` flag.
- `flintshot region --out PATH` and `flintshot fullscreen --out PATH`
  flags (Phase 1 Task 3): capture and write a PNG to `PATH`. Default
  when `--out` is omitted is `/tmp/flintshot-<ISO-8601>Z.png` (e.g.
  `/tmp/flintshot-20260721T143022Z.png`).
- Real `run_region` / `run_fullscreen` bodies backed by
  `flintshot_core::capture_fullscreen` (Phase 1 Task 3). The CLI is
  now a working one-shot capture tool; editor wiring follows in
  Task 6.
- `flintshot-core::render::render()` now uploads the background
  screenshot on first frame, caches the texture handle, and blits
  to the full image rect on every subsequent frame (Phase 1 Task 4).
  Annotation dispatch (Tasks 8/11/12/13) will be appended after
  this background-blit step.
- `flintshot-editor-egui::EguiCanvas::upload_image` /
  `draw_image` real implementations (Phase 1 Task 4). Upload goes
  through `egui::Context::load_texture` with
  `TextureOptions::NEAREST` for pixel-sharp screenshots; blit
  converts physical → logical px at `pixels_per_point`. The canvas
  now carries a stable `egui::Context` plus a per-frame `Painter`
  bound by the embedding shell via `EguiCanvas::bind_painter`.
- `flintshot-editor-egui::EguiShell::run_editor` real
  implementation (Phase 1 Task 5): opens a single-window eframe
  app sized to the screenshot, runs the shared `render()` path
  each frame, and returns `EditorOutcome::Cancelled` on `Esc` or
  window-close-via-X. Window title `"flintshot — annotate"`.
  Internal `EditorApp` (`eframe::App`) lives in the new `app.rs`
  module.
- New `FlintshotError::Shell { detail }` variant for editor-shell
  lifecycle failures (eframe startup, GPU surface init, etc.).
  Sits alongside `Capture` / `Config` and follows the same
  `detail: String` field convention.
- `flintshot-editor-egui::EguiShell::run_editor` real
  implementation (Phase 1 Task 5). Opens an `eframe` window titled
  `"flintshot — annotate"`, sized to the screenshot dimensions,
  displays the screenshot via the shared `render()` path, and
  returns `EditorOutcome::Cancelled` when the user presses `Esc`
  or closes the window. Outcome is propagated through a shared
  `Rc<RefCell<Option<Result<EditorOutcome, FlintshotError>>>>`
  slot because `eframe::run_native` consumes the `Box<dyn App>`
  and fields cannot be recovered by value after the event loop
  ends. `Rc<RefCell<…>>` (not `Arc<Mutex<…>>`) because eframe's
  native event loop is strictly single-threaded.
  `flintshot-editor-egui` now has a 3-module layout:
  `canvas.rs` (`EguiCanvas`, unchanged from Task 4), `app.rs`
  (private `EditorApp` `eframe::App` impl), and `shell.rs`
  (`EguiShell` `EditorShell` impl + `run_editor_blocking`
  helper). `lib.rs` is now a thin module-export file.
  Annotation tools, toolbar, and keyboard shortcuts land in
  Tasks 7+ — Task 5 is the "de-risk egui integration" slice.
- New `FlintshotError::Shell { detail }` variant for editor-shell
  / window-system failures (Phase 1 Task 5). Currently wraps
  `eframe::Error` via `to_string()`; will be reused for Task 9's
  `rfd` / `arboard` failures and Phase 3's pin window.
- `flintshot region` / `flintshot fullscreen` now route through the
  `EguiShell` editor when `--out` is omitted (Phase 1 Task 6). Capture
  → `EditorShell::run_editor(state)` → log the outcome. The editor
  path is gated on the `egui` Cargo feature; `--no-default-features`
  builds reject `region` / `fullscreen` without `--out` with a clear
  error ("editor not built; pass --out PATH to capture headlessly").
  Outcome logging: `Saved(p)` logs the path, `Cancelled` logs and
  exits cleanly with no output file, `CopiedToClipboard` / `Uploaded`
  are wired for Tasks 9 / 4, and the `#[non_exhaustive]` wildcard arm
  warns on future variants.
- `flintshot-core::EditorState` gains a `pending: Option<Annotation>`
  field plus the state-machine methods `begin_annotation`,
  `update_annotation`, `commit_annotation`, `cancel_annotation`,
  `undo`, `redo`, and `hit_test` (Phase 1 Task 7). The pending
  annotation holds the in-flight drag-based tool output; the shared
  `render()` path now draws it on top of `anns` so the user sees live
  feedback. `undo()` clears `pending` (Ctrl+Z mid-drag cancels the
  in-flight stroke, matching typical editor semantics).
- `flintshot-core::Annotation` gains `bounding_box() -> Option<Rect>`
  and `contains(point: Point) -> bool` methods (Phase 1 Task 7).
  These underpin `EditorState::hit_test`, which returns the topmost
  annotation under the cursor — used in Phase 1.F's select tool.
  `contains` uses stroke-width-aware tolerances so thin lines remain
  selectable.
- `flintshot-core::render` now exposes the public `MockCanvas` struct
  and `DrawCall` enum (Phase 1 Task 7) for snapshot tests across the
  annotation tools. `DrawCall` has one variant per `Canvas` method;
  `MockCanvas` records every call into a public `Vec<DrawCall>`. The
  inline `#[cfg(test)]`-only `MockCanvas` from Task 4 is replaced by
  this public version.
- `flintshot-core::render::render()` now iterates `state.anns` and
  dispatches each annotation via the new public `render_ann(canvas,
  ann)` helper (Phase 1 Task 7). `state.pending` is drawn last (on
  top) so the user sees live feedback during drags. All 9 annotation
  variants have a dispatch path; `Blur` renders as a grey placeholder
  + `tracing::warn!` until Task 13 adds real `image_ops::blur`.
- Rectangle annotation tool end-to-end (Phase 1 Task 8):
  `Tool::Rect` is now interactive. Mouse-down inside the canvas
  begins a rect annotation with the active color/stroke; drag updates
  the opposite corner; release commits. An editor toolbar
  (`egui::Panel::top`) appears at the top of the window with a Rect
  tool selector (`selectable_value`) and an annotation-count label.
  `Ctrl+Z` undoes the last commit and clears any in-flight pending
  rect. Click-without-drag is filtered at the shell boundary (cancel
  on degenerate rect rather than commit). `EguiCanvas::draw_rect_outline`
  and `EguiCanvas::draw_rect_filled` now have real egui `Painter`
  implementations using `StrokeKind::Inside` (so the stroke's outer
  edge coincides with the dragged rect — "what you dragged is what
  you get"). Other `Canvas` primitives (`draw_ellipse`, `draw_line`,
  `draw_arrow`, `draw_text`) remain stubs until Tasks 11/12/13.
- Save / Copy output paths (Phase 1 Task 9). Two new toolbar buttons
  in the editor (`💾 Save`, `📋 Copy`) compose the final image
  (background + all committed annotations) and either write a PNG
  via `rfd::FileDialog` (blocking save dialog + `image::save_with_format`
  with PNG forced) or place it on the system clipboard via
  `arboard::Clipboard::set_image`. On success the editor publishes
  the matching `EditorOutcome` variant (`Saved(path)` / `CopiedToClipboard`)
  and closes; on failure it publishes `Err(FlintshotError)` and
  closes (matching the existing render-error pattern). Dialog cancel
  keeps the editor open with no outcome published. Two new workspace
  deps: `rfd = "0.17"`, `arboard = "3"` (default features; X11 on
  Linux, native elsewhere).
- New `flintshot-core::render::compose_final(state: &EditorState)
  -> RgbaImage` (Phase 1 Task 9). Clones `state.image` and renders
  every committed annotation into the clone via the shared
  `render_ann` dispatch — the same path the live editor uses, so
  what the user sees is what gets saved / copied. No toolkit-specific
  "screenshot the canvas" path is involved. Excludes `state.pending`
  (in-flight annotations are not part of the saved output). The
  function backs the Save / Copy buttons and is the source of truth
  for Phase 4's Upload path.
- New `flintshot-core::input` module (Phase 1 Task 10) with three
  public types: `Modifiers`, `InputEvent`, `DispatchOutcome`. The
  shell translates its native input events into `InputEvent`s at the
  toolkit boundary; `EditorState::dispatch_input` walks them and
  returns a `DispatchOutcome` naming the side-effect the shell
  should perform (Save / Copy / Cancel). The dispatcher is pure and
  unit-testable — no `egui::Context` or `eframe` types reach it —
  so a future `iced` / `slint` shell reuses it verbatim. Includes
  the cross-platform modifier mapping: egui's `Modifiers::command`
  (Ctrl on Linux / Windows, ⌘ on macOS) maps onto `Modifiers::ctrl`
  so the dispatcher's chord predicates work uniformly without
  `cfg!(target_os = "macos")` in core.
- Keyboard shortcuts now route through `EditorState::dispatch_input`
  (Phase 1 Task 10): `Esc` (cancel), `Ctrl+Z` (undo), `Ctrl+Y` and
  `Ctrl+Shift+Z` (redo), `Ctrl+S` (save), `Ctrl+C` (copy). The
  pointer flow (down → drag → up) also routes through the
  dispatcher; the zero-area rect filter from Task 8's "deviation 3"
  moved into core so all shells inherit it without duplicating the
  predicate. Existing `EditorApp::logic` is now a thin translator:
  egui input → `InputEvent`, dispatch, `if matches!` on outcome.
- Arrow annotation tool end-to-end (Phase 1 Task 11). Toolbar gains an
  "→ Arrow" button; drag from tail to head commits an
  `Annotation::Arrow` (tail = down point, head = up point, endpoints
  not normalized — direction matters). `head_size` defaults to 10
  physical px; the shaft inherits `state.stroke_width`. New shared
  `flintshot_core::arrow_barbs(a, b, head_size) -> [Point; 2]`
  computes the ±30° barb endpoints so `EguiCanvas` (live view) and
  `ImageCanvas` (`compose_final` → Save / Copy) render identical
  arrows (ADR-0015 WYSIWYG). `ImageCanvas::draw_line` filled in
  alongside; degenerate arrows (sub-px drag) cancel on pointer-up,
  matching the Rect tool's click-without-drag contract.
- Text annotation tool end-to-end (Phase 1 Task 12). Toolbar gains a
  "T Text" button; click places a caret, typing appends to an
  in-flight `Annotation::Text` draft, `Enter` commits, `Esc` cancels
  the draft (stays in editor), `Backspace` deletes the last char. Empty
  drafts never commit. While editing, keyboard shortcuts (`Ctrl+S`,
  `Ctrl+Z`, etc.) are suppressed so the user can type freely; pointer
  events are ignored until the draft is committed or cancelled (Phase 1
  text editing is modal). New `InputEvent::TextInput(char)` carries
  printable-char input from the shell (`egui::Event::Text`) into the
  dispatcher. Multi-line text is deferred to Phase 2.
- `ab_glyph` + `fontdb` added to `flintshot-core` (Phase 1 Task 12).
  `ImageCanvas::draw_text` rasterises text annotations into the saved /
  copied PNG via `ab_glyph` glyph outlines + source-over alpha
  compositing; `fontdb` discovers a system sans-serif face via
  fontconfig (cached in a process-wide `OnceLock`). See ADR-0017 for
  the swap from the originally-specced `cosmic-text` (shipped `ab_glyph`
  instead to keep the highest-risk Phase 1 task achievable — the plan
  pre-authorised this fallback). `EguiCanvas::draw_text` uses egui's
  native `Painter::text` (no new egui-side dep).
- Blur / pixelate annotation tool end-to-end (Phase 1 Task 13).
  Toolbar gains a "░ Blur" button with a Gaussian / Pixelate
  `BlurKind` toggle (visible only while the Blur tool is active). Drag
  selects a region; on commit, `flintshot-core::image_ops::blur` crops
  the region from `state.image`, applies either
  `imageproc::filter::gaussian_blur_f32` (Gaussian, sigma = strength)
  or a block-average (Pixelate, block = strength), and blits the
  result back over the region. New `Canvas::draw_image_region(img,
  dest)` primitive carries the freshly-computed sub-image from core to
  the shell (egui uploads a uniquely-named texture; `ImageCanvas` copies
  pixels in-place). `render_ann` now takes a `source: &RgbaImage`
  parameter so the blur dispatch can crop from the background. New
  `flintshot-core::image_ops` module re-exports `blur` and `crop_region`.
- Select / crop region tool end-to-end (Phase 1 Task 14). Toolbar gains
  a "⬚ Select" button; drag selects a region (rendered with a distinct
  blue outline so it's distinguishable from a committed `Rect`), and
  `Enter` or a "✂ Crop to selection" toolbar button crops `state.image`
  to the selection. Existing annotations translate by subtracting the
  crop origin; annotations fully outside the crop are dropped;
  rect-shaped annotations (`Rect` / `Ellipse` / `Blur`) that partially
  overlap are geometrically clipped to the overlap. Crop is undoable:
  the pre-crop image + anns are captured in a `Snapshot::full` on
  `undo`, and `undo()` / `redo()` restore the image when the snapshot
  carries one. New `Rect::intersect` / `Rect::contains` geometry
  helpers, `Annotation::crop_and_translate` per-variant dispatch, and
  `EditorState::selection: Option<Rect>` field for the in-flight drag.
- Shared error types: `FlintshotError`, `Result` (libs); `color_eyre`
  reporting init in the binary.
- `tracing` + `tracing-subscriber` initialized from
  `RUST_LOG`/`EnvFilter`.
- Integration test pinning the boundary: no GUI toolkit types in
  `flintshot-core`'s public API
  (`crates/flintshot-core/tests/no_gui_types_in_core.rs`).
- CI pipeline (`.github/workflows/ci.yml`): `fmt --check`, `clippy
  -D warnings`, `cargo test`, MSRV (1.95) build, opt-in
  `flintshot-cosmic` build job.
- Project documentation: `README.md`, `docs/VISION.md`,
  `docs/ARCHITECTURE.md`, `docs/ROADMAP.md`, `docs/adrs/`
  (ADRs 0001–0012), `CONTRIBUTING.md`, `AGENTS.md`, `docs/TEST_MATRIX.md`.
- Dual licensing: `MIT OR Apache-2.0`, with both license files in the
  repo root and the `license` field on every crate.

### Changed

- Planning docs reorganized to the canonical OpenCode harness v0.5.0+
  layout. Root `SPEC.md`, `tasks/plan.md`, `tasks/todo.md`, and
  `tasks/archive/` are eliminated; in their place, each effort lives at
  `docs/specs/<effort-slug>/` with `spec.md` (frontmatter
  `status: draft|active|shipped`), thin `plan.md`, flat `todo.md`, and
  per-task `tasks/00N-*.md` files. Shipped Phase 0 →
  `docs/specs/phase-0-scaffolding/` (status: shipped); in-flight Phase 1
  → `docs/specs/phase-1-capture-editor/` (status: active). `docs/adr/`
  renamed to `docs/adrs/`. `docs/ROADMAP.md` "How we plan" now describes
  the status-flip lifecycle (no archive step). Links in `README.md`,
  `CONTRIBUTING.md`, and `AGENTS.md` updated to the new paths.
- `EditorState::undo` / `redo` fields changed from
  `Vec<Vec<Annotation>>` to `Vec<Snapshot>` (Phase 1 Task 14). The new
  `Snapshot` type captures both `anns` and an optional `image`, so
  crop (which mutates the source image) is undoable. Annotation-only
  commits carry `image: None` (no extra clone); crop carries
  `image: Some(pre_crop)`. Callers that pushed raw `Vec<Annotation>`
  onto `undo` / `redo` must switch to `Snapshot::anns_only(vec)` or
  `Snapshot::full(vec, img)`.
- `EditorState::selection` field renamed to `selected_index` (Phase 1
  Task 14). The old `selection: Option<usize>` held the hit-tested
  annotation index; it collided with the new `selection: Option<Rect>`
  drag-rectangle field. `selected_index` is still `None` (not yet wired
  into any tool — reserved for Phase 2's select-and-move feature).
- `Color` (de)serialization switched from struct-of-fields (the default
  `serde::Serialize` derive) to hex string (`#rrggbb` / `#rrggbbaa`).
  Affects every type that embeds `Color` (currently `Annotation` and
  `Config::UiConfig`); downstream consumers of `serde_json` /
  `postcard` encoded `Color` values need to update.
- `flintshot-core::render::render()` signature gained a third
  parameter: `bg: &mut Option<C::Image>` for the toolkit-specific
  background texture handle cache (Phase 1 Task 4). The shell owns
  the cache field; `render()` borrows it for the duration of the
  frame. Previous signature was `render(state, canvas)`; existing
  code must thread the cache through. No external callers existed
  at the time of the change.
- `flintshot-editor-egui` now uses a 3-module layout
  (`canvas.rs` / `app.rs` / `shell.rs`) rather than a single
  `lib.rs` (Phase 1 Task 5). Public API unchanged — `pub use`
  re-exports in `lib.rs` preserve the import paths. `canvas.rs`
  is the Task 4 content (unchanged); `app.rs` is the new private
  `EditorApp` `eframe::App` impl; `shell.rs` is the `EguiShell`
  `EditorShell` impl + `run_editor_blocking` helper.
- `justfile` gained `bootstrap`, `nextest`, and `watch` recipes, and
  `test` now prefers `cargo-nextest` (with fallback to `cargo test`)
  for unit + integration runs. Doc tests still use `cargo test`
  since nextest doesn't collect them. `just ci` is unchanged — it
  still mirrors the GitHub Actions `test` job byte-for-byte
  (`cargo test`, no nextest) so local pre-push results match CI.
  `just bootstrap` installs the optional `cargo-watch` and
  `cargo-nextest` subcommands idempotently. AGENTS.md and
  CONTRIBUTING.md document the new recipes.

### Removed

- Throwaway `flintshot --dump-config` debug flag (Phase 1 Task 3).
  Replaced by the real `flintshot config` subcommand.
- Default-timestamped `--out` path for `flintshot region` /
  `flintshot fullscreen` (Phase 1 Task 6). When `--out` is omitted
  the CLI now opens the editor instead of writing
  `/tmp/flintshot-<ISO-8601>.png`. Explicit `--out PATH` still writes
  a PNG (Task 3's path preserved as the headless / scripting escape
  hatch).
- `crates/flintshot-app/src/time_util.rs` (Phase 1 Task 6). Its only
  caller was `OutputArgs::resolve_path`, removed alongside the
  default-timestamp path above. The algorithm (Howard Hinnant's
  `civil_from_days`) is recoverable from git history; per the Task 3
  close-out note, Phase 4's upload-cache timestamps will promote it
  to `flintshot-core` rather than re-derive it.
- `crates/flintshot-editor-egui/examples/open_editor.rs` (Phase 1
  Task 6). The example was the Task 5 smoke probe for
  `EguiShell::run_editor`; the `flintshot region` CLI path now covers
  that role end-to-end.

### Notes

- Phase 0 closed against the local standing gate (`fmt`, `clippy`,
  `test`, MSRV build, cosmic build) — no git remote is configured yet,
  so the GitHub Actions pipeline has not executed against a push.
  First push will run the real CI; any failure becomes a session note.
- Standing gate corrected during Task 1 close-out: the gate commands
  in `AGENTS.md`, `docs/TODO.md`, `docs/PLAN.md`, `CONTRIBUTING.md`,
  `justfile`, and `.github/workflows/ci.yml` previously used
  `cargo test --all-targets` and `cargo test --doc` without a package
  selector — which silently ran only `flintshot-app`'s zero tests and
  errored on `flintshot-app`'s missing library target. Corrected to
  `cargo test --workspace --all-targets` and
  `cargo test --doc -p flintshot-core -p flintshot-editor-egui`.
  Same fix applied to the clippy command (`--workspace`).
- CI's `lint`, `test`, `msrv`, and `cosmic-build` jobs now install
  `xcap`'s Linux build deps (`libclang-dev`, `libpipewire-0.3-dev`,
  etc.) — needed because `flintshot-core` pulls in `xcap` for capture.
- No released versions yet. This unreleased section tracks Phase 0
  scaffolding plus Phase 1 Tasks 1 (`config` module), 2 (`capture`
  module), 3 (CLI wiring: `region`/`fullscreen` write PNGs,
  `config` subcommand, `--out` flag), 4 (`render()` background
  blit + `EguiCanvas::upload_image` / `draw_image`), 5
  (`EguiShell::run_editor` opens an eframe window, displays the
  screenshot, returns `EditorOutcome::Cancelled` on `Esc` / close),
  and 6 (`region` / `fullscreen` routed through `EguiShell` when
  `--out` is omitted; `--out PATH` preserved as the headless path;
  `cfg(not(feature = "egui"))` fallback errors clearly), and 7
  (editor state machine: `pending` field + `begin/update/commit/
  cancel_annotation`, snapshot `undo`/`redo`, `hit_test`; public
  `MockCanvas` + `DrawCall`; `render()` dispatches all 9 variants +
  draws `pending` on top), and 8 (`Tool::Rect` interactive
  end-to-end: egui pointer events → state machine → `render()`;
  toolbar with Rect selector; `Ctrl+Z` undo; zero-area rect filter),
  and 9 (Save / Copy buttons in the editor toolbar; PNG write via
  `rfd`; clipboard via `arboard`; new `compose_final` in core for
  headless render), and 10 (keyboard shortcuts via a pure
  `EditorState::dispatch_input`; new `flintshot-core::input` module
  with `Modifiers` / `InputEvent` / `DispatchOutcome`; cross-
  platform modifier mapping; pointer flow also routes through the
  dispatcher; `EditorApp::logic` reduced to a thin translator), and
  14 (`Tool::Select` drag-select + crop: new `EditorState::crop` /
  `confirm_selection` / `selection` field; `Annotation::
  crop_and_translate` per-variant; `Snapshot` type for image-aware
  undo/redo; `Rect::intersect` / `Rect::contains`; shell toolbar
  gains Select + Crop buttons; bg cache invalidated on image-dims
  change).

## Release cheat sheet (for maintainers)

When cutting the first release:

1. Confirm Phase exit criteria in `docs/ROADMAP.md` are met.
2. Move the `## [Unreleased]` contents to a new `## [0.1.0] - YYYY-MM-DD`
   section.
3. Bump `version = "..."` in every `Cargo.toml` (workspace +
   per-crate).
4. Tag with `git tag -s v0.1.0`; tags are signed.
5. Publish crates in dependency order: `flintshot-core` first, then
   dependents.
6. Add a fresh `## [Unreleased]` heading above the new release.
