# flintshot

Cross-platform screenshot capture and annotation tool, written in pure Rust.
Inspired by [Flameshot](https://flameshot.org), designed to be fast, modular,
and friendly to the [COSMIC desktop](https://system76.com/cosmic).

> **Status:** capturing, annotating, and sharing all work. See
> [`docs/ROADMAP.md`](docs/ROADMAP.md) for the milestone index.

## At a glance

- **Pure-Rust stack**: eframe/egui for UI, xcap for capture, tokio for async
- **Cross-platform**: Linux (X11 + Wayland), macOS, Windows
- **Two interaction models**: editor-window (Satty-style, ships first) +
  live overlay (Flameshot-style, Phase 5)
- **COSMIC panel applet** as an alternate front-end (Phase 3)
- **Pluggable upload backends**: Imgur ships first; `Backend` trait designed
  for extension

## Getting started

### Prerequisites

- **Rust 1.95+** (auto-selected via `rust-toolchain.toml`)
- **Linux:** X11/Wayland dev libraries for screen capture:
  ```bash
  sudo apt-get install -y pkg-config libclang-dev libxcb1-dev libxrandr-dev \
    libdbus-1-dev libpipewire-0.3-dev libwayland-dev libegl-dev libgbm-dev
  ```
- **macOS / Windows:** No extra deps needed

### Run the app

```bash
# Build
cargo build

# Capture fullscreen and open editor
cargo run -- fullscreen

# Capture with delay (useful for setting up the screen)
cargo run -- region --delay-ms 2000

# Capture and save directly (skip editor)
cargo run -- region --out /tmp/screenshot.png

# See all options
cargo run -- --help
cargo run -- region --help
```

### Daemon, tray & hotkeys (Phase 2)

Run the resident daemon for the system tray, global hotkeys, and IPC
(the tray needs `--features tray`, which pulls GTK on Linux):

```sh
cargo run -- daemon                 # tray* + hotkeys + IPC server
cargo run -- daemon --replace       # take over from a running daemon
cargo run --features tray -- daemon # with the system tray enabled
```

`flintshot region` / `fullscreen` delegate to a running daemon when one
is present (centralising the event stream) and fall back to standalone
otherwise; `--standalone` forces the in-process path.

> **Wayland hotkeys:** on a Wayland session the daemon binds global
> hotkeys through the xdg-desktop-portal `GlobalShortcuts` interface
> (requires xdg-desktop-portal ≥ 1.16 and a frontend that implements
> it — KDE Plasma, and COSMIC where the packaged portal lists it).
> Run `flintshot hotkeys bind` (or use the tray's *Bind shortcuts…*)
> once to open the portal bind dialog; `[hotkeys]` chords are offered
> as suggestions. On desktops without that interface (GNOME, sway),
> the daemon logs a notice and the compositor-keybinding workaround
> still applies: bind it to `flintshot region` / `flintshot
> fullscreen`. See
> [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) §Cross-platform
> concerns and
> [`docs/adrs/ADR-0029`](docs/adrs/ADR-0029-ashpd-global-shortcuts.md).

### Development

For contributors. The `justfile` wraps the common loop; see
[`CONTRIBUTING.md`](CONTRIBUTING.md) for the full workflow.

```bash
just bootstrap     # install cargo-watch + cargo-nextest (one-time)
just dev           # auto-rebuild + re-run on file changes
just ci            # run the same checks CI runs, locally
just test          # tests (prefers nextest when installed)
```

Dev shortcuts exist for capture subcommands too — e.g.
`just fullscreen`, `just region --delay-ms 500`. See
`just --list` for the full surface.

### What works right now

- Capture fullscreen/region (via xcap)
- Editor window with annotation tools (rectangle, arrow, text, blur, ellipse, line, marker, pencil, number)
- Save to file / copy to clipboard
- Config file at `~/.config/flintshot/config.toml`
- Daemon with tray, hotkeys, and IPC server

### Coming next (Phase 3b / 4)

- COSMIC panel applet
- Upload backends (Imgur, …)

See [`docs/ROADMAP.md`](docs/ROADMAP.md) for the milestone index.

## Documentation

Product & design:

- [`docs/VISION.md`](docs/VISION.md) — product vision, scope, non-goals
- [`docs/ROADMAP.md`](docs/ROADMAP.md) — milestone index (thin)
- Git history — the spec trail: each effort's goal, decisions, deviations, and
  outcome live in enriched commit messages. Look up any effort with
  `git log --grep='<effort>'` (e.g. `git log --grep=phase-1-capture-editor`)
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — workspace, crates, dependencies, data model
- [`docs/adrs/`](docs/adrs/) — technical decision records (ADRs)
- [`docs/REFERENCES.md`](docs/REFERENCES.md) — open-source reference projects (comparables, agents, chat/streaming UI, Rust AI infra)
- [`docs/research/`](docs/research/) — dated, source-cited research notes backing roadmap decisions
- [`docs/TEST_MATRIX.md`](docs/TEST_MATRIX.md) — cross-platform manual QA grid

Working in the repo:

- [`CONTRIBUTING.md`](CONTRIBUTING.md) — dev setup, workflow, code style
- [`AGENTS.md`](AGENTS.md) — operational guide for AI agents (build/test/lint commands)
- [`CHANGELOG.md`](CHANGELOG.md) — release history

## Project name vs directory

The project is **`flintshot`** (the binary you'll install and the crates you'll
depend on). The local directory may still be named `capture` from initial
scaffolding — that's incidental; cargo package names are independent of the
directory.

## License

Dual-licensed under `MIT OR Apache-2.0`, at your option.
