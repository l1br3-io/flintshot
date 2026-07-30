# Task 007 — System tray menu

> Phase 2.D · size: M · depends on: 003, 006

## Description

Replace the Task 003 placeholder tray with a real `tray-icon` menu:
*Capture region*, *Capture fullscreen*, *Open config*, *Quit*. Each
item routes through the same handler the IPC path uses (Task 006's
dispatch) — region/fullscreen spawn a worker, *Open config* launches
the user's editor on `config.toml` (or prints the path if no
`$EDITOR`), *Quit* sends the shutdown signal. Embed an icon asset so
the tray isn't blank on platforms without a default.

## Acceptance criteria

- [ ] Tray menu has: *Capture region*, *Capture fullscreen*, *Open
      config*, *Quit* — in that order, with separators as appropriate.
- [ ] *Capture region* / *Capture fullscreen* spawn the worker child
      (same path as IPC) and emit `CaptureStarted`/`EditorOpened`.
- [ ] *Open config* opens `config.toml` in `$EDITOR` (fallback
      `$VISUAL`, then print the path if neither is set).
- [ ] *Quit* triggers the same graceful shutdown as `Request::Quit`.
- [ ] Tray creation failure is non-fatal: the daemon logs a warning and
      keeps running (IPC + hotkeys still usable) — important for
      compositors without an SNI host.
- [ ] Embedded icon renders (no blank/default icon on Linux + macOS).

## Verification

- [ ] Standing gate passes.
- [ ] Manual (TEST_MATRIX): `cargo run -- daemon`; each menu item does
      its action; *Quit* exits cleanly; tray icon is visible.
- [ ] Manual: kill the tray host mid-run (or run where no SNI exists);
      confirm daemon stays alive with a logged warning.

## Dependencies

Task 3 (tray + loop), Task 6 (dispatch path to route through).

## Files likely touched

- `crates/flintshot-app/src/tray.rs` (new)
- `crates/flintshot-app/src/daemon.rs` (route menu events to dispatch)
- `crates/flintshot-app/src/main.rs` (wire module)
- `crates/flintshot-app/assets/` (icon asset, embedded via
  `include_bytes`)

## Size

M
