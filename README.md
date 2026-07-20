# flintshot

Cross-platform screenshot capture and annotation tool, written in pure Rust.
Inspired by [Flameshot](https://flameshot.org), designed to be fast, modular,
and friendly to the [COSMIC desktop](https://system76.com/cosmic).

> **Status:** Planning. The project is in active design — no code yet.
> See [`docs/ROADMAP.md`](docs/ROADMAP.md) for the phased delivery plan.

## At a glance

- **Pure-Rust stack**: eframe/egui for UI, xcap for capture, tokio for async
- **Cross-platform**: Linux (X11 + Wayland), macOS, Windows
- **Two interaction models**: editor-window (Satty-style, ships first) +
  live overlay (Flameshot-style, Phase 5)
- **COSMIC panel applet** as an alternate front-end (Phase 3)
- **Pluggable upload backends**: Imgur ships first; `Backend` trait designed
  for extension

## Documentation

Product & design:

- [`docs/VISION.md`](docs/VISION.md) — product vision, scope, non-goals
- [`docs/ROADMAP.md`](docs/ROADMAP.md) — phased delivery plan
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — workspace, crates, dependencies, data model
- [`docs/DECISIONS.md`](docs/DECISIONS.md) — technical decision records (ADRs)
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
