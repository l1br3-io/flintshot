---
status: shipped
started: 2026-09-03
shipped: 2026-09-03
---

# Spec — Fix tray menu popup in every tray-launched capture on COSMIC

> Bug-fix effort, follow-up to `hide-tray-for-capture` task 005
> (ADR-0037). Found on L5 (COSMIC, Wayland, 2026-09-03): the
> click-anchored 600 ms menu grace did **not** remove the tray menu
> popup from tray-launched captures — it landed in *every* shot.
> Source reading of `cosmic-applet-status-area` showed no grace
> duration could ever fix it. Shipped as commit `1b12801`; this spec
> is backfilled at closeout (the fix landed before the spec was
> written — recorded honestly, not prettified).

## Objective

Tray-launched captures on SNI panels freeze a frame containing
**neither the tray icon nor the dismissed menu popup** — causally,
not by hoping a grace outlasts an animation the panel never runs.

## Root cause

- Activating a `DBusMenu` item only forwards the `clicked` event;
  `cosmic-applet-status-area`'s handler carries an upstream
  `// TODO: Close menu?` (`status_menu.rs`, `Msg::ClickToken`) —
  the popup never closes on activation.
- The applet ignores SNI `NewStatus`, so ADR-0036's
  `set_visible(false)` status flip removes neither the open popup
  nor, on COSMIC, reliably even the icon.
- The applet *does* destroy its popup when the item whose menu is
  open unregisters — and dropping `tray_icon`'s `TrayIcon`
  unregisters the SNI item. That is the one daemon-side lever.

## Scope (in / out)

**In:**

- Linux capture-time hide becomes an **unregister / re-register
  cycle** (ADR-0038): `DaemonTray` carries `Option<TrayIcon>` plus
  the session flavor (Wayland flag) so it can be rebuilt offering
  the same menu; `Hide` drops the handle, `Show`/`ForceShow`
  rebuild via `tray::build` behind the ADR-0032 watcher barrier.
- macOS/Windows keep `set_visible` flips (native menus close on
  activation).
- ADR-0037 grace machinery unchanged — the destroy's completion is
  still unobservable, so the ack keeps waiting `MENU_GRACE` anchored
  at the click.
- Docs: ADR-0038, CHANGELOG `[Unreleased]`, TEST_MATRIX runbook
  rows retuned to the unregister semantics.

**Out:**

- Upstream's `TODO: Close menu?` (the right long-term fix; this
  cycle can demote back to a status flip once panels close popups).
- ADR-0037's open edge (applet / desktop-action menus arriving over
  IPC with no origin marker).
- Tray menu contents, hotkey/portal, worker, editor paths.
- Timing changes (`MENU_GRACE` stays 600 ms).

## Exit criteria

1. On L5 (COSMIC), a tray-menu-launched capture's frozen frame
   contains neither flintshot's icon nor its menu popup; every
   other panel item is untouched.
2. The tray returns after the grab on every trigger, including
   error paths and the killed-worker safety bound (now a full
   re-register).
3. Standalone gates green: `fmt`, clippy `--workspace --all-targets
   -D warnings`, workspace tests, doc tests.
4. TEST_MATRIX runbook rows updated to the unregister semantics
   with the commit hash recorded.

## Testing strategy

- Manual (L5): the ADR-0036/0037 runbook rows in TEST_MATRIX —
  retuned in this effort — are the live verification.
- Unit: existing tray-gate / daemon-loop tests pin the state
  machine; no new logic branches were added (the gate's *what*
  is unchanged — only the daemon's *how*).
- Gates re-run at closeout (2026-09-03): 507 tests, 0 failures.

## Decisions

- **Unregister, not status flip** — the panel destroys its popup
  only on item unregistration; `NewStatus` is ignored. Full
  rationale and consequences in ADR-0038.
- **Rebuild behind the watcher barrier** — a re-register on a
  session whose watcher died mid-capture would silently lose the
  icon until the watchdog's next sweep; the ADR-0032 barrier pokes
  the watcher back first (same trade as daemon startup).
- **Session flavor rides in `DaemonTray`** — the rebuilt menu must
  offer the same items (the Wayland-only *Bind shortcuts…* entry),
  so the flag survives even an initial `tray::build` failure.

## Open questions

None — the mechanism is decided by ADR-0038; the upstream `TODO`
is recorded there as the eventual demotion path.
