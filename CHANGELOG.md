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

- Phase 5 floating overlay toolbar: once a selection exists, a
  compact two-row toolbar floats beside it — the ten tool chips and
  undo/redo (row 1, reusing the editor toolbar's chips and glyph
  language verbatim) and the accept row (✓ accept / ✕ cancel, the
  Save / Pin / Upload outputs, and the ember Copy primary; row 2).
  It anchors below the selection's bottom-left, flips above it when
  the space below runs out, and clamps into the viewport (pure,
  unit-tested placement math); it hides during fresh region drags
  and follows the region through resize/move gestures. Toolbar
  presses are swallowed before selection chrome or dispatcher
  dispatch (it floats inside the overlay panel), the hover cursor
  and magnifier stand down over it, and a tool-chip click mid-text
  draft commits the draft first. Copy is Enter-equivalent (Enter =
  Copy per the spec); the Save / Pin / Upload pipelines land later
  in Phase 5 — their buttons are present and their clicks log a
  notice until then.
- Phase 5 overlay selection chrome: the live overlay now layers
  Flameshot-style selection affordances over the dispatcher's
  selection — a dim mask over everything outside the active region
  (the whole screen until one is drawn), 8 grab handles (corners +
  edge midpoints) with per-direction resize cursors, move-by-drag
  inside the region (clamped to the screen), a live `W × H` size
  badge that flips below the region near the top screen edge, and
  the pixel-zoom magnifier shown on hover (no modifier hold in the
  overlay). Handle/move gestures run while the Select tool is active
  and mutate the selection through pure, unit-tested geometry
  helpers; fresh region drags keep flowing through the unchanged
  dispatcher. Annotation tools are untouched — presses with a
  drawing tool active draw, they never grab the region.
- Phase 5 overlay shell MVP: `flintshot region --overlay` /
  `fullscreen --overlay` (and `interaction-mode = "overlay"`) now open
  the real thing — a fullscreen, undecorated eframe viewport showing
  the frozen capture, rendered through the shared `render()` path.
  The overlay opens with the Select tool (first drag draws a region
  selection), drives the unchanged `EditorState` dispatcher for all
  input (single-key tool switching and Ctrl+Z/Y work; the
  egui→dispatcher translation helpers are factored out of the editor
  window for reuse — no second dispatcher), and closes on `Esc`
  (cancel, clean exit) or `Enter` (accept; the Enter→Copy output
  pipeline lands later in Phase 5 — until then an accepted close
  publishes `Cancelled` with a notice). New
  `EguiShell::run_overlay(state)` concrete entry (not a trait
  addition), plus shared `state_from_config` / outcome reporting in
  the CLI so both shells start from the same persisted `[ui]`
  defaults.
- `[ui] interaction-mode` config key (`"editor"` default | `"overlay"`,
  Phase 5 groundwork): selects how a fresh capture is presented. Stored
  as a free-form string like `magnifier-modifier`, so unknown values
  warn and fall back to `"editor"` without breaking config loading.
- `--overlay` / `--editor` flags on `flintshot region` and `flintshot
  fullscreen` (mutually exclusive): per-invocation override of
  `[ui] interaction-mode`. Mode resolution is CLI flag > config value,
  resolved once in `capture_and_dispatch` and logged with its source.
  An explicit flag runs the capture in this process (the daemon IPC
  request cannot carry it). Daemon-spawned workers keep picking the
  mode from their own config.

- Editor toolbar redesign: two-row "rail + bay" layout in
  `flintshot-editor-egui`. The rail holds the ten tool chips,
  undo/redo, and the output actions (Save, Pin, Upload, and the
  ember-filled Copy primary with sheet→check "Copied" feedback); the
  bay shows only the controls the active tool uses (swatches +
  custom color popup, stroke presets + width slider, text typography
  popover, blur-kind segmented control, select dims + crop) plus a
  per-tool hint. All icons are painter-drawn vector glyphs (see
  ADR-0027) — no emoji, no icon-font dependency. New private modules
  `theme` (graphite + ember token set and `Visuals`) and `toolbar`.
- Single-key tool shortcuts in the editor: unmodified
  `S R E A L M P T B N` select Select/Rectangle/Ellipse/Arrow/Line/
  Marker/Pencil/Text/Blur/Number. Modifier chords (Ctrl+S etc.) keep
  their dispatcher meanings; text-editing mode is exempt.
- Undo/Redo toolbar buttons (previously keyboard-only) — they replay
  the same Ctrl+Z / Ctrl+Y dispatch the keypresses produce.
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
- Color / stroke-width pickers + config persistence (Phase 1 Task 15).
  Toolbar gains a color swatch (`egui::color_edit_button_srgb`) and a
  stroke-width slider (1–20 px); a font-size slider (8–72) appears
  while the Text tool is active. Changes update `state.color` /
  `state.stroke_width` / `state.font_size` live. On editor close,
  `persist_ui_config` writes the current values back to
  `~/.config/flintshot/config.toml` via `Config::load` + `Config::save`
  (preserves unrelated sections). Config write failures surface as
  `tracing::warn!` — non-blocking, annotation continues. CLI now loads
  `Config::load()` before opening the editor and seeds `EditorState`
  with the persisted defaults.
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

#### Phase 2 — Daemon, tray, hotkeys, IPC

- `flintshot-core::ipc`: transport-agnostic IPC schema
  (`Request`/`Event` enums, `#[non_exhaustive]`, postcard-serde) and the
  `socket_path()` resolver. `Event` uses postcard-native external tagging
  (`#[serde(tag = "kind")]` is `WontImplement` under postcard); see
  ADR-0011.
- `flintshot-core::error::FlintshotError::Ipc { detail }` variant.
- `flintshot-core::config::HotkeyConfig` (`[hotkeys]` section:
  `region`/`fullscreen` chord strings, serde-defaulted) with
  comment-preserving merge, and a public `config_path()`.
- `flintshot-core::capture::monitors()` + `MonitorInfo` (xcap monitor
  enumeration for the tray's per-monitor submenu).
- Workspace dependencies for the resident daemon: `interprocess`
  (local-socket transport, `tokio` feature), `postcard` (binary
  encoding), `global-hotkey` (X11/macOS/Windows hotkeys), `notify`
  (config hot-reload), `single-instance` (daemon-role lock). Resolves
  the `single` vs `single-instance` open question: `single` is an
  unrelated Itertools helper; `single-instance` is the Tauri-affiliated
  single-instance lock.
- `flintshot daemon`: long-running process — tokio side thread + main-
  thread platform loop bridged by mpsc (ADR-0018), single-instance lock
  (daemon role only), `interprocess` IPC server with length-prefixed
  postcard framing, `Request::Quit` graceful shutdown, and `Ctrl-C`
  exit. Proven at runtime.
- `flintshot daemon --replace`: sends `Request::Quit` to the running
  daemon, waits for it to exit, then takes over (ADR-0020). Runtime-
  confirmed it swaps daemons cleanly.
- Worker-spawn model (ADR-0019): the daemon never captures in-process —
  tray/hotkey/IPC capture actions spawn a `flintshot` child (resolved
  via `current_exe`, reusing the Phase 1 CLI) and broadcast
  `CaptureStarted`/`EditorOpened` to IPC subscribers.
- System tray (`tray` feature): *Capture region*, *Capture fullscreen*,
  *Fullscreen on monitor →* (per-monitor submenu), *Open config*,
  *Quit*. Tray failure is non-fatal (compositor without an SNI host).
- Global hotkeys from `[hotkeys]` (X11/macOS/Windows); on Wayland the
  daemon logs the compositor-keybind workaround and continues. Hot-
  reload: editing `[hotkeys]` re-registers without restarting the
  daemon.
- CLI detect-and-delegate: `flintshot region`/`fullscreen` hand the
  capture to a running daemon when one is present (centralising the
  event stream), and fall back to the Phase 1 in-process path
  otherwise. `--standalone` forces the in-process path.
- `flintshot-app` `tray` feature (default OFF): gates the `tray-icon`
  crate, whose Linux backend (`libappindicator`) pulls the full GTK3
  stack that the default CI build doesn't install. The daemon runs
  without it (IPC + hotkeys + single-instance); build with
  `--features tray` to enable the system tray.
- Boundary test extended: `flintshot-core` also rejects `use
  interprocess` / `use global_hotkey` / `use tray_icon` / `use notify`.

##### Notes (Phase 2)

- `Event` uses postcard-native external tagging; the spec/ARCHITECTURE
  `#[serde(tag = "kind")]` internally-tagged form is `WontImplement`
  under postcard (Task 1 found this; kept the schema, dropped the tag).
- Default hotkey chords are `Ctrl+Shift+PrintScreen` /
  `Ctrl+Alt+PrintScreen` — `global-hotkey` names the key `PrintScreen`,
  not `Print` (ARCHITECTURE.md's example updated).
- Wayland global hotkeys are deferred to Phase 6 (`ashpd`
  GlobalShortcuts portal); bind a compositor keybinding to `flintshot
  region` / `flintshot fullscreen` in the meantime.
- The tray's per-monitor submenu is built at tray creation; muda/tray-
  icon don't portably expose a submenu-open event, so hot-plugging a
  monitor mid-session needs a daemon restart (documented limitation).
- Text tool overhaul (see ADR-0021):
  - **Box indicator**: a solid block caret at the insertion point plus
    a hollow bounding box around the in-flight text (replaces the thin
    caret bar). Live-view-only; never reaches Save/Copy output.
  - **Multi-line text** (`Shift+Enter` inserts a newline; plain `Enter`
    still commits). Pulled forward from Phase 2 — both the egui view
    and the `ImageCanvas` save path now lay out multiple baselines.
  - **Decoupled text colour**: the Text tool gets its own colour
    picker distinct from the drawing-tools colour (persisted as
    `[ui] default-text-color`).
  - **Text customisation**: font family (Sans / Mono), weight
    (Regular / Bold), style (Upright / Italic), alignment
    (Left / Center / Right), optional background highlight, and
    optional outline (colour + width). New core types `FontFamily`,
    `FontWeight`, `FontStyle`, `TextAlign`, `TextOutline`,
    `TextStyle`, `TextMetrics`; `Annotation::Text` gained matching
    fields with `#[serde(default)]`. `Canvas::draw_text` /
    `Canvas::measure_text` now take a `&TextStyle` bundle and return
    `TextMetrics` (width / height / last-line width). Bold is real on
    the save path (fontdb weight query) and faux on the egui view
    (egui 0.35 has no weight API); serif family falls back to sans
    until a serif face is bundled.

#### Phase 2b — annotation tools + magnifier

- Ellipse tool end-to-end (Phase 2b Task 001): `EguiCanvas::draw_ellipse`
  renders an `EllipseShape` (outline + optional fill via the egui
  `Painter`); `ImageCanvas::draw_ellipse` rasterizes fill + outline into
  the `RgbaImage` so committed ellipses appear in saved/copied PNGs
  (fixes a latent Phase 1 bug where the no-op silently dropped them).
  Toolbar exposes "◯ Ellipse" alongside "▭ Rectangle".
- Line tool end-to-end (Phase 2b Task 002): toolbar exposes "╱ Line"
  alongside the other shape tools. Drag from one point to another
  commits an `Annotation::Line` with a live rubber-band preview;
  click-without-drag (sub-1px length) is now treated as degenerate and
  cancels rather than committing an invisible zero-length stroke — the
  `finish_pointer_up` guard mirrors the existing Arrow check. Hit-test
  reuses the point-to-segment distance tolerance so thin strokes stay
  clickable.
- Marker + Pencil freehand tools (Phase 2b Task 003). New pure
  `flintshot-core::freehand` module: `smooth_polyline(points, tension)`
  applies a Catmull-Rom spline to raw pointer samples inside
  `render_ann` (pending + committed, so WYSIWYG holds), and
  `MARKER_ALPHA` (`0x66`) is applied to `Annotation::Marker.color` at
  construction so every backend renders a translucent highlight with no
  render-path special-casing. New `Canvas::draw_polyline` method (with
  a `draw_line`-looping default) lets `ImageCanvas` composite the whole
  stroke once per pixel (avoids stamp-compounding a translucent marker
  toward opaque in saved PNGs) and `EguiCanvas` issue one
  `PathShape::line`; `MockCanvas` records a `DrawPolyline`. Toolbar
  exposes "🖍 Marker" and "✏ Pencil". See ADR-0022.
- `Color::with_alpha(a)` helper on `flintshot-core::Color`.
- `Stroke::ZERO` constant on `flintshot-core::Stroke` — a zero-width
  stroke that renders nothing, used by callers that need a `&Stroke`
  for a fill-only shape (e.g. `Canvas::draw_ellipse` with `Some(fill)`
  and no outline, as the Number tool's bubble does).
- Number tool end-to-end (Phase 2b Task 004). Click-to-place dispatch
  path: a `PointerDown` with `Tool::Number` commits an
  `Annotation::Number` immediately (no drag, no `pending`) via the new
  `EditorState::place_number(pos)` entry point, increments
  `next_number`, and pushes a `Snapshot`. The bubble renders as a true
  filled circle via `Canvas::draw_ellipse(bbox, &Stroke::ZERO,
  Some(color))` (equal axes — a circle is an inscribed ellipse; reuses
  Task 001's impl), replacing the square `draw_rect_filled`
  placeholder. Digit text colour is picked by Rec. 601 luminance
  (white on dark fills, black on light) so the number stays legible
  regardless of bubble colour. Toolbar exposes "# Number" alongside
  the other tools.
- Magnifier widget (Phase 2b Task 005). New toolkit-owned
  `flintshot-editor-egui::magnifier` module: a pixel-zoom inset
  (`INSET_PX = 96`, `ZOOM = 4.0`) that samples `EditorState::image`
  directly and paints a cached one-frame texture. Triggered by holding
  a modifier (default `Space`, configurable via `[ui] magnifier-modifier`)
  or always-on-while-drawing (`[ui] magnifier-always-on = true`, default
  `false`). The pure `sample_zoomed_region(image, cursor, zoom, inset)`
  helper is unit-tested headless (center-sample, edge-clamp,
  smaller-than-inset fallback, `zoom < 1.0` clamp). No core abstraction —
  an iced/slint shell implements its own per ARCHITECTURE.md §What stays
  toolkit-specific. Two new `[ui]` config fields (`#[serde(default)]`,
  kebab-case per convention).

#### Phase 3 — Pin-to-desktop + COSMIC applet

- `flintshot pin` scaffolding (Phase 3 Task 001). New `[pin]` config
  section (`flintshot-core::config::PinConfig`: `default_opacity = 0.5`,
  `copy_on_close = false`, `default_position = "center"`; re-exported
  from `flintshot-core`). `flintshot pin <path>` now loads the image,
  resolves window geometry (natural size by default; `--size WxH`
  overrides; active-monitor centre by default, `--at X,Y` overrides),
  merges CLI overrides (`--opacity`, `--no-clipboard`) over `[pin]`
  config, and reaches `EguiShell::run_pin` — still a `NotImplemented`
  stub until Task 2 (logged gracefully, not panicked).
- `flintshot-core::capture::MonitorInfo` gained `x`, `y`, `width`,
  `height` (the monitor's virtual-desktop geometry in physical px) so
  the pin window can centre itself on the active monitor via
  `flintshot_core::monitors()`. Existing readers (tray submenu) are
  unaffected.
- `flintshot pin <path>` now floats a real borderless, always-on-top,
  transparent window (Phase 3 Task 002). `EguiShell::run_pin` opens an
  eframe viewport (`.with_decorations(false)`, `.with_always_on_top()`,
  `.with_transparent(true)`, `.with_resizable(true)`, positioned/sized
  from the `Rect` arg) driven by the new
  `flintshot-editor-egui` `PinApp` (`eframe::App`). The window blits the
  image once (cached texture) and implements all five UX features:
  drag-to-move anywhere (via `ViewportCommand::StartDrag`), close on
  `Esc` or double-click, instant hover-opacity (opaque while hovered,
  `[pin] default-opacity` otherwise), aspect-locked resize via a
  bottom-right handle (`ViewportCommand::InnerSize`), and optional
  copy-on-close to the clipboard via `arboard`. Resolved
  `--opacity` / `--no-clipboard` overrides reach the window through a
  new `EguiShell::with_pin_config` builder (the `EditorShell::run_pin`
  trait signature stays `(image, rect)`).
- Worker save reporting + daemon last-saved state (Phase 3 Task 003).
  New `flintshot_core::ipc::Request::ReportSaved { path }` variant: a
  worker that closes its editor with `EditorOutcome::Saved(path)`
  reports the path back to the daemon (best-effort — silently skipped
  when no daemon answers, so a standalone worker is unaffected). The
  daemon tracks the most recent save in a new
  `flintshot_app::ipc_server::LastSaved`
  (`Arc<std::sync::Mutex<Option<PathBuf>>>`), created in `daemon::run`
  and shared with the IPC server; `PinLast`/`OpenLast` (Task 4) and
  Phase 4 upload-from-last read it back. On receipt the daemon also
  broadcasts the previously-unused `Event::Saved { path }` to
  subscribers. See ADR-0023.
- Daemon `PinLast` / `OpenLast` dispatch + monotonic pin ids (Phase 3
  Task 004). `Request::PinLast` reads the last-saved path from the
  daemon's state, spawns `flintshot pin <path>`, and emits
  `Event::Pinned { id }` with a daemon-assigned monotonic id (via a new
  `flintshot_app::worker::PinIdSource` counter). `Request::OpenLast`
  spawns `flintshot open <path>` to re-edit the last-saved capture. With
  no prior save, both emit `Event::Error { code: "no-last-capture" }`.
  New `flintshot_app::worker::DispatchState` bundles the last-saved
  handle + pin-id counter and is threaded through every dispatch entry
  point (IPC `handle_connection`, tray `drain_tray_menu`, hotkey
  `drain_hotkeys`). New `flintshot open <path>` CLI subcommand loads an
  existing image into the editor (no capture).
- Editor toolbar Pin button (Phase 3 Task 005). A "📌 Pin" button in the
  egui editor toolbar composes the final image (selection-aware, like
  Copy),   writes it to a temp PNG (`flintshot-pin-<pid>-<n>.png` under
  the OS temp dir, persisted for the pin worker to read), and spawns
  `flintshot pin <tmpfile>` as a detached child resolved via
  `current_exe` (mirroring `worker::CommandSpawner`). The editor closes
  on success. New `EditorOutcome::Pinned` variant names the outcome so
  the daemon / future callers can distinguish it. Compose / temp-write
  failures close the editor with an error; spawn failures keep the
  editor usable (Save / Copy still work) so the user can fall back.
- COSMIC applet IPC client (Phase 3 Task 006). New
  `flintshot-cosmic::ipc` module: length-prefixed postcard framing
  (duplicated ~30 LOC from `flintshot-app::ipc_server` per ADR-0010),
  `send_request` (one-shot connect-send-disconnect), `subscribe`
  (long-lived `EventStream`), and `probe_daemon` (`DaemonState` enum
  for running/not-running). Headless-testable against a hand-rolled
  in-process listener (no `flintshot-app` dep). The applet is a pure
  IPC client: depends on `flintshot-core` + `libcosmic` +
  `interprocess`/`postcard`/`tokio` only — no `egui`, no `flintshot-app`.
- Real COSMIC panel applet replacing the stub (Phase 3 Task 007).
  `flintshot-cosmic` is now a `cosmic::applet::run` entry driven by a
  `cosmic::Application` impl (`flintshot_cosmic::app::App`). The panel
  button opens a popup mirroring the tray menu item-for-item: *Capture
  region*, *Capture fullscreen*, per-monitor entries (from
  `flintshot_core::monitors()`), *Pin last*, *Open last*, *Open config*
  (shells out to `$EDITOR` via `flintshot_core::config_path()`, ~10 LOC
  mirror of the daemon's `open_config_file`), and *Quit*. Each action
  resolves through a pure `flintshot_cosmic::menu` dispatch
  (`action_for(&MenuMessage) -> Action`, unit-tested headless) and sends
  the matching `Request` via the Task 6 IPC client. The live event
  stream + daemon-absent status line is Task 8.
- COSMIC applet live event stream + daemon-absent state (Phase 3 Task
  008). The popup now shows a live status line (*Idle* / *Capturing…* /
  *Editor opened* / *Saved \<basename\>* / *Pinned #N* /
  *Uploaded \<url\>* / *Error: \<message\>*, tinted red) driven by a
  persistent `Subscribe` connection. A background `Subscription`
  runs the reconnect loop: probe → subscribe → read events, with a 2 s
  re-probe on stream loss or daemon absence. When the daemon is down,
  the popup shows *"daemon not running — start with `flintshot
  daemon`"* and disables daemon-dependent buttons (*Open config* stays
  enabled). New pure `ipc::status_for_event(&Event) -> String` maps
  each event variant to its status text (unit-tested headless).
#### Phase 4 — upload

- `[upload]` config section (Phase 4 Task 002). New
  `flintshot-core::config::UploadConfig` (`default-backend`, built-in
  default `"imgur"`), re-exported from `flintshot-core` and wired into
  `Config::load` / `Config::save` with the same comment-preserving
  merge as `[pin]`; a missing section falls back to defaults. No
  per-backend sub-tables — credentials live in the OS keychain, never
  in config.
- IPC schema upload variants (Phase 4 Task 003):
  `Event::UploadStarted { backend }` (optimistic dispatch signal,
  mirrors `CaptureStarted`) and `Request::ReportUploaded { url,
  delete_handle }` (worker → daemon result report, parallels
  `ReportSaved`). Postcard round-trip tests extended for both.
- `flintshot-app::upload` module (Phase 4 Task 004) with the first
  concrete `UploadBackend` impl: `ImgurBackend`. POSTs multipart form
  data to `https://api.imgur.com/3/image`; supports anonymous (zero-
  config) and Client-ID authenticated (`Authorization: Client-ID <id>`
  from `keyring`) modes. HTTP-status mapping: 2xx → `UploadOutcome`,
  401/403 → `UploadError::Auth`, 429 + other non-2xx →
  `UploadError::Backend`, transport failure → `UploadError::Transport`.
  `resolve_backend("imgur")` returns an `Arc<dyn UploadBackend>` from
  the registry; unknown ids → `UploadError::UnknownBackend`. Response
  mapping tested via `mockito` (no real network). New workspace deps:
  `reqwest` (rustls-tls, multipart, json), `mockito` (dev-dep);
  `keyring` added to `flintshot-app`.
- `UploadOutcome::new(url, delete_handle)` constructor (Phase 4 Task
  004). Required because `UploadOutcome` is `#[non_exhaustive]`, so
  external crates cannot use struct-literal construction.
- `flintshot upload <path>` CLI body (Phase 4 Task 005) — replaces the
  Phase 0 stub. Loads the file (clear `no such file` error), sniffs
  the format via `image::guess_format` (PNG / JPEG / WebP sent as-is;
  no re-encode, no `--mime` flag), resolves the backend from
  `--backend` over `[upload] default-backend` over the built-in
  `"imgur"`, reads the Client-ID from the keyring entry `flintshot` /
  `imgur:client-id` when present (a failed keychain read falls back to
  anonymous with a log line), runs the backend on a fresh tokio
  runtime, prints the URL to stdout as the last line
  (machine-parseable; diagnostics to stderr), copies the URL to the
  clipboard (`arboard`), fires a desktop notification (`notify-rust`)
  on success and on failure, reports the result to the daemon
  best-effort, and exits non-zero on any failure.
- `flintshot auth imgur [--clear]` subcommand (Phase 4 Task 006) —
  the only credential-entry path. Prompts for the Imgur Client-ID with
  hidden input (`rpassword`) when stdin is a TTY, or reads
  `IMGUR_CLIENT_ID` otherwise; stores it in the OS keychain
  (`flintshot` service, `imgur:client-id` entry) — never in
  `config.toml`, logs, or stdout. `--clear` removes the entry;
  clearing an absent entry is a no-op with exit 0.
- Daemon `Request::UploadLast` dispatch (Phase 4 Task 007) — replaces
  the `unsupported` stub. Resolves the `ReportSaved`-fed last-saved
  path, spawns `flintshot upload <path> [--backend <id>]` as a worker
  process (`--backend` omitted when it equals the built-in default),
  and emits `Event::UploadStarted { backend }` optimistically before
  the spawn (mirrors `CaptureStarted`). With no prior save it emits
  `Event::Error { code: "no-last-capture" }` and does not spawn
  (mirrors `PinLast` / `OpenLast`).
- Worker → daemon upload-result reporting (Phase 4 Task 008). On
  success `run_upload` sends `Request::ReportUploaded { url,
  delete_handle }` through the same `delegate_to` path as
  `ReportSaved` — best-effort and silent when no daemon answers. The
  IPC server intercepts it in `handle_connection` and broadcasts
  `Event::Uploaded` to every subscriber. See ADR-0025.
- Editor *Upload* button (Phase 4 Task 009). A "☁ Upload" button in
  the egui toolbar composes the final image (selection-aware, like
  Pin), writes it to a temp PNG (`flintshot-upload-<pid>-<n>.png`,
  left on disk for the OS temp reaper; a reaper thread waits on the
  child so it doesn't zombie), spawns `flintshot upload <tmpfile>` as
  a detached worker, and closes the editor with
  `EditorOutcome::Uploaded(String::new())` — the outcome signals
  initiation; the real URL reaches subscribers via the worker's
  `ReportUploaded`, not the editor. Compose / write failures close the
  editor with the error; a spawn failure keeps the editor open so
  Save / Copy still work.

### Changed

- Editor toolbar reorganized from a single overflowing row into the
  two-row rail + bay layout (see Added above). The always-visible
  color/stroke pickers now appear only for tools that use them; the
  annotation counter and the static "Drag to draw · Ctrl+Z undo ·
  Esc cancel" hint row are removed (replaced by per-tool hints); the
  emoji-labeled buttons are replaced by painter-drawn glyph chips.
- `UploadBackend` trait methods now return `Pin<Box<dyn Future + Send>>`
  instead of `impl Future` (Phase 4 Task 004). The `-> impl Future`
  return type made the trait not dyn-compatible, but the registry and
  daemon need `Arc<dyn UploadBackend>`. This is the standard boxed-
  async pattern; `Send + Sync + 'static` shape preserved.

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
- `Snapshot` extended with `next_number: u32` (Phase 2b Task 004) so
  undoing a Number-tool placement decrements the auto-incrementing
  counter (otherwise it would ratchet upward across undos and the
  next placement would skip a number). Constructor signatures widen:
  `Snapshot::anns_only(anns)` → `Snapshot::anns_only(anns,
  next_number)`, and `Snapshot::full(anns, image)` →
  `Snapshot::full(anns, image, next_number)`. The undo / redo paths
  always restore `next_number` from the snapshot.
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

### Fixed

- `ImageCanvas::draw_ellipse` was a no-op throughout Phase 1 — committed
  ellipses silently dropped from saved/copied PNGs even though they
  rendered correctly in the live editor (a WYSIWYG hole masked because
  no Phase 1 tool ever committed an `Annotation::Ellipse`). Phase 2b
  Task 001 filled in the real rasterizer (fill + outline into the
  `RgbaImage`), so ellipses now round-trip through Save / Copy
  identically to the live view.
- Crop no longer leaves the cropped image partly hidden behind the
  editor toolbar. The egui canvas now offsets every draw call by the
  canvas panel's top-left, so the image's (0, 0) lands at the panel
  origin instead of the window's — the toolbar used to cover the
  topmost rows of every screenshot, which became visible as soon as
  a small crop made the hidden strip a large fraction of the image.
  The shell also keeps requesting `ViewportCommand::InnerSize` until
  the panel reports the cropped dimensions, so the window resizes to
  fit on toolkits / platforms that honour the command.
- Text tool no longer traps the editor in modal mode. Clicking elsewhere
  while editing commits the current draft and places a new caret (or
  starts the newly-selected tool) — Flameshot-like behaviour. Switching
  tools via the toolbar while a draft is active now flushes the draft
  instead of blocking all subsequent input.
- Copy-to-clipboard no longer loses the image on X11. The
  `arboard::Clipboard` handle is kept alive in the `EditorApp` for the
  window's lifetime (X11's clipboard is owner-based: dropping the handle
  or exiting the process clears the selection). The editor window stays
  open after copy with "Copied!" feedback; the user pastes and then
  closes manually.
- Copy now respects the active `Tool::Select` selection. When a
  non-degenerate selection rect is present, Copy emits just the
  selected region (annotations baked in, clipped at the region
  boundary); without a selection it falls back to the full image.
  Editor state is left untouched — the displayed image and selection
  rect remain as-is, so the user can adjust and copy again. This
  decouples "copy the selection" (read-only output) from the Crop
  tool (which permanently narrows the editor). New
  `flintshot_core::compose_final_region(state, rect)` is the
  selection-aware source of truth alongside `compose_final`.
- Clicking any toolbar button (Copy / Crop / Save / tool selectors /
  pickers) with an active `Tool::Select` selection no longer corrupts
  the selection rect. The shell's pointer dispatch previously emitted
  `PointerDrag` for every frame the primary button was held,
  regardless of where the press began — so a press that began on the
  toolbar fed `update_selection` with the toolbar's (negative
  canvas-y) coordinates, extending the rect up past the image origin
  before Copy / Crop read it. The shell now tracks whether the
  current press began inside the canvas, and only emits drag/up
  events for canvas-originated presses.
- Text caret now follows the insertion point. Previously the block
  caret was pinned to the first line's end — after `Shift+Enter` it
  stayed on line 0 while typing continued on the new line below. A
  `text_cursor` (char index into the draft) now tracks the real
  insertion point; `←`/`→`/`↑`/`↓` move it (wrapping across line
  breaks, preserving the column), and `Home`/`End` jump to the line
  edges. Typing, backspace, and newline all operate at the cursor, so
  text can now be edited mid-string rather than only appended.

### Notes

- **Phase 4 deviations from the spec draft** (documented in
  ADR-0025 and `docs/TEST_MATRIX.md` Phase 4 caveats): upload
  *failures* do not reach daemon subscribers (the sketched
  `Event::Error { code: "upload-failed" }` broadcast was dropped; the
  worker owns failure UX — notification, stderr, non-zero exit), and
  no tray / COSMIC-applet *Upload last* menu item shipped —
  `Request::UploadLast` is reachable over IPC from scripts and custom
  clients.
- **Phase 1 (Capture + Editor Window) shipped 2026-07-28.** All
  checkpoints A–F complete; exit criteria in
  `docs/specs/phase-1-capture-editor/spec.md` met. Spec status flipped
  to `shipped`; `docs/ROADMAP.md` moves Phase 2 to **In flight**. No
  version tag cut — `0.x.y` remains in effect until Phase 4 (upload)
  per the versioning policy.

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
  change), and 15 (color / stroke-width / font-size pickers in the
  toolbar; config persistence on editor close via `Config::load` +
  `Config::save`; CLI seeds `EditorState` from `Config::load()`).

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
