# ADR-0041: The applet closes its own popup before capture requests

**Context.** Reported live on COSMIC (2026-09-07): the "menu" still
appears in screenshots launched from a panel UI. Two distinct leaks
hide behind that report.

1. **The SNI tray menu** (`cosmic-applet-status-area`'s popup). The
   ADR-0037 menu grace (600 ms, click-anchored) already waits out
   panels that close their popup on activation — but COSMIC never
   does (upstream `TODO: Close menu?`), and no daemon-side lever can
   close another process's popup: SNI has no unregistration method,
   watchers drop items only at process exit, and `tray-icon` 0.24's
   GTK `Drop` merely flips status to `Passive` (verified in the
   vendored source; the ADR-0038 unregister/re-register attempt
   leaked a duplicate item per capture and was reverted by
   ADR-0039). No delay duration fixes this; it stays a known
   limitation.
2. **The `flintshot-cosmic` applet's own popup** — ADR-0037's
   recorded open edge. The applet fired the capture `Request` with
   its popup still open, and the IPC path carries no origin marker,
   so the daemon applied only its 250 ms icon grace: the applet's
   popup reliably landed in the shot. Unlike the SNI popup, this
   surface is *ours* — `destroy_popup` closes it causally.

**Decision.** The applet self-serves the menu grace. On a
capture-request click (`request_spawns_capture`: region, fullscreen,
per-monitor) it (1) destroys its popup at the click via
`destroy_popup`, and (2) waits a 250 ms settle window
(`POPUP_CLOSE_SETTLE`) before sending the `Request`, covering the
unmap round trip + COSMIC's close animation. No IPC schema change:
an origin marker would let the daemon wait instead, but it cannot
anchor earlier than request receipt, so the timing is equivalent
while a wire change would break old-daemon/new-client interop.

Non-capture requests (pin/open/quit) keep the popup open — its live
status line is their feedback; only captures freeze pixels.

**Consequences.**

- Applet-launched captures grab a popup-free frame: the 250 ms
  settle (applet) + spawn/connect + 250 ms icon grace (daemon) put
  click-to-grab at ≈ 600 ms — the same rhythm as the SNI tray path.
- The applet loses its "Capturing…" status line for captures (the
  popup is gone); the editor/overlay opening is the feedback.
  Pin/open/quit keep their status updates.
- Works against existing daemons (no wire change); a daemon that
  someday grows origin-aware graces can subsume the settle, not
  the popup close.
- The SNI tray menu on COSMIC remains in shot (upstream bug;
  ADR-0039's accepted limitation) — the applet is the
  popup-clean capture path there.
- Refines ADR-0037's open edge: the applet half is fixed
  client-side; desktop-action launcher menus (not ours, no popup
  marker) remain uncovered.
