# flintshot

Cross-platform screenshot capture and annotation tool, written in pure Rust.
Inspired by [Flameshot](https://flameshot.org), designed to be fast, modular,
and friendly to the [COSMIC desktop](https://system76.com/cosmic).

> **Status:** Phase 1 (capture + editor window) in progress. Basic capture works;
> annotation tools and save/copy coming soon. See [`SPEC.md`](SPEC.md) for the
> current phase spec, or [`docs/ROADMAP.md`](docs/ROADMAP.md) for the milestone index.

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

- ✅ Capture fullscreen/region (via xcap)
- ✅ Open editor window
- ✅ Config file at `~/.config/flintshot/config.toml`

### Coming soon (Phase 1)

- ⏳ Annotation tools (rectangle, arrow, text, blur)
- ⏳ Save to file (native dialog)
- ⏳ Copy to clipboard
- ⏳ Undo/redo

See [`SPEC.md`](SPEC.md) for the current phase spec, or
[`docs/ROADMAP.md`](docs/ROADMAP.md) for the milestone index.

## Documentation

Product & design:

- [`docs/VISION.md`](docs/VISION.md) — product vision, scope, non-goals
- [`docs/ROADMAP.md`](docs/ROADMAP.md) — milestone index (thin)
- [`SPEC.md`](SPEC.md) — current phase requirements (Phase 1)
- [`tasks/plan.md`](tasks/plan.md) · [`tasks/todo.md`](tasks/todo.md) — current phase plan & checklist
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — workspace, crates, dependencies, data model
- [`docs/adr/`](docs/adr/) — technical decision records (ADRs)
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
