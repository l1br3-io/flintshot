---
status: shipped
started: 2026-07-20
shipped: 2026-07-20
---

# Spec — Phase 0: Scaffolding

> Shipped 2026-07-20. Phase 0 predates the plan/todo system; this spec
> is extracted from the original roadmap and stands alone (no
> `plan.md` / `todo.md` paired with it). For the project narrative, see
> [`../../VISION.md`](../../VISION.md); for current direction, see
> [`../../ROADMAP.md`](../../ROADMAP.md).

## Goal

A building workspace with empty crates, the UI abstraction layer in
place, CI green, docs in place.

## Deliverables

- [x] `Cargo.toml` workspace with 4 members: `flintshot-app`,
      `flintshot-core`, `flintshot-editor-egui`, `flintshot-cosmic`
- [x] `default-members = ["crates/flintshot-app"]` so casual builds skip
      the COSMIC crate
- [x] `rust-toolchain.toml` pinning 1.95
- [x] `clap` CLI in `flintshot-app` with subcommands: `daemon`, `region`,
      `fullscreen`, `pin`, `upload` (all stubs)
- [x] `flintshot-app` feature flags: `default = ["egui"]`, with optional
      dep on `flintshot-editor-egui` (room for future `iced`, `slint`
      features)
- [x] `Backend` trait + types in `flintshot-core` (no impls yet)
- [x] Geometry types in `flintshot-core::geom` — `Point`, `Rect`,
      `Color`, `Stroke`, `Key` (replaces egui types in shared data
      model)
- [x] `Canvas` + `EditorShell` traits in `flintshot-core` (signatures
      only, no impls)
- [x] Stub `flintshot-editor-egui` impl of `Canvas` + `EditorShell`
      (returns `FlintshotError::NotImplemented`)
- [x] `tracing` + `color-eyre` init
- [x] CI workflow: `fmt --check`, `clippy -- -D warnings`, `cargo test`,
      MSRV check
- [x] CI workflow: opt-in `cargo build -p flintshot-cosmic` (separate
      job, needs system deps)
- [x] `LICENSE-MIT` + `LICENSE-APACHE` files,
      `license = "MIT OR Apache-2.0"` in every Cargo.toml
- [x] `README.md` at root
- [x] Docs in place (VISION + ROADMAP + ARCHITECTURE + DECISIONS)
- [x] Operational docs: `CONTRIBUTING.md`, `AGENTS.md`, `CHANGELOG.md`,
      `docs/TEST_MATRIX.md`

## Exit criteria

`cargo build` and `cargo build -p flintshot-cosmic` both succeed on a
clean checkout; CI is green; `flintshot --help` lists all subcommands;
the trait abstractions compile cleanly with no egui types leaking into
`flintshot-core`.
