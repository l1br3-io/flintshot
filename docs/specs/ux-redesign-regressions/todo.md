# ux-redesign-regressions todo

- [x] Task 1: Home route geometry matches the stable window geometry
      (001-home-stable-geometry) — d739d3c
- [x] Task 2: Topbar glyph kit — painted vector icons
      (002-topbar-glyph-kit) — f780440 (landed wired with Task 3:
      the `-D warnings` dead-code gate rejects a commit carrying
      unconsumed glyph fns)
- [x] Task 3: Topbar consumes painted glyphs + tofu-ban scan
      (003-topbar-painted-icons) — f780440
- [x] Task 4: Copy feedback — the hero's Copied state
      (004-copy-copied-state) — 80f0ccf
- [x] Task 5: Settings-route Capture dispatches — dead control
      (005-settings-capture-dispatch) — f99b80b
- [x] Task 6: Worker Copy no longer closes the window on Linux
      (006-worker-copy-lifetime) — 8ca0a97
- [x] Task 7: Closeout — CHANGELOG, ADR-0059 (painted glyphs) +
      ADR-0060 (worker copy lifetime), full gate green

Remaining before the spec flips to `shipped`: the manual protocol
(spec §Commands) — Home 1024×640 resizable, every icon rendered,
Copy shows "Copied" + `wl-paste --list-types` serves `image/png`,
worker Copy pastes after the window lingers, Settings Capture
dispatches. Maintainer's eyes on a live session.
