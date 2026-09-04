# Task 001 — `DaemonTray` becomes rebuildable (hide = unregister)

**Status:** done (shipped in commit `1b12801`; backfilled)

## What

Restructure the daemon's tray ownership so a capture-time hide on
Linux is an **SNI unregister** and the restore a **full rebuild**:

- `DaemonTray` carries `Option<tray_icon::TrayIcon>` plus the
  session flavor (`wayland: bool`) instead of a bare handle.
- `hide()` on Linux drops the handle — the panel destroys its
  still-open menu popup and removes the icon (ADR-0038);
  macOS/Windows keep `set_visible(false)`.
- `restore()` rebuilds via `crate::tray::build` behind the
  ADR-0032 `ensure_watcher` barrier, offering the same menu (the
  Wayland-only *Bind shortcuts…* item) — hence the flavor flag
  survives even an initial `tray::build` failure.
- Headless tests and no-tray builds stand in via `DaemonTray::none()`
  (`from_opt` serves tray builds; `#[allow(dead_code)]` carries a
  reason).

## Why

`cosmic-applet-status-area` never closes an SNI menu popup on
activation (upstream `TODO`) and ignores `NewStatus` — no grace
duration or status flip can remove it. Item unregistration is the
only panel-observed lever.

## Verification

Existing daemon-loop / tray-gate tests green (the state machine is
unchanged); manual L5 pass per the retuned TEST_MATRIX rows.
