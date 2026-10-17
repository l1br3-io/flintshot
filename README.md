# flintshot

<p align="center">
  <img src="assets/icons/hicolor/scalable/apps/flintshot.svg" width="110" alt="flintshot logo" />
</p>

**Cross-platform screenshot capture and annotation, in pure Rust.**
Inspired by [Flameshot](https://flameshot.org), native to
[Wayland and the COSMIC desktop](https://system76.com/cosmic), at home
on X11, macOS, and Windows too.

[![CI](https://github.com/l1br3-io/flintshot/actions/workflows/ci.yml/badge.svg)](https://github.com/l1br3-io/flintshot/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/l1br3-io/flintshot)](https://github.com/l1br3-io/flintshot/releases/latest)
[![License](https://img.shields.io/github/license/l1br3-io/flintshot)](#license)
<!-- Add after the crates.io publish lands (packaging/RELEASE.md §4):
     [![crates.io](https://img.shields.io/crates/v/flintshot-app)](https://crates.io/crates/flintshot-app)
-->

## Install

One-liners from the latest [GitHub Release](https://github.com/l1br3-io/flintshot/releases/latest)
(**v0.1.1** is the first public release):

**Linux / macOS**

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/l1br3-io/flintshot/releases/download/v0.1.1/flintshot-app-installer.sh | sh
```

**Windows** (PowerShell)

```powershell
irm https://github.com/l1br3-io/flintshot/releases/download/v0.1.1/flintshot-app-installer.ps1 | iex
```

Package managers — the same release artifacts, each channel's own
entry point:

| Channel | Command |
|---|---|
| winget (Windows) | `winget install --id l1br3.flintshot --source winget` |
| Homebrew | `brew install l1br3-io/tap/flintshot` |
| cargo-binstall | `cargo binstall flintshot-app` *(installs the `flintshot` binary)* |
| Arch Linux (AUR) | `yay -S flintshot-bin` |
| deb / rpm / AppImage | download from the [release page](https://github.com/l1br3-io/flintshot/releases/latest) |
| Flathub / COSMIC Store | *in progress* — see [`packaging/flatpak/`](packaging/flatpak/) |

## What you get

- **Capture through the OS's own selection UI** where the platform
  has one: the XDG Screenshot Portal on Wayland (COSMIC, KDE, GNOME)
  with region select and confirmation, native grabs elsewhere
  (X11, macOS, Windows). `flintshot capture` is the single action;
  `flintshot backends` shows what `auto` picks on your machine.
- **A real editor**, not a paste-and-pray dialog: select, crop,
  shapes, arrows, freehand, text (with re-edit), numbered steps,
  blur/redact, resize, an edit *history* timeline, undo/redo — every
  gesture undoable. Unsaved captures survive navigation within a
  session (the app window's "This session" row).
- **A daemon when you want one**: system tray, global hotkeys (the
  Wayland GlobalShortcuts portal where supported, X11 grabs
  elsewhere), IPC for tray/applet front-ends — and everything works
  standalone without it.
- **An app window** in the Windows 11 Snipping Tool spirit: capture
  controls, recent captures with thumbnails, settings, AI settings.
- **Uploads**: imgur ships first (`flintshot auth imgur` stores the
  credential in the OS keyring — never in a config file).
- **Ask-the-shot AI**: send what you see to any OpenAI-compatible
  vision model (OpenAI, xAI, Gemini, Ollama, LM Studio, OpenRouter),
  read the streamed answer in-app, save reusable prompt transforms.
  Your API key lives in the OS keyring.
- **COSMIC panel applet** as an alternate front-end (opt-in build —
  `just cosmic`).

<!-- Screenshots: capture with flintshot itself, drop PNGs in
     docs/screenshots/, then uncomment:

## Screenshots

| Region capture + editor | Ask-the-shot AI | The app window |
|---|---|---|
| ![](docs/screenshots/editor.png) | ![](docs/screenshots/ai.png) | ![](docs/screenshots/home.png) |
-->

## The CLI

```
flintshot capture            # the OS selection UI (or fallback grab), then the editor
flintshot app                # the app window; auto-starts the daemon
flintshot daemon             # tray + global hotkeys + IPC (build with --features tray)
flintshot open IMG.png       # edit an existing image
flintshot upload IMG.png     # upload via a configured backend, prints the URL
flintshot auth imgur         # store/clear backend credentials (OS keyring)
flintshot hotkeys bind       # bind the capture hotkey via the Wayland portal
flintshot config             # print the resolved configuration
flintshot backends           # capture-backend probe report
```

`flintshot <command> --help` for the full flag surface;
`flintshot app --route settings` cold-starts on any page.

## Building from source

Rust **1.95+** is auto-selected by `rust-toolchain.toml`. A default
build (editor + capture, no tray) needs no system libraries beyond a
C toolchain. The **tray** feature pulls the GTK stack on Linux:

```sh
# minimal build
cargo build --release

# with the system tray (Linux)
sudo apt-get install -y pkg-config libclang-dev libxcb1-dev libxrandr-dev \
  libdbus-1-dev libpipewire-0.3-dev libwayland-dev libegl-dev libgbm-dev \
  libgtk-3-dev libayatana-appindicator3-dev libxdo-dev
cargo build --release --features tray
```

[`just`](https://github.com/casey/just) drives everything:
`just ci` (the full gate), `just daemon --refresh` (dev loop with
tray), `just install-user` (deploy + restart the daemon). See the
[justfile](justfile) for the full surface.

## Platform notes

- **Wayland**: capture rides the XDG Screenshot Portal (prompt-free
  region select on COSMIC/KDE). Global hotkeys bind through the
  GlobalShortcuts portal (`flintshot hotkeys bind`); on compositors
  without it, bind a compositor keybinding to `flintshot capture`.
- **X11**: global-hotkey grabs work out of the box.
- **macOS / Windows**: native capture paths; editor and AI are
  fully cross-platform.
- **Flatpak**: packaging in progress — see
  [`packaging/flatpak/`](packaging/flatpak/).

## Documentation

Product & design:

- [`docs/VISION.md`](docs/VISION.md) — product vision, scope, non-goals
- [`docs/ROADMAP.md`](docs/ROADMAP.md) — milestone index
- Git history — the spec trail: each effort's goal, decisions,
  deviations, and outcome live in enriched commit messages
  (`git log --grep='<effort>'`).
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — workspace, crates,
  dependencies, data model
- [`docs/adrs/`](docs/adrs/) — technical decision records (ADRs)
- [`docs/research/`](docs/research/) — dated, source-cited research
  notes backing roadmap decisions

Working in the repo:

- [`CONTRIBUTING.md`](CONTRIBUTING.md) — dev setup, workflow, style
- [`AGENTS.md`](AGENTS.md) — operational guide for AI agents
- [`CHANGELOG.md`](CHANGELOG.md) — release history

## Status

First public release **v0.1.1** is out.
[`docs/ROADMAP.md`](docs/ROADMAP.md) tracks what's queued next
(export formats, watermarking, crash-resume session restore, a
scripting surface).

## License

Dual-licensed under [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE) — your choice.
