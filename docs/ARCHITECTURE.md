# Architecture

This document captures the technical architecture of `flintshot`. For
product vision, see [VISION.md](VISION.md). For delivery sequencing, see
[ROADMAP.md](ROADMAP.md). For decision rationale, see
[decision records](adr/).

## Workspace layout

```
flintshot/
├── Cargo.toml                  # [workspace], default-members = ["crates/flintshot-app"]
├── crates/
│   ├── flintshot-app/          # bin: CLI + daemon + tray; features = {default=["egui"]}
│   ├── flintshot-core/         # lib: geom, editor, render (Canvas trait), shell trait,
│   │                           #      capture, config, IPC, Backend trait
│   ├── flintshot-editor-egui/  # lib: egui impl of Canvas + EditorShell
│   └── flintshot-cosmic/       # bin: COSMIC panel applet (libcosmic/iced)
└── docs/
```

## Crate responsibilities & boundaries

### `flintshot-core` (lib)
**Owns:** platform-agnostic primitives — geometry types (`Point`,
`Rect`, `Color`, `Stroke`, `Key`), the `Annotation` enum, `EditorState`
and all editor logic (hit-testing, undo/redo, tool state machines,
blur/pixelate computation), the `Canvas` and `EditorShell` traits, the
capture API, image operations, config schema, IPC protocol types, the
`Backend` upload trait, and any shared error types.

**Does not own:** anything GUI-toolkit-specific. No `egui`, no
`libcosmic`, no `iced`, no `slint` types appear in its public API.
`image::RgbaImage` from the `image` crate is the only image type that
crosses the boundary.

**Consumers:** every other crate.

### `flintshot-editor-egui` (lib)
**Owns:** the egui implementation of `Canvas` (mapping primitives to
`egui::Painter` calls) and `EditorShell` (driving eframe windows for
the editor and pin-to-desktop). Translates egui input events into
`flintshot-core::InputEvent`. Owns the toolbar/palette widgets,
magnifier, color picker — all the toolkit-specific UI.

**Does not own:** editor state, annotation logic, capture, IPC server,
system tray.

**Consumers:** `flintshot-app` (via the `egui` feature flag).

### `flintshot-app` (bin)
**Owns:** the entry point — CLI dispatch (`clap`), daemon lifecycle,
tray icon, global hotkeys, single-instance lock, IPC server, and
spawning editor windows via whichever `EditorShell` impl the active
feature flag selects.

**Does not own:** capture primitives, the `Backend` trait, the
`Canvas`/`EditorShell` traits themselves (these come from
`flintshot-core`).

**Feature flags:**
```toml
[features]
default = ["egui"]
egui   = ["dep:flintshot-editor-egui"]
# iced  = ["dep:flintshot-editor-iced"]   # future
# slint = ["dep:flintshot-editor-slint"]  # future
```

**Consumers:** end users via the installed binary.

### `flintshot-cosmic` (bin)
**Owns:** a separate binary that runs as a COSMIC panel applet. It is a
pure IPC client — it connects to the daemon socket, sends `Request`s,
and reads the live `Event` stream over a persistent `Subscribe`
connection. The popup mirrors the tray's action set (capture, per-
monitor capture, pin/open last, config, quit) plus a status line fed by
the event stream, and degrades to a clear *"daemon not running"* state
when the socket is absent.

**Does not own:** capture, editor, anything beyond the popup UI. It
depends on `flintshot-core` (types + `monitors()`) + `libcosmic` only —
no `egui`, no `flintshot-app`, no `flintshot-editor-egui`.

**Consumers:** COSMIC desktop users via the panel.

### Dependency direction
```
flintshot-cosmic ─────────┐
                          ├──► flintshot-core
flintshot-app ────────────┤
                          └──► flintshot-editor-egui ──► flintshot-core
```
No crate depends on `flintshot-app` or `flintshot-cosmic`. They are
leaf binaries. A future `flintshot-editor-iced` would slot in beside
`flintshot-editor-egui` as a sibling.

## Process model

```
┌────────────────────────┐
│  flintshot daemon      │ (long-running)
│  - tray-icon           │
│  - global-hotkey       │
│  - single-instance     │
│  - IPC server (socket) │
│  - config watcher      │
└─────────┬──────────────┘
          │ spawn / signal
          ▼
┌────────────────────────┐
│  flintshot (worker)    │ (short-lived per action)
│  - capture             │
│  - editor window       │
│  - upload              │
│  - pin window          │
└────────────────────────┘
```

A single binary handles both roles; behavior is selected by subcommand:

- `flintshot daemon` — long-running, owns tray + hotkeys + IPC server
- `flintshot region [--monitor N] [--delay SECS]` — one-shot: capture
  region, open editor
- `flintshot fullscreen [--monitor N]` — one-shot: capture full screen,
  open editor
- `flintshot pin <path>` — open a pin window for an existing image
- `flintshot open <path>` — open an existing image in the editor (no capture)
- `flintshot upload <path> --backend=imgur` — upload and exit
- `flintshot config` — print or edit config

### IPC protocol

Local socket via `interprocess`:
- Linux/macOS: Unix domain socket at `$XDG_RUNTIME_DIR/flintshot.sock`
- Windows: named pipe at `\\.\pipe\flintshot`

Messages are length-prefixed `postcard`-encoded (chosen over JSON for
size and zero-copy friendliness). Schema lives in
`flintshot-core::ipc`:

```rust
pub enum Request {
    CaptureRegion { monitor: Option<u32>, delay_ms: u32 },
    CaptureFullscreen { monitor: Option<u32> },
    OpenLast,
    PinLast,
    UploadLast { backend: String },
    ReportSaved { path: PathBuf },   // worker → daemon: best-effort save report
    Quit,
    Subscribe,  // stream of events back to the client (for applet/tray)
}

pub enum Event {
    CaptureStarted,
    EditorOpened,
    Saved { path: PathBuf },
    Uploaded { url: String, delete_handle: Option<String> },
    Pinned { id: u64 },
    Error { code: String, message: String },
}
```

## UI abstraction layer

`flintshot` is UI-toolkit-agnostic by design. `flintshot-core` owns all
shared types and pure logic; UI toolkits implement two traits. egui is
the default; swapping or adding a toolkit (iced, slint) is purely
additive. See [ADR-0012](adr/ADR-0012-canvas-editorshell-traits.md)
for rationale.

### Geometry types (`flintshot-core::geom`)

Minimal, framework-free. Grows only as features demand.

```rust
pub struct Point  { pub x: f32, pub y: f32 }
pub struct Rect   { pub min: Point, pub max: Point }
pub struct Color  { pub r: u8, pub g: u8, pub b: u8, pub a: u8 }
pub struct Stroke { pub color: Color, pub width: f32 }

pub enum Key {
    Escape, Enter, Backspace,
    Ctrl, Shift, Alt, Super,
    Char(char),
    // …grows as needed
}
```

### `Canvas` trait (`flintshot-core::render`)

Immediate-mode drawing primitives for annotations. The associated
`Image` type lets each toolkit use its native texture handle
(`egui::TextureHandle`, etc.). Blur/pixelate are computed in core and
emitted as plain image-blit instructions, so output is identical across
toolkits.

```rust
pub trait Canvas {
    type Image;

    fn upload_image(&mut self, img: &image::RgbaImage) -> Self::Image;
    fn draw_image(&mut self, img: &Self::Image, rect: Rect);

    fn draw_rect_outline(&mut self, rect: Rect, stroke: &Stroke);
    fn draw_rect_filled (&mut self, rect: Rect, color: Color);
    fn draw_ellipse     (&mut self, rect: Rect, stroke: &Stroke, fill: Option<Color>);
    fn draw_line        (&mut self, a: Point, b: Point, stroke: &Stroke);
    fn draw_arrow       (&mut self, a: Point, b: Point, stroke: &Stroke, head_size: f32);
    fn draw_text        (&mut self, pos: Point, text: &str, size: f32, color: Color);
}

/// Shared render logic — same code path for every UI toolkit.
pub fn render<C: Canvas>(state: &EditorState, canvas: &mut C) {
    // draw background image, iterate state.anns, dispatch per variant
}
```

### `EditorShell` trait (`flintshot-core::shell`)

Toolkit lifecycle: open the editor (or a pin window), block until the
user is done, return the outcome.

```rust
pub trait EditorShell {
    fn run_editor(&self, state: EditorState) -> Result<EditorOutcome>;
    fn run_pin   (&self, image: image::RgbaImage, rect: Rect) -> Result<()>;
}

pub enum EditorOutcome {
    Saved(PathBuf),
    CopiedToClipboard,
    Uploaded(String),
    Pinned,
    Cancelled,
}
```

### What stays toolkit-specific (no abstraction)
- Toolbar / palette widgets
- Color picker
- Magnifier (toolkit-owned, reads from `EditorState::image`)
- Text input for `Annotation::Text`
- File dialogs (already abstracted by `rfd`)
- Window decorations, theming

## Data model

All types use `flintshot-core` geometry, never egui's. The shell owns
toolkit-specific resources (texture handles) — they don't appear in
`EditorState`.

### Annotation
```rust
pub enum Annotation {
    Rect    { rect: Rect, stroke: Stroke, fill: Option<Color> },
    Ellipse { rect: Rect, stroke: Stroke, fill: Option<Color> },
    Line    { a: Point, b: Point, stroke: Stroke },
    Arrow   { a: Point, b: Point, stroke: Stroke, head_size: f32 },
    Marker  { points: Vec<Point>, color: Color, width: f32 }, // semi-transparent
    Pencil  { points: Vec<Point>, color: Color, width: f32 },
    Text    { pos: Point, text: String, font_size: f32, color: Color },
    Blur    { rect: Rect, kind: BlurKind, strength: u8 }, // Gaussian or pixelate
    Number  { pos: Point, n: u32, color: Color, diameter: f32 },
}

pub enum BlurKind { Gaussian, Pixelate }
```

### EditorState
```rust
pub struct EditorState {
    pub image: image::RgbaImage,           // source screenshot, never mutated
    pub anns: Vec<Annotation>,             // committed annotations
    pub undo: Vec<Vec<Annotation>>,        // history snapshots
    pub redo: Vec<Vec<Annotation>>,
    pub tool: Tool,
    pub color: Color,
    pub stroke_width: f32,
    pub font_size: f32,
    pub next_number: u32,
    pub selection: Option<usize>,          // index into anns
}
```

Note: no `TextureHandle` field — the shell uploads the image once and
caches the toolkit-specific handle on its side. Core never sees it.

### Config
```toml
# ~/.config/flintshot/config.toml

[ui]
default_color = "#ff0000"
default_stroke_width = 3.0
default_font_size = 16.0
interaction_mode = "editor"  # or "overlay" (Phase 5)

[capture]
include_cursor = true
default_delay_ms = 0

[hotkeys]
region = "Ctrl+Shift+PrintScreen"
fullscreen = "Ctrl+Alt+PrintScreen"

[pin]
default_opacity = 0.5          # resting opacity; snaps to 1.0 while hovered
copy_on_close = false          # copy image to clipboard when the pin closes
default_position = "center"    # "center" (active monitor) or "X,Y"

[upload]
default_backend = "imgur"

[ipc]
socket_path = ""  # empty = platform default
```

Loaded via `serde` + `toml_edit`. Hot-reloaded via `notify` file
watcher in the daemon.

### Upload `Backend` trait
```rust
#[async_trait::async_trait]
pub trait UploadBackend: Send + Sync {
    fn id(&self)    -> &'static str;        // "imgur", used in CLI/config
    fn name(&self)  -> &'static str;        // "Imgur", used in UI

    async fn upload(
        &self,
        image: &[u8],                       // encoded PNG/JPEG
        mime: ImageMime,
        ctx: &UploadContext,                // auth creds from keyring, filename, etc.
    ) -> Result<UploadOutcome, UploadError>;

    async fn delete(&self, handle: &str)    // optional; Imgur supports, 0x0 doesn't
        -> Result<(), UploadError> { Ok(()) }

    fn config_schema(&self) -> serde_json::Value { serde_json::Value::Null }
}

pub enum ImageMime { Png, Jpeg, Webp }

pub struct UploadContext<'a> {
    pub credentials: &'a keyring::Entry,
    pub filename: Option<&'a str>,
    pub progress: tokio::sync::watch::Sender<f32>,
}

pub struct UploadOutcome {
    pub url: String,                        // public URL
    pub delete_handle: Option<String>,      // for backends that support deletion
    pub raw: serde_json::Value,             // full provider response, for diagnostics
}
```

## Tech stack

| Concern              | Crate                                       | Why                                                                        |
| -------------------- | ------------------------------------------- | -------------------------------------------------------------------------- |
| GUI / event loop (default shell) | `eframe` (egui + winit + glow)  | Native multi-window via viewports; immediate mode perfect for live drawing |
| Screen capture       | `xcap`                                        | Best X11/Wayland/macOS/Windows story                                       |
| Image decode/encode  | `image`                                       | De facto; also the cross-toolkit image type used in `Canvas`               |
| Image processing     | `imageproc`                                   | Blur, pixelate helpers (used in core)                                      |
| Text shaping         | `cosmic-text` + `ab_glyph`                       | High-quality text rendering for annotations                                |
| Global hotkeys       | `global-hotkey` (Tauri)                        | X11/macOS/Windows; defer Wayland to portal                                 |
| Wayland hotkeys (P6) | `ashpd` (`GlobalShortcuts` portal)              | xdg-desktop-portal ≥ 1.16                                                  |
| System tray          | `tray-icon` (Tauri)                            | Cross-platform SNI                                                         |
| Clipboard            | `arboard`                                     | Native image clipboard                                                     |
| File dialogs         | `rfd`                                         | Native async dialogs                                                       |
| Single-instance lock | `single-instance` (Tauri)                     | Cross-platform                                                             |
| IPC                  | `interprocess`                                | Local socket / named pipe                                                  |
| IPC encoding         | `postcard`                                    | Compact, serde-compatible                                                  |
| Async                | `tokio`                                       | Upload, IPC, hotkey events                                                 |
| Logging              | `tracing` + `tracing-subscriber`                  | Standard                                                                   |
| Errors               | `thiserror` (libs) + `color-eyre` (bins)         | Library/user split                                                         |
| CLI                  | `clap`                                        | Standard                                                                    |
| Config               | `serde` + `toml_edit` + `notify`                  | Read, preserve comments, hot-reload                                          |
| Secrets              | `keyring`                                     | OS keychain for upload credentials                                         |
| Notifications        | `notify-rust`                                 | Cross-platform desktop notifications                                        |
| COSMIC applet (P3)   | `libcosmic` (with `tokio`, `wayland` features)   | Official COSMIC toolkit                                                    |

All choices are documented in [decision records](adr/).

## Cross-platform concerns

### Linux
- **X11**: full feature set; `global-hotkey` and live overlay both work
- **Wayland**: capture via `xcap` (which uses `xdg-desktop-portal`
  under the hood); first capture per session may show a portal dialog;
  global hotkeys require Phase 6 portal integration — the daemon
  detects this and logs a "bind a compositor keybinding to `flintshot
  region`/`flintshot fullscreen`" notice instead of failing (tray +
  IPC still work); live overlay falls back to editor-window
- **System tray**: `tray-icon` (libappindicator) needs an SNI host; on
  compositors without one the daemon logs a warning and continues
  (best-effort — IPC + hotkeys are unaffected)

### macOS
- **Screen Recording permission** must be granted in System Settings →
  Privacy & Security. `flintshot` detects this on first capture and
  guides the user.
- **Global hotkeys** work via `global-hotkey` (CGEventTap); first use
  may prompt for Accessibility / Input Monitoring permission — grant it
  in System Settings → Privacy & Security.
- **Live overlay** works (borderless fullscreen window).

### Windows
- **Capture** works via DXGI desktop duplication (handled by `xcap`).
- **Global hotkeys** work via `RegisterHotKey` (handled by
  `global-hotkey`).
- **Live overlay** works.
- **Packaging** target is MSI for v1; Scoop bucket as a convenience.

## Known risks & mitigations

| Risk                                                    | Mitigation                                                                                    |
| ------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| Wayland: no global hotkeys in `global-hotkey`           | CLI subcommand + compositor keybind; `ashpd` portal in Phase 6                                  |
| Wayland: live overlay can't grab fullscreen input       | Phase 5 falls back to editor-window on Wayland; honest UX, not a hidden bug                   |
| Wayland: capture needs xdg-desktop-portal (user prompt) | `xcap` handles this; document that first capture per session may show a portal dialog           |
| Multi-window eframe is younger than single-window       | Each window is an independent viewport; root stays alive — well-trodden path in egui examples |
| egui text rendering quality                             | `cosmic-text` for proper shaping; upload glyph atlas per-font-size                              |
| macOS: screen recording permission prompt               | Detect with `xcap`, guide user to System Settings                                               |
| High-DPI / fractional scaling                           | egui + xcap both report scale factors; use `Point`/`Rect` in physical px, render at logical px   |
| `libcosmic` system deps complicate CI                   | Workspace `default-members` excludes `flintshot-cosmic`; separate CI job for it                |
| Abstraction leaks: associated `Image` type on `Canvas`  | Non-object-safe trait (no `dyn Canvas`); we always know concrete type at call site             |
| Future second toolkit hits missing trait methods        | Trait is `#[non_exhaustive]`-style; grow as needed, no premature abstraction                    |

## Testing strategy

### Unit tests
- `flintshot-core`: heavy unit coverage — geometry, annotation
  serialization, IPC roundtrip, config parsing, image ops, blur
  computation, hit-testing, undo/redo state transitions
- `flintshot-editor-egui`: toolkit-specific concerns (texture upload,
  event translation, egui widget behavior)

### Integration tests
- `flintshot-app`: daemon lifecycle, IPC end-to-end, single-instance
  behavior
- Each on a virtual framebuffer where possible (`xvfb` in CI)

### Snapshot tests
- A `MockCanvas` impl in `flintshot-core::render` records drawing
  commands; tests assert the command sequence for known
  `EditorState`s. Toolkit-independent, runs without a GPU.

### Manual test matrix
Tracked in [`TEST_MATRIX.md`](TEST_MATRIX.md). Platforms × interaction
modes × features; live as of Phase 1.

## Build matrix & CI

GitHub Actions, three platforms:

| Job            | Runs                                           | Notes                                                |
| -------------- | ---------------------------------------------- | ---------------------------------------------------- |
| `lint`         | `cargo fmt --check`, `cargo clippy -- -D warnings` | All crates                                           |
| `test`         | `cargo test`                                     | All default-members                                  |
| `msrv`         | `cargo +1.95 build`                              | Verifies MSRV                                        |
| `cosmic-build` | `cargo build -p flintshot-cosmic`                | Linux only; needs `apt install cmake libxkbcommon-dev ...` |
| `cross`        | `cargo build --target x86_64-pc-windows-gnu` etc. | Cross-compile smoke test (no runs)                   |

## Coding conventions

- Follow `tech/rust/` conventions (loaded automatically via
  `.opencode/rules/tech.md`)
- MSRV 1.95 — `#![rustversion::since(1.95)]` gates if we ever need
  newer features
- Workspace-wide `clippy::pedantic` allowed-by-default; opt-in lints
  per module
- All public types get `#[non_exhaustive]` where we might extend them
- All errors go through `thiserror::Error` in libs,
  `color-eyre::Result` in bins
- No `unwrap()` / `expect()` outside tests and `const` contexts; use
  `color-eyre`'s context instead
- Every public function gets a doc comment; private ones don't have to
- Comments only when the "why" isn't obvious; never to restate the code
- **No egui types in `flintshot-core`'s public API** — enforce with a
  dedicated test that scans for `use egui::` in the core crate

## Directory note

The local directory may be named `capture` (from initial scaffolding).
That's incidental — `cargo` uses package names from `Cargo.toml`, not
the parent directory. To rename later: `git mv` the directory and
update any absolute paths in docs.
