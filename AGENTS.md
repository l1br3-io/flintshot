# AGENTS.md

Operational guide for AI agents (and humans who think like one) working
in this repo. Read this first; it tells you how to verify your work.

For product intent, see [`docs/VISION.md`](docs/VISION.md). For layout
and boundaries, see [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). For
sequencing, see [`docs/ROADMAP.md`](docs/ROADMAP.md). For the current
effort's spec, plan, and task checklist, see
[`docs/specs/phase-1-capture-editor/`](docs/specs/phase-1-capture-editor/).

## Project shape

Pure-Rust Cargo workspace. Four crates; only `flintshot-app` builds by
default.

```
crates/
├── flintshot-app/          # bin: CLI + daemon + tray (default-member)
├── flintshot-core/         # lib: types, traits, pure logic — no GUI deps
├── flintshot-editor-egui/  # lib: egui impl of Canvas + EditorShell
└── flintshot-cosmic/       # bin: COSMIC panel applet (opt-in build)
```

Non-negotiable boundary: **no GUI toolkit types in `flintshot-core`'s
public API.** Enforced by `crates/flintshot-core/tests/no_gui_types_in_core.rs`.
If you add `use egui::...` (or `iced`, `slint`, `libcosmic`) to core,
that test must fail.

## Commands

All commands run from the repo root unless noted.

### Build

```sh
cargo build                      # default-members (flintshot-app + deps)
cargo build --all-targets        # default-members + tests/benches/examples
cargo build --workspace          # all crates (incl. flintshot-core tests)
cargo build -p flintshot-cosmic  # opt-in; needs system deps (see below)
```

### Test

```sh
cargo test --workspace --all-targets   # unit + integration, all crates
cargo test --doc -p flintshot-core -p flintshot-editor-egui  # doc tests (lib crates only)
cargo test -p flintshot-core           # just the core crate
```

`cargo test --all-targets` without `--workspace` only exercises
`default-members` (just `flintshot-app`, which currently has no tests).
`cargo test --doc` errors on bin crates (`flintshot-app`,
`flintshot-cosmic`) because they have no library target — scope it to
lib crates explicitly.

### Lint / format (CI gate)

```sh
cargo fmt --all --check                  # formatting check (no writes)
cargo fmt --all                          # apply formatting
cargo clippy --workspace --all-targets -- -D warnings
```

CI runs `RUSTFLAGS="-D warnings"`, so any clippy warning fails CI. Do
not silence with `#[allow(...)]` without a justifying comment. Clippy
needs `--workspace` for the same reason `cargo test` does — otherwise
only `flintshot-app` is directly linted and `flintshot-core`'s lints
go unchecked.

### Run

```sh
cargo run -- --help              # see subcommand surface
cargo run -- capture             # the single Capture action (OS selection UI on
                                 # Wayland; fallback: monitor grab -> editor)
cargo run -- app                 # app window (capture home; auto-starts the daemon)
cargo run -- backends            # capture-backend diagnostics (probes + auto pick)
```

### Release (cargo-dist + cargo-release)

```sh
just dist-plan                   # validate the release plan (fast, no build)
just dist-build                  # build host artifacts + custom formats, no upload
just dist-lint                   # actionlint on owned workflows (release.yml excluded)
just release-dry [minor|patch]   # preview version bump / changelog / tag — no changes
just release [minor|patch]       # ci gate -> plan gate -> bump+tag+push; tag triggers release.yml
```

Boundaries: `release.yml` is cargo-dist's *generated* output — change
`dist-workspace.toml` and regenerate (`dist generate --mode ci`),
never hand-edit (CI checks freshness). Version bumps go through
`just release`, never manual tags. Linux release floor: Debian 12 /
Ubuntu 24.04+ / Fedora 39+ (PipeWire-header constraint; see
`docs/adrs/ADR-0068-release-pipeline.md`). Custom formats live in
`packaging/{linux,macos}/` + `.github/workflows/package-*.yml`
(post-announce jobs; smoke via `packaging-test.yml` dispatch).

### Dev tooling (optional, recommended)

The `justfile` defines `bootstrap`, `watch`, `nextest`, and `dev`
recipes that wrap a few cargo subcommands. They fall back gracefully
when the subcommand is missing, but work best with them installed.
Get them all at once:

```sh
just bootstrap     # installs: cargo-watch, cargo-nextest
```

| Tool             | Used by            | Why                                    |
| ---------------- | ------------------ | -------------------------------------- |
| `cargo-watch`    | `just dev`, `just watch` | Rebuild / re-run on every save.   |
| `cargo-nextest`  | `just test` (preferred), `just nextest` | Faster test runner, nicer UX. |

CI does **not** use `cargo-nextest` — `just ci` and the `test` job in
`.github/workflows/ci.yml` both invoke `cargo test` directly so local
results match CI (`just ci` passes `-q`: quieter output, identical
results). `just test` (without `ci`) prefers
nextest when available for speed; doc tests always run via
`cargo test` since nextest doesn't collect them.

### Linux system deps (default builds)

`flintshot-core` depends on `xcap` for screen capture, which needs
native X11 / Wayland / PipeWire libraries at build time on Linux —
even for `cargo check` or `cargo clippy`. On Debian / Ubuntu:

```sh
sudo apt-get install -y --no-install-recommends \
  pkg-config libclang-dev libxcb1-dev libxrandr-dev \
  libdbus-1-dev libpipewire-0.3-dev libwayland-dev libegl-dev \
  libgbm-dev
```

CI installs these in the `lint`, `test`, and `msrv` jobs (see the
`XCAP_LINUX_DEPS` env in `.github/workflows/ci.yml`); the `tray-build`
job additionally compiles the tray feature path (adds
`libgtk-3-dev` + `libayatana-appindicator3-dev` +
`libxdo-dev` — libxdo is a link-time dep via muda, the tray menu
lib). macOS and
Windows need no extra deps for Phase 1's capture path.

> **Note:** `libgbm-dev` is required at link time even though xcap's
> own README does not list it. The xcap README also omits
> `pkg-config` in some forms; both are included above.

### COSMIC crate system deps (Linux only)

The `flintshot-cosmic` crate is **not** in `default-members` because
`libcosmic` pulls in additional system libraries on top of the xcap
deps above. To build it locally on Debian / Ubuntu:

```sh
sudo apt-get install -y --no-install-recommends \
  cmake libxkbcommon-dev libfontconfig-dev libfreetype6-dev \
  libglib2.0-dev libgfxbuffer-dev pkg-config
cargo build -p flintshot-cosmic
```

CI does this in the dedicated `cosmic-build` job; do not add
`flintshot-cosmic` to default-members.

## Toolchain

- MSRV **1.95**, pinned by `rust-toolchain.toml`. CI verifies with
  `cargo +1.95 build`. Do not use stable-only features past 1.95.
- Edition **2021**.
- `rustfmt.toml`: `max_width = 100`.

## Conventions

- `#![forbid(unsafe_code)]` in every crate. No `unsafe`.
- `#![warn(clippy::pedantic)]` is on by default; allow per-module with
  a comment explaining why.
- `#![warn(missing_docs)]` in `flintshot-core`. Every public item gets
  a doc comment.
- Errors: `thiserror` in libs (`flintshot-core::error` defines
  `FlintshotError` / `Result`), `color_eyre` in bins.
- No `unwrap()` / `expect()` outside tests and `const` contexts.
- **No comments restating code.** Comments only when the "why" is
  non-obvious.
- Public types get `#[non_exhaustive]` where extension is plausible.
- Serde types use `#[serde(rename_all = "kebab-case")]` for TOML/JSON
  fields by convention.

Authoritative style detail lives in
`.opencode/tech/rust/coding-style.md`, `.opencode/tech/rust/patterns.md`,
`.opencode/tech/rust/testing.md`. Read them before non-trivial work.

## Where things live (frequently edited)

| You're touching…                | Go to                                                       |
| ------------------------------- | ---------------------------------------------------------- |
| CLI subcommands                 | `crates/flintshot-app/src/cli.rs`                          |
| App window (page router)        | `crates/flintshot-app/src/app_window.rs` (`Page`, epochs, pump, grab router) + `flintshot-editor-egui/src/home.rs` |
| Shared grab flow (worker + app) | `crates/flintshot-app/src/capture_flow.rs`                 |
| Geometry / data model           | `crates/flintshot-core/src/{geom,editor}.rs`              |
| Session capture store           | `crates/flintshot-core/src/session.rs` (see ADR-0067; app wiring in `app_window.rs`'s `Shared`) |
| Capture backends / registry     | `crates/flintshot-core/src/capture/` (`Capturer` trait, `xcap` + portal, `select_backend`) |
| `Canvas` / `EditorShell` traits | `crates/flintshot-core/src/{render,shell}.rs`             |
| Upload `Backend` trait          | `crates/flintshot-core/src/upload.rs`                     |
| egui editor implementation      | `crates/flintshot-editor-egui/src/`                        |
| Cross-crate types re-exports    | `crates/flintshot-core/src/lib.rs`                         |
| CI pipeline                     | `.github/workflows/ci.yml`                                 |
| Release pipeline (cargo-dist)   | `dist-workspace.toml`, `release.toml`, `.github/workflows/release.yml` (generated) |
| Custom packaging (deb/rpm/AppImage/dmg) | `packaging/{linux,macos}/`, `.github/workflows/package-*.yml` |
| Tech stack rationale            | `docs/adrs/`                                               |

## When you finish a change

Run, in order, before declaring done:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
```

If the change is user-visible or modifies the workspace shape, update
`CHANGELOG.md` under `## [Unreleased]`. If it adds or revises a
non-obvious technical choice, add an ADR entry in `docs/adrs/`
(numbered `ADR-NNNN`, format: **Context · Decision · Consequences**).

If a `cargo` command is missing from this file, ask the maintainer and
then add it here so the next agent knows.
