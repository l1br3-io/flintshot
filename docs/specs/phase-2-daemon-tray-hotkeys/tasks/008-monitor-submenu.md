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

- [x] Tray menu gains a *Capture fullscreen →* submenu listing every
      monitor `xcap` reports, labelled by index and (where available)
      name, e.g. "0 · DP-1 (primary)".
      _"Fullscreen on monitor →" submenu; each entry
      `"<N> · <name> (primary)"`. Runtime-confirmed: no "no monitors"
      entry on this box, i.e. the monitors were enumerated._
- [x] Selecting a monitor spawns `fullscreen --monitor N` via the
      worker path and emits `CaptureStarted`/`EditorOpened`.
      _`flintshot.fullscreen-monitor.<N>` →
      `Request::CaptureFullscreen { monitor: Some(N) }` → dispatch.
      Unit-tested (`monitor_submenu_item_maps_to_fullscreen_with_index`)._
- [~] Monitor list is refreshed each time the submenu is opened (hot-
      plug a monitor → it appears without restarting the daemon).
      _**Known limitation**: muda/tray-icon don't portably expose a
      submenu-open event, so the list is built at tray creation.
      Hot-plug needs a tray rebuild (future work); restarting the daemon
      refreshes it. Recorded here rather than hidden._
- [x] Degenerate case (zero monitors, or `Monitor::all()` errors):
      submenu shows a disabled "no monitors" entry; no panic.
      _`append_monitor_entries` falls back to a disabled
      `flintshot.no-monitors` item + a logged warning._

## Verification

- [x] Standing gate passes. _(fmt, default + tray clippy, workspace +
      tray tests, doc tests.)_
- [ ] Manual (TEST_MATRIX, multi-monitor rig if available): submenu
      lists all monitors; each captures the correct one. _Submenu builds
      with real monitors (runtime-smoke); clicks are interactive._
- [ ] Manual (single monitor): submenu lists exactly one entry; it
      captures the primary monitor. _Single-monitor build path is the
      same code; interactive click is the matrix row._

## Files touched

- `crates/flintshot-core/src/capture.rs` — `monitors()` + `MonitorInfo`
  (keeps `xcap` in core; the tray depends on core, not xcap).
- `crates/flintshot-app/src/tray.rs` — submenu + `append_monitor_entries`
  + monitor-id parse in `action_for`.

## Dependencies

Task 7 (tray menu + dispatch path).

## Files likely touched

- `crates/flintshot-app/src/tray.rs` (submenu + monitor enumeration)

## Size

S
