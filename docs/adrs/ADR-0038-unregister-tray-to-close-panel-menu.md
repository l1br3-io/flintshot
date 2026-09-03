# ADR-0038: Unregister the tray item to close the panel's menu

**Context.** Follow-up finding to ADR-0037 (L5/COSMIC, 2026-09-03):
the click-anchored 600 ms menu grace did not remove the tray **menu**
from tray-launched captures — on *every* shot. Source reading of
`cosmic-applet-status-area` (the panel surface that renders SNI
menus) explains why:

- Activating a menu item only forwards the `DBusMenu` `clicked`
  event; closing the popup is an upstream `// TODO: Close menu?`
  (`status_menu.rs`, `Msg::ClickToken`). The popup never closes on
  activation, so **no grace duration can ever fix it**.
- The applet ignores SNI `NewStatus` signals, so the ADR-0036
  `set_visible(false)` (status → `Passive`) flip removes neither the
  open popup nor, on COSMIC, reliably even the icon.
- The applet *does* destroy the open popup when the item whose menu
  is open **unregisters** (`Event::Unregistered` → `destroy_popup`),
  and `tray-icon`'s Linux backend gives us exactly that lever:
  dropping the `TrayIcon` unregisters the SNI item.

**Decision.** On Linux, a capture-time tray "hide" is an
unregister/re-register cycle instead of a status flip. The gate's
`Hide` action drops the daemon's `TrayIcon` (the panel then destroys
the popup and removes the icon — causally, not by blind timing);
`Show` / `ForceShow` rebuild it via `tray::build`, behind the same
watcher barrier startup uses (ADR-0032) so a dead watcher is poked
back before the re-registration attempt rather than the icon going
missing until the watchdog's next sweep. macOS/Windows keep
`set_visible` flips: their native menus close on activation and the
status flip is honored. The ADR-0037 grace machinery is unchanged —
the unregister's *completion* is still unobservable (no API queries
another process's popup state), so the ack keeps waiting out
`MENU_GRACE` anchored at the click, which now covers a causal
destroy instead of a speculative close animation.

**Consequences.**

- Tray-launched captures freeze a frame with neither the icon nor
  the menu on every SNI panel, including COSMIC's never-closing
  popup — the bug this ADR exists for.
- Every daemon-dispatched capture now costs a full SNI
  unregister/re-register round: the icon flickers out at the trigger
  and returns after the grab (the same visible rhythm as before,
  just via a heavier mechanism). Re-registration is the same code
  path as daemon startup and re-enumerates the monitor submenu as a
  side bonus (per-capture hot-plug refresh).
- A rebuild that fails (logged, not retried inline) leaves the tray
  absent until the next capture's hide/restore cycle or a daemon
  restart — the next `Show` retries by construction. Worst case the
  watcher barrier blocks the main loop ~2 s, only on a session whose
  watcher is already dead (same trade as startup).
- Upstream's `TODO: Close menu?` remains the *right* long-term fix;
  when panels close their popups on activation, this cycle could be
  demoted back to a status flip. Until then this is the only
  daemon-side lever.
- Supersedes the mechanism (not the timings) of ADR-0036/0037 on
  Linux: `TrayAction::Hide`/`Show` now mean unregister/re-register
  there.
