# ADR-0037: Click-anchored menu grace for tray-launched captures

**Context.** Follow-up finding to ADR-0036 (L5, 2026-09-01): the
handshake removed the tray **icon** from frozen captures, but the
tray **menu** — the popup the click dismissed — still appeared in
the shot. The popup is rendered by the panel (libappindicator
exports a DBusMenu; the panel draws it), and its disappearance is a
DBusMenu activation round trip plus the panel's close animation
(a spring on COSMIC, a fade elsewhere) — which outlasts the icon's
250 ms repaint grace. Worse, nothing observable says when the popup
is gone: no API queries another process's popup state, and muda
exposes no menu-closed event.

The trigger matters: only tray clicks involve a menu. Hotkeys, CLI
delegation, and daemon-internal dispatches freeze the screen with no
popup closing, and taxing them with a longer wait would slow the
flagship hotkey flow for nothing.

**Decision.** The pre-hide at tray-menu dispatch
(`PreHideOrigin::TrayMenu`) sets the hide's confirmation grace to
`MENU_GRACE` (600 ms), anchored at the click — the same instant the
icon hide anchors. The handshake's ack therefore leaves only after
the popup's round trip + animation has plausibly settled, and the
worker grabs an icon-free *and* menu-free frame. `PreHideOrigin::Other`
(hotkeys) and step-created hides (IPC / applet / CLI delegate — the
daemon cannot know what UI launched those) keep the 250 ms icon
grace.

**Consequences.**

- Tray-launched captures take ~600–700 ms click-to-grab (up from
  ~300 ms) — the blind wait's price. `MENU_GRACE` is a tunable
  const: too short re-ices the mid-fade popup into the shot; too
  long makes the tray menu feel sluggish.
- If a panel animates longer than 600 ms, the ghost returns — the
  TEST_MATRIX row exists to catch exactly that per platform; the
  fix is a const bump, not surgery.
- The COSMIC applet's own popup and desktop-action launcher menus
  are **out of scope**: those captures arrive over IPC with no
  origin marker, so the daemon cannot target them without a schema
  change (deferred until someone reports it — recorded as this
  ADR's open edge).
- Supersedes nothing in ADR-0036 — this refines one grace constant
  into a per-origin choice.
