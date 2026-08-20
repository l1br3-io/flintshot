# Phase 6 — Wayland global hotkeys: task status

Single status source; mirrors `plan.md`. Tick in commit order.

- [x] 001 IPC contract: `Request::BindShortcuts` + `flintshot hotkeys bind` CLI
- [x] 002 Portal session: silent attach + activation dispatch + Wayland selection + fallback
- [x] 003 Bind flow: portal dialog with suggested chords, CLI end-to-end
- [x] 004 Tray item + `ShortcutsChanged` refresh + Wayland hot-reload notice
- [x] 005 Dogfood COSMIC + KDE, TEST_MATRIX, ADR-0029, CHANGELOG
      (happy-path rows 4–8 + real-X11 pass deferred with maintainer
      sign-off — see TEST_MATRIX Phase 6 caveats)
