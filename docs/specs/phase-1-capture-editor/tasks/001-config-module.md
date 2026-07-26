# Task 001 — Config module in `flintshot-core::config`

> Phase 1.A · size: S · depends on: none

## Description

Add a `config` module owning the TOML schema from `ARCHITECTURE.md §Config`
(only the fields Phase 1 touches: `[ui]` color/stroke_width/font_size,
`[capture]` include_cursor/default_delay_ms). Load via `dirs` + `toml`,
write via `toml_edit` (preserve comments / formatting). All types
`#[non_exhaustive]` with `#[serde(default)]` so missing fields don't break
load. New dep: `dirs`, `toml`, `toml_edit`, `serde` (already in).

## Acceptance criteria

- [x] `Config::load()` reads `~/.config/flintshot/config.toml` (or
      platform equivalent via `dirs::config_dir()`); returns
      `Ok(Config::default())` when the file is missing.
- [x] `Config::save()` writes the config back without clobbering
      comments (verified roundtrip on a fixture file).
- [x] All public types have doc comments (`#![warn(missing_docs)]`).
- [x] Unknown fields in the TOML are ignored, not errors.
- [x] `Color` serializes as `"#rrggbb"` (or `"#rrggbbaa"`) hex string,
      not four integers. Add a `serde` helper module.

## Verification

- [x] Standing gate passes.
- [x] `cargo test -p flintshot-core config::` — unit tests cover:
      missing-file defaults, roundtrip with comment preservation,
      hex-color roundtrip, unknown-field tolerance.
- [x] Manual: `cargo run -- config` (new CLI verb, see Task 3 wiring)
      prints the loaded config; or temporarily add a `--dump-config`
      debug flag. _(Verified via the throwaway `--dump-config` flag —
      see Task 3 for the real `config` subcommand.)_

## Dependencies

None.

## Files likely touched

- `crates/flintshot-core/src/config.rs` (new)
- `crates/flintshot-core/src/lib.rs` (re-export)
- `crates/flintshot-core/Cargo.toml` (deps: `dirs`, `toml`, `toml_edit`)

## Size

S
