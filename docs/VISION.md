# Vision

## One-liner

`flintshot` is a fast, modular, cross-platform screenshot capture and
annotation tool, written in pure Rust, with first-class COSMIC desktop
integration.

## Why build this

The Rust desktop ecosystem has matured enough to challenge the established
screenshot tools:

- **Flameshot** (C++/Qt) — the de-facto standard, but its codebase is hard
  to extend, builds slowly, and its Wayland support is bolted on.
- **Satty** (Rust + GTK4) — proves Rust is viable, but depends on the GTK
  runtime and centres its UX on the editor-window model only.
- **GNOME Screenshot / Spectacle / Snipping Tool** — competent but tightly
  bound to their desktop environment.

`flintshot` is built from the ground up for:

1. **Pure-Rust maintainability** — no C++/Qt or GTK runtime, single static
   binary per platform.
2. **Real cross-platform parity** — Linux (X11 + Wayland), macOS, Windows
   all first-class.
3. **Both UX models** — Flameshot-style live overlay *and* Satty-style
   editor window, because each shines in different contexts.
4. **COSMIC as a peer citizen** — a libcosmic-based panel applet, not just
   a tray icon.
5. **Pluggable upload** — Imgur first, but designed so that 0x0, SFTP,
   custom backends are first-class.

## Target users

- Power users on Linux/Wayland who want a Flameshot-grade tool that
  actually works on modern compositors
- COSMIC desktop users who want native-feeling panel integration
- Cross-platform developers who want the same tool on every OS
- People who hate Electron — this is one binary, no webview

## Design principles

1. **Pure Rust, single binary** — no runtimes beyond libc and the
   platform's native windowing.
2. **Editor-window first, overlay later** — robustness over flair; ship the
   model that works everywhere first.
3. **Modular crates** — the editor, capture logic, IPC, and COSMIC applet
   are independent. None of them depend on the others' GUI toolkit.
4. **Battle-tested dependencies** — prefer mature crates from established
   authors (Tauri team, emilk, image-rs, etc.) over rolling our own.
5. **Honesty about platform limits** — if Wayland can't do global hotkeys
   without a portal, document it; don't pretend.
6. **Battle-tested over novel** — every dependency must have a clear
   maintenance story and a real user base.
7. **UI-toolkit abstraction via traits** — egui is the default, not the
   only option. Core owns the types and pure logic (`Point`, `Rect`,
   `Color`, `EditorState`, `Annotation`); toolkits implement `Canvas`
   and `EditorShell` traits. Adding a new toolkit (iced, slint, etc.)
   is purely additive.

## Feature surface (target parity with Flameshot)

### Capture
- Full screen, current screen, region, active window, with delay
- Multi-monitor (combined or per-monitor)
- HiDPI and fractional-scale aware

### Annotation
- Rectangle, ellipse, line, arrow, freehand pencil, marker (highlight)
- Text with proper shaping (`cosmic-text`)
- Blur and pixelate
- Auto-incrementing number bubbles
- Move, resize, undo/redo, magnifier on selection
- Color and stroke-width picker, saved across sessions

### Outputs
- Save to file (native dialog via `rfd`)
- Copy to clipboard (`arboard`)
- Pin as floating always-on-top window
- Upload (Imgur first; pluggable `Backend` trait)
- Open with external app
- Notification on completion (`notify-rust`)

### System integration
- Background daemon (`flintshot daemon`)
- System tray (`tray-icon`) with quick actions
- Global hotkeys (`global-hotkey` on X11/macOS/Windows; `ashpd`
  GlobalShortcuts portal on Wayland)
- Single-instance lock
- TOML config at `~/.config/flintshot/config.toml` (or platform equivalent)
- Optional COSMIC panel applet (libcosmic/iced)

### UX
- Two interaction models, user-selectable:
  - **Editor-window** (default): capture → editor opens → annotate → save
  - **Live overlay** (Phase 5): fullscreen transparent window, draw in place

## Non-goals

- **Video / screen recording** — out of scope; use OBS
- **OCR** — out of scope; integrate with `tesseract` externally if needed
- **Self-hosting image hosting** — use a `Backend` plugin
- **Mobile platforms** — desktop only
- **WebAssembly** — out of scope
- **Replacing Flameshot for everyone** — we'd be happy if Flameshot users
  switch, but we're not chasing feature parity with every Flameshot edge
  case

## Success criteria

- v1 ships: capture + editor + tray + hotkeys + pin + upload, on all
  three OSes
- Cold-start-to-capture latency under 200ms on commodity hardware
- Single static binary per platform under 25 MB (release, stripped)
- Works on at least: GNOME (Wayland), KDE Plasma (Wayland), COSMIC,
  Sway/Hyprland, macOS 14+, Windows 10/11
- A clear contribution story: a new annotation tool can be added in
  < 200 LOC

## Naming

`flintshot` — a flint strikes a spark, captures a moment. Fire theme
continues from Flameshot; the name is short, available on crates.io, and
trademark-free.

## Where this fits

This vision is the top of the planning hierarchy — the why. Direction,
milestones, and the delivery lifecycle (DEFINE → PLAN → BUILD → VERIFY →
REVIEW → SHIP) live in [`ROADMAP.md`](./ROADMAP.md).
