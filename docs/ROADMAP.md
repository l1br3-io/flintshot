# Roadmap

Six phases. Each is independently shippable and produces a usable artifact.
Effort estimates are rough, solo-developer assumes; adjust to your own pace.

## Versioning

- `0.x.y` throughout development; no API stability guarantees
- `1.0.0` once Phase 4 (upload) lands and we have at least 30 days of
  dogfooding
- Post-1.0 semver: breaking crate API changes bump minor; user-visible
  behavior changes bump minor; fixes bump patch

## Phase 0 — Scaffolding

**Goal:** a building workspace with empty crates, the UI abstraction
layer in place, CI green, docs in place.

**Deliverables:**
- [ ] `Cargo.toml` workspace with 4 members: `flintshot-app`,
      `flintshot-core`, `flintshot-editor-egui`, `flintshot-cosmic`
- [ ] `default-members = ["crates/flintshot-app"]` so casual builds skip
      the COSMIC crate
- [ ] `rust-toolchain.toml` pinning 1.95
- [ ] `clap` CLI in `flintshot-app` with subcommands: `daemon`, `region`,
      `fullscreen`, `pin`, `upload` (all stubs)
- [ ] `flintshot-app` feature flags: `default = ["egui"]`, with optional
      dep on `flintshot-editor-egui` (room for future `iced`, `slint`
      features)
- [ ] `Backend` trait + types in `flintshot-core` (no impls yet)
- [ ] Geometry types in `flintshot-core::geom` — `Point`, `Rect`,
      `Color`, `Stroke`, `Key` (replaces egui types in shared data
      model)
- [ ] `Canvas` + `EditorShell` traits in `flintshot-core` (signatures
      only, no impls)
- [ ] Stub `flintshot-editor-egui` impl of `Canvas` + `EditorShell`
      (returns `todo!()`)
- [ ] `tracing` + `color-eyre` init
- [ ] CI workflow: `fmt --check`, `clippy -- -D warnings`, `cargo test`,
      MSRV check
- [ ] CI workflow: opt-in `cargo build -p flintshot-cosmic` (separate
      job, needs system deps)
- [ ] `LICENSE-MIT` + `LICENSE-APACHE` files,
      `license = "MIT OR Apache-2.0"` in every Cargo.toml
- [ ] `README.md` at root
- [ ] Docs in place (this file + VISION + ARCHITECTURE + DECISIONS)

**Exit criteria:** `cargo build` and `cargo build -p flintshot-cosmic`
both succeed on a clean checkout; CI is green; `flintshot --help` lists
all subcommands; the trait abstractions compile cleanly with no egui
types leaking into `flintshot-core`.

**Rough effort:** 1–2 days.

---

## Phase 1 — Capture + editor window

**Goal:** `flintshot region` opens an editor window over a screenshot and
lets the user annotate, save, or copy.

**Deliverables:**
- [ ] `xcap` integration for single-monitor capture
- [ ] `flintshot-editor-egui`: eframe window implementing `EditorShell`;
      screenshot uploaded via `Canvas::upload_image`
- [ ] All annotation drawing routed through the `Canvas` trait from day
      one — no direct `egui::Painter` calls for annotations
- [ ] Tools: **Select (region crop), Rectangle, Arrow, Text, Blur**
- [ ] Color and stroke-width picker, persisted in config
- [ ] Outputs: **Save** (`rfd` dialog), **Copy to clipboard** (`arboard`)
- [ ] Undo/redo with snapshot history
- [ ] Keyboard: `Esc` cancel, `Ctrl+S` save, `Ctrl+C` copy,
      `Ctrl+Z`/`Ctrl+Y` undo/redo
- [ ] Initial config file at platform-appropriate location (`dirs` crate)
- [ ] `cosmic-text` for text annotation rendering
- [ ] HiDPI-aware coordinates

**Exit criteria:** A user can run `flintshot region`, drag-select a
region, draw a red rectangle with an arrow pointing at it, add a text
label, save the result as a PNG, and have it land in their clipboard
too. The abstraction holds: no egui types leak into `flintshot-core`.

**Rough effort:** 1–2 weeks.

---

## Phase 2 — Tray, hotkeys, multi-monitor, polish

**Goal:** Flameshot-style daemon behaviour and complete annotation toolset.

**Deliverables:**
- [ ] `flintshot daemon` with `tray-icon` menu (Region / Fullscreen /
      Open last / Settings / Quit)
- [ ] `single-instance` lock — second `flintshot daemon` exits cleanly
      and signals the first
- [ ] IPC protocol via `interprocess` local socket
- [ ] `global-hotkey` integration on X11, macOS, Windows (configurable
      keybindings in TOML)
- [ ] Multi-monitor capture (combined image, awareness of which monitor
      the cursor is on)
- [ ] Tools: **Ellipse, Line, Marker, Pencil, Move/select existing
      annotation**
- [ ] Magnifier on region selection (zoomed pixel view)
- [ ] Auto-incrementing number bubbles

**Exit criteria:** User starts `flintshot daemon` at login, presses a
global hotkey, gets the editor on the active monitor, annotates, and
saves — without ever launching from a terminal.

**Rough effort:** 1 week.

**Wayland caveat:** Global hotkeys won't work on Wayland here; users
bind `flintshot region` to a compositor shortcut. Phase 6 closes this
gap via `ashpd`.

---

## Phase 3 — Pin-to-desktop + COSMIC applet

**Goal:** Floating pinned screenshots + native COSMIC panel integration.

**Deliverables:**
- [ ] `flintshot pin <path>`: borderless, always-on-top, draggable,
      resizable floating window via eframe viewport (routed through
      `EditorShell::run_pin`)
- [ ] Pin launched from the editor's "Pin" button or tray menu
- [ ] Multiple pins can coexist; each is independent
- [ ] **COSMIC applet implementation** (`flintshot-cosmic`):
  - Panel icon, click → popup
  - Popup buttons: Region / Fullscreen / Window / Open last / Pin last /
    Upload last / Settings
  - Each button sends IPC to the daemon (reuses `flintshot-core` IPC
    types)
  - `.desktop` file with `X-COSMIC-Applet=true`, metainfo with
    `com.system76.CosmicApplet` provides id
  - SVG panel icon
  - `i18n-embed` for localization (English first)

**Exit criteria:** User pins three screenshots from the editor and
arranges them on their desktop; a COSMIC user clicks the panel icon,
picks "Region", and the daemon opens the editor.

**Rough effort:** 1 week (3–5 days for pin + 3–4 days for applet).

---

## Phase 4 — Upload

**Goal:** Upload annotated screenshots to a hosting provider, with Imgur
shipping first.

**Deliverables:**
- [ ] `flintshot-upload` module in `flintshot-core` (or split crate if
      large)
- [ ] `Backend` trait (already designed in Phase 0) implemented for
      Imgur
- [ ] Credentials stored via `keyring` (never in config file)
- [ ] Upload progress reported via IPC to tray/applet
- [ ] On success: URL copied to clipboard, optional notification via
      `notify-rust`
- [ ] On failure: error surfaced in UI, retry button
- [ ] Backend selectable via `--backend=imgur` CLI flag or config
- [ ] Imgur delete-hash stored locally for 30 days (allows deletion
      from tray menu)

**Exit criteria:** User annotates a screenshot, clicks Upload,
authenticates once via OAuth, gets an Imgur URL in their clipboard, and
can delete the upload later from the tray menu.

**Rough effort:** 3–5 days.

---

## Phase 5 — Live overlay (Flameshot-style)

**Goal:** Optional fullscreen transparent overlay where the user draws
in place over the actual screen.

**Deliverables:**
- [ ] Per-monitor fullscreen transparent eframe viewports
- [ ] Same `EditorState` model as editor-window mode — code reuse, not
      a separate UI
- [ ] Annotation drawing routed through the same `Canvas` trait; the
      overlay is just another shell surface
- [ ] Cross-monitor selection (drag from one monitor to another)
- [ ] User-selectable default mode in config
- [ ] **Wayland fallback:** capture via xdg-desktop-portal → open
      editor window (no true overlay)
- [ ] Documentation clearly distinguishing behavior on X11 vs Wayland
      vs macOS vs Windows

**Exit criteria:** On X11, the user presses a hotkey and draws a
selection rectangle *on top of* their actual screen, in real time; the
result is the cropped, annotated image.

**Rough effort:** 1–2 weeks.

**Known hard part:** Wayland compositors don't allow arbitrary
fullscreen input-grab windows. The fallback path is mandatory, not
optional.

---

## Phase 6 — Wayland global hotkeys via `ashpd`

**Goal:** First-class global hotkey support on Wayland compositors that
support the `GlobalShortcuts` portal.

**Deliverables:**
- [ ] `ashpd` integration for
      `org.freedesktop.portal.GlobalShortcuts`
- [ ] Runtime detection of portal support (KDE, Hyprland, GNOME with
      extension)
- [ ] Compositor-assisted keybinding setup flow
- [ ] Graceful fallback: if portal unsupported, log a clear message
      and tell the user to bind a compositor shortcut manually

**Exit criteria:** On Plasma 6 Wayland, the user binds a global
shortcut via the portal prompt and `flintshot region` fires when they
press it.

**Rough effort:** 3 days.

---

## Stretch goals (post-1.0)

- OCR via `tesseract` or `rusty-tesseract`
- GIF/short-video capture (probably out of scope; consider
  `gpu-screen-recorder` integration instead)
- Annotation presets (named combinations of color + width + font)
- Plugin system for `Backend` trait (dylib or WASM?)
- Flatpak packaging with `xdg-desktop-portal` integration baked in
- Windows packaging via MSI, macOS via notarized `.app`
- Mobile companion app (very speculative)
- Alternative editor shells: `flintshot-editor-iced`,
  `flintshot-editor-slint` (exercise the abstraction)
