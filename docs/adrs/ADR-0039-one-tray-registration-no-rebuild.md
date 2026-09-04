# ADR-0039: One long-lived tray registration — no drop/rebuild cycle

**Context.** ADR-0038 made the capture-time tray hide an
unregister/re-register cycle on Linux: `DaemonTray::hide` dropped the
`TrayIcon` (believing the drop unregisters the SNI item) and
`DaemonTray::rebuild` called `tray::build` again to restore it.
Reported live afterwards: **the tray menu duplicates** — after each
capture session the panel grows another flintshot icon + menu, and
they only all clear when the daemon exits. Source reading confirms
the mechanism end to end:

- `tray-icon` 0.24's GTK backend `Drop` only calls
  `app_indicator_set_status(Passive)` and deletes the icon PNG; it
  never unrefs the `AppIndicator` GObject (the `libappindicator`
  Rust binding has no `Drop`), so dispose — the only code that
  unregisters the object path — never runs. The drop *leaks* a
  fully registered item.
- Even a properly disposed indicator would not remove the panel
  entry: SNI has **no** `UnregisterStatusNotifierItem` method.
  `cosmic-applet-status-area`'s watcher keys items by
  `(sender unique name, sender+path)` and removes one only when the
  sender's **unique bus name loses its owner** — i.e. process exit.
  libappindicator registers over `g_bus_get`'s process-lifetime
  singleton connection, so that unique name never drops while the
  daemon lives.
- `TrayIconBuilder::new()` mints a fresh `{pid}-{counter}` id per
  build, which becomes the SNI object path. Every rebuild therefore
  registers a **new** item — and the old one can never be removed.
  One capture cycle = one permanently duplicated menu.

**Decision.** The daemon owns exactly **one** `TrayIcon` for its
whole life, built once at startup behind the ADR-0032 watcher
barrier, under a stable id (`with_id("flintshot")`, so any future
same-path registration replaces rather than accumulates).
`TrayGate` actions apply as `set_visible` flips (`Active` ↔
`Passive`) with the ADR-0031 GLib pump — the ADR-0036/0037 timing
machinery (handshake, graces, safety restore) is unchanged.
`DaemonTray` loses its `rebuild` path entirely.

**Consequences.**

- The duplication bug is gone: one registration, one icon, one
  menu; panels clean it up on daemon exit (unique-name loss) as
  they always did.
- The ADR-0038 ambition (close the panel's menu popup for
  tray-launched captures) is **not** achievable daemon-side on
  COSMIC and is accepted as a known limitation: the applet never
  closes a popup on activation (upstream
  `cosmic-applets` `TODO: Close menu?`), and there is no
  in-process unregistration for any client. Conforming panels that
  honor `Passive` still hide the icon (the ADR-0036 goal); the
  ADR-0037 menu grace still waits out popup close animations on
  panels that do close them.
- Monitor hot-plug no longer refreshes the *Fullscreen on monitor*
  submenu mid-session (a rebuild would duplicate the item); the
  list refreshes on daemon restart, as pre-ADR-0038.
- Supersedes the *mechanism* of ADR-0038 (its timing analysis
  stands); ADR-0036/0037 semantics are restored on all platforms.
