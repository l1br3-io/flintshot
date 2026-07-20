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
  `docs/ARCHITECTURE.md`, `docs/ROADMAP.md`, `docs/DECISIONS.md`
  (ADRs 0001–0012), `CONTRIBUTING.md`, `AGENTS.md`, `docs/TEST_MATRIX.md`.
- Dual licensing: `MIT OR Apache-2.0`, with both license files in the
  repo root and the `license` field on every crate.

### Changed

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

### Removed

- Throwaway `flintshot --dump-config` debug flag (Phase 1 Task 3).
  Replaced by the real `flintshot config` subcommand.

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
  blit + `EguiCanvas::upload_image` / `draw_image`), and 5
  (`EguiShell::run_editor` opens an eframe window, displays the
  screenshot, returns `EditorOutcome::Cancelled` on `Esc` / close).

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
