# ADR-0049: Remove pin-to-desktop

**Context.** Pin-to-desktop shipped in Phase 3: `flintshot pin
<path>`, the editor/overlay Pin button, tray and COSMIC-applet *Pin
last* (`Request::PinLast`), pin worker sessions with a pre-grab
kill/respawn handshake, and a `[pin]` config section. The promise was
a captured image floating always-on-top, draggable, resizable,
aspect-locked.

It never worked as intended on Wayland. There is no always-on-top
protocol for ordinary top-level windows outside layer-shell, so most
compositors ignore the winit hint — the pin opens and drags fine but
does not float. X11, macOS, and Windows honour the hint, but the
product's bar is cross-platform parity, and Wayland is the primary
Linux target (portal captures, COSMIC desktop). A real fix means a
layer-shell-based pin window: a significant platform-specific
investment (per-compositor testing, a second windowing path beside
eframe, its own input/opacity/drag semantics) for a feature that is
nice-to-have, not core. Meanwhile the feature carries ongoing costs:
a 372-line `pin` module, a third trait method on `EditorShell`, pin
session tracking and respawn argvs in the daemon's pre-grab
handshake, a monotonic pin-id counter, and IPC wire variants.

**Decision.** Remove pin-to-desktop from the product entirely — not
deferred behind a feature flag, not stubbed. Dropped from the CLI
surface, the IPC protocol (`Request::PinLast`, `Event::Pinned`), the
editor/overlay toolbars, the tray/applet menus, the config schema
(`PinConfig`), and the living docs. Dead code is deleted: the pin
window module, `EditorShell::run_pin`, `EditorOutcome::Pinned`,
daemon pin sessions and the pin kill/respawn legs of the capture
gate. Historical records (shipped specs, CHANGELOG history, earlier
ADRs) keep their pin mentions; the ROADMAP Phase 3 row is annotated
"pin later removed, ADR-0049" so the history stays readable.

The postcard wire-format break is accepted: deleting enum variants
re-indexes every variant after them, so old and new binaries cannot
interoperate. Every IPC client except the COSMIC applet is spawned
by the daemon itself (tray helper, capture workers) and is therefore
version-locked; the one skew window — a long-lived applet surviving
a daemon upgrade — degrades to a garbage decode → connection error.
No tombstone variants, no compat shim. Existing config files with a
`[pin]` section are silently ignored (config parsing has no
`deny_unknown_fields`); no migration.

**Consequences.**

- Users lose `flintshot pin`, the Pin button, tray/applet *Pin
  last*, and `[pin]` config — replaced by nothing. Flameshot-style
  "keep this snippet visible" re-enters only as a layer-shell effort
  if it is ever missed enough to justify the platform work.
- The `EditorShell` trait, `Request`/`Event` enums, and daemon
  session machinery all shrink; the pre-grab handshake now covers
  the tray, app window, and capture sessions only (the pin legs were
  the gate's most stateful part: respawn argvs, per-pin geometry).
- The wire break means daemon and COSMIC applet must be upgraded
  together — same package/build, which is how they ship.
- IPC roundtrip tests still exercise the surviving variants, so the
  shrunken wire format stays covered (ADR-0011's append-only rule
  continues for future variants).
- If a pin ever returns, it returns as a layer-shell citizen with
  its own ADR; the deleted code is recoverable from git history.
