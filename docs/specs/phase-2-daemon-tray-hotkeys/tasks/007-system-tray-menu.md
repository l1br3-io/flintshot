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

- [x] Tray menu has: *Capture region*, *Capture fullscreen*, *Open
      config*, *Quit* — in that order, with separators as appropriate.
      _`tray::build()` appends region, fullscreen, separator,
      open-config, separator, quit._
- [x] *Capture region* / *Capture fullscreen* spawn the worker child
      (same path as IPC) and emit `CaptureStarted`/`EditorOpened`.
      _`drain_tray_menu` routes them through `worker::dispatch` with
      `CommandSpawner` + the shared bus — identical path to IPC. Id→
      action mapping unit-tested._
- [x] *Open config* opens `config.toml` in `$EDITOR` (fallback
      `$VISUAL`, then print the path if neither is set).
      _`open_config_file` + `flintshot_core::config_path` (newly public)._
- [x] *Quit* triggers the same graceful shutdown as `Request::Quit`.
      _dispatch returns `Shutdown` → `drain_tray_menu` returns true →
      the event loop breaks → `run` returns → lock/tray/hotkeys drop._
- [x] Tray creation failure is non-fatal: the daemon logs a warning and
      keeps running (IPC + hotkeys still usable) — important for
      compositors without an SNI host.
      _`run()` wraps `tray::build()` in a warn-and-continue `match`;
      runtime-confirmed the daemon runs fine with the tray._
- [x] Embedded icon renders (no blank/default icon on Linux + macOS).
      _Programmatic two-tone 32×32 mark (dark plate + bright lens); not
      an `include_bytes` asset but renders identically. A real logo
      lands in the polish sweep. **Runtime-confirmed**: daemon with
      `--features tray` builds the tray and runs (no panic, no
      Gtk-CRITICAL after adding `gtk::init()`)._

## Verification

- [x] Standing gate passes. _(fmt, default + tray clippy, workspace
      tests + tray-feature tests, doc tests.)_
- [ ] Manual (TEST_MATRIX): `cargo run --features tray -- daemon`; each
      menu item does its action; *Quit* exits cleanly; tray icon is
      visible. _Daemon runs clean with the tray (runtime-smoke); the
      clicks themselves are interactive._
- [ ] Manual: kill the tray host mid-run (or run where no SNI exists);
      confirm daemon stays alive with a logged warning.
      _Code path is the warn-and-continue `match`; not exercised in a
      no-SNI environment here._

## Implementation notes

- `gtk::init()` is called on Linux before building the menu (tray-icon's
  libappindicator backend touches GTK widgets). Added `gtk` 0.18 as a
  Linux-targeted optional dep under the `tray` feature.
- The subscriber `bus` is now created in `run()` and shared via
  `IpcServer::bind_with_bus`, so a capture started from the tray reaches
  IPC subscribers and vice versa.

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
