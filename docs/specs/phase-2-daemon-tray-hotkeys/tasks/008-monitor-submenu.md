# Task 008 — Multi-monitor tray submenu

> Phase 2.D · size: S · depends on: 007

## Description

Add a *Capture fullscreen →* submenu to the tray that lists each
monitor (via `xcap::Monitor::all()`), each entry spawning
`flintshot fullscreen --monitor N`. This is the multi-monitor UX Phase
2 ships (per spec Open Question 1 — combined/stitched capture and the
interactive picker overlay are deferred). Reuses Phase 1's `--monitor`
path under the hood.

## Acceptance criteria

- [ ] Tray menu gains a *Capture fullscreen →* submenu listing every
      monitor `xcap` reports, labelled by index and (where available)
      name, e.g. "0 · DP-1 (primary)".
- [ ] Selecting a monitor spawns `fullscreen --monitor N` via the
      worker path and emits `CaptureStarted`/`EditorOpened`.
- [ ] Monitor list is refreshed each time the submenu is opened (hot-
      plug a monitor → it appears without restarting the daemon).
- [ ] Degenerate case (zero monitors, or `Monitor::all()` errors):
      submenu shows a disabled "no monitors" entry; no panic.

## Verification

- [ ] Standing gate passes.
- [ ] Manual (TEST_MATRIX, multi-monitor rig if available): submenu
      lists all monitors; each captures the correct one.
- [ ] Manual (single monitor): submenu lists exactly one entry; it
      captures the primary monitor.

## Dependencies

Task 7 (tray menu + dispatch path).

## Files likely touched

- `crates/flintshot-app/src/tray.rs` (submenu + monitor enumeration)

## Size

S
