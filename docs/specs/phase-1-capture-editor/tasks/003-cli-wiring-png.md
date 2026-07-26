# Task 003 — CLI wiring: `region` / `fullscreen` produce a PNG

> Phase 1.A · size: S · depends on: 001, 002

## Description

Replace the `run_region` and `run_fullscreen` stubs with real capture
calls. Add a `--out PATH` flag (default
`/tmp/flintshot-<timestamp>.png`) for one-shot save. No editor yet —
that's Task 6. Goal: prove the capture stack end-to-end against a real
display. Add a `config` subcommand that prints the loaded TOML.

## Acceptance criteria

- [x] `flintshot region --out /tmp/foo.png` writes a real PNG of the
      primary monitor (or the selected region, if Phase 1.F's crop tool
      is being demoed early — out of scope here, just do fullscreen-crop
      to a sane default).
- [x] `flintshot fullscreen --monitor 0 --out /tmp/foo.png` writes the
      PNG of monitor 0.
- [x] `flintshot config` prints the loaded TOML to stdout.
- [x] Failure paths (no display, write permission denied) return
      `color_eyre` reports with context, not raw panics.
- [x] Default `--out` filename includes an ISO-8601 timestamp.

## Verification

- [x] Standing gate passes.
- [x] Manual: `cargo run -- region --out /tmp/x.png && file /tmp/x.png`
      reports `PNG image data, ...`.
- [x] Manual: `cargo run -- fullscreen --monitor 99` fails cleanly
      with a readable error.
- [x] Manual: `cargo run -- config` prints valid TOML.

## Dependencies

Task 1, Task 2.

## Files likely touched

- `crates/flintshot-app/src/cli.rs` (real bodies; new `config` verb;
  `--out` flag)
- `crates/flintshot-app/Cargo.toml` (no new deps unless `time`/`chrono`
  for timestamps — prefer `std::time` + formatting helper)

## Size

S
