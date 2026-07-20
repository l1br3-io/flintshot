# Contributing to flintshot

Thanks for considering a contribution. This file is the short path from
"interested" to "merged." Design context lives in [`docs/`](docs/);
read at least [`docs/VISION.md`](docs/VISION.md) and
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) before non-trivial work.

## Project status

Phase 0 (scaffolding) is in place; Phase 1 (capture + editor window)
is the active frontier. See [`docs/ROADMAP.md`](docs/ROADMAP.md) for
what each phase covers and what's shippable today.

Subcommands in `flintshot-app/src/cli.rs` carry a `[Phase N]` tag in
their help text indicating when they're meant to do real work. Don't
implement ahead of the roadmap without coordinating — the surrounding
plumbing may not exist yet.

## Development setup

Requirements:

- Rust **1.95** (pinned via `rust-toolchain.toml`; `rustup` picks it up
  automatically)
- A C toolchain (for native deps like `xcap`'s platform bindings)
- Linux: typical X11 / Wayland dev packages if you want to run captures
- macOS / Windows: the standard Rust toolchain is enough for Phase 1

Clone and verify:

```sh
git clone https://github.com/anomalyco/flintshot
cd flintshot
cargo build
cargo test --all-targets
cargo run -- --help
```

If you intend to touch `flintshot-cosmic`, also install its system deps
(see [`AGENTS.md`](AGENTS.md#cosmic-crate-system-deps-linux-only) or the
`cosmic-build` job in `.github/workflows/ci.yml`).

## Daily workflow

```sh
cargo fmt --all                 # format
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo test --doc
```

CI runs all four with `-D warnings`; matching that locally saves a
round-trip.

## Where to put things

| Change                                | Location                                                  |
| ------------------------------------- | --------------------------------------------------------- |
| New annotation tool / editor logic    | `crates/flintshot-core/src/editor.rs`                     |
| New geometry primitive                | `crates/flintshot-core/src/geom.rs`                       |
| New `Canvas` / `EditorShell` methods  | `crates/flintshot-core/src/{render,shell}.rs`             |
| egui-specific rendering               | `crates/flintshot-editor-egui/src/`                       |
| New CLI subcommand or arg             | `crates/flintshot-app/src/cli.rs`                         |
| New upload backend                    | impl `UploadBackend` — usually `crates/flintshot-core/src/upload.rs` or a new module |
| Config fields                         | `flintshot-core` config schema, plus `docs/ARCHITECTURE.md` config sample |
| A non-obvious technical decision      | new ADR in `docs/DECISIONS.md`                            |

## Code style (short version)

- `#![forbid(unsafe_code)]` everywhere. No `unsafe`.
- Public types in `flintshot-core` get doc comments (`#![warn(missing_docs)]`).
- Default to private; widen visibility only when an external caller needs it.
- Errors: `thiserror` in libs, `color_eyre` in bins.
- No `unwrap()` / `expect()` outside tests and `const` contexts.
- Comments only when the "why" isn't obvious — never restate the code.
- No GUI toolkit types (`egui`, `iced`, `slint`, `libcosmic`) in
  `flintshot-core`'s public API. A test enforces this.

Full style detail: `.opencode/tech/rust/coding-style.md`,
`.opencode/tech/rust/patterns.md`, `.opencode/tech/rust/testing.md`.

## Tests

- **Unit tests** in-module (`#[cfg(test)] mod tests`) for tight logic
  loops (geometry, hit-testing, undo/redo, serialization roundtrips).
- **Integration tests** in `crates/<crate>/tests/` for cross-module
  behavior. See `crates/flintshot-core/tests/no_gui_types_in_core.rs`
  for the pattern.
- **Snapshot-style** tests via a `MockCanvas` (planned in Phase 1 per
  `docs/ARCHITECTURE.md`); assert the drawing command sequence for a
  known `EditorState`, run without a GPU.

Tests that need a display should be marked `#[ignore]` and documented
in `docs/TEST_MATRIX.md` so CI doesn't fail and humans know to run them
manually.

## Submitting a change

1. Open an issue first for anything beyond a typo or trivial fix — it
   keeps design discussion out of the PR diff.
2. Branch from `main`; keep one logical change per PR.
3. Write commit messages in the conventional-imperative voice
   (`Add ellipse annotation tool`, not `added ellipse`).
4. Update `CHANGELOG.md` under `## [Unreleased]` for user-visible
   changes (new subcommands, changed config schema, behavior shifts).
5. Add an ADR in `docs/DECISIONS.md` if you introduced or revised a
   non-obvious technical choice. Format: **Context · Decision ·
   Consequences**.
6. Make sure all four daily-workflow commands are green.
7. PR description should reference the issue and summarize the change,
   the verification you did, and any manual testing.

Small, well-scoped PRs review faster than big rewrites.

## License

By contributing, you agree your contributions are licensed under
`MIT OR Apache-2.0`, dual-licensed the same as the rest of the project.
No CLA.

## Conduct

Be kind, be specific, be wrong out loud. We're building a screenshot
tool, not defending a kingdom.
