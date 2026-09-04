# Plan: fix-tray-menu-popup — unregister the tray to close the panel's menu

## Overview

ADR-0037's 600 ms menu grace could not remove the tray menu popup
from tray-launched captures on COSMIC because the panel never
closes an SNI menu popup on activation (upstream `TODO`) and
ignores `NewStatus` status flips. The only lever that makes the
panel destroy its popup — and reliably drop the icon — is
unregistering the SNI item, which `tray-icon`'s Linux backend
exposes as dropping the `TrayIcon`. The capture-time hide on Linux
therefore becomes an unregister / re-register cycle (ADR-0038):
`DaemonTray` is restructured to carry `Option<TrayIcon>` plus the
session flavor so it can drop and rebuild the whole tray; restore
rebuilds behind the ADR-0032 watcher barrier so a dead watcher is
poked back before the re-registration attempt. macOS/Windows keep
`set_visible` flips; the ADR-0037 grace timings are unchanged (the
destroy's completion is still unobservable). Backfilled at
closeout — the work shipped as commit `1b12801` in three moves
mirrored by the tasks below.

## Architecture decisions

- **`DaemonTray` owns rebuildability, the gate stays dumb** — the
  `TrayGate` state machine (ADR-0036/0037) is untouched in *what*
  it decides; only the daemon's *how* changes (`TrayAction::Hide`
  now means unregister on Linux). No new logic branches to test.
- **The Wayland flag must survive icon loss** — a rebuild has to
  offer the same menu (the Wayland-only *Bind shortcuts…* item), so
  `DaemonTray` is constructed with the session flavor even when
  `tray::build` fails, and headless tests stand in via `none()`.
- **Rebuild goes through `tray::build`, not a cached handle** —
  same code path as daemon startup, which re-enumerates the monitor
  submenu as a side bonus (per-capture hot-plug refresh).
- **Watcher barrier before re-register** (ADR-0032's
  `ensure_watcher`) — without it, a watcher that died mid-capture
  would swallow the re-registration until the watchdog's next
  sweep; the barrier blocks ≤ ~2 s only on an already-dead
  watcher.

## Task index

1. `tasks/001-daemon-tray-rebuildable.md` — `DaemonTray`
   restructure: `Option<TrayIcon>` + session flavor; Linux
   `hide()` drops the handle, `restore()` rebuilds behind the
   watcher barrier; macOS/Windows `set_visible`.
2. `tasks/002-tray-gate-semantics.md` — `TrayAction` / `MENU_GRACE`
   doc retune: hide = unregister on Linux; grace now waits out a
   causal destroy, not a speculative close animation.
3. `tasks/003-docs-adr-0038.md` — ADR-0038 + index, CHANGELOG
   `[Unreleased]` entry, TEST_MATRIX runbook rows retuned.

## Verification

Manual L5 pass per the retuned TEST_MATRIX rows (tray-launch →
frozen frame without icon or menu; every trigger restores the
tray; killed-worker safety bound now a full re-register). Standing
gates re-run at closeout: `fmt`, clippy `--workspace --all-targets
-D warnings`, `cargo test --workspace --all-targets` (507 passed),
doc tests.
