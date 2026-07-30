# Task 009 — Global hotkeys from `[hotkeys]` config

> Phase 2.D · size: M · depends on: 002, 003

## Description

Add a `[hotkeys]` section to `flintshot-core::config` (chord strings →
accelerator tokens) and register global hotkeys on daemon start using
`global-hotkey`. Hotkey fires route through the same dispatch handler
as tray clicks and IPC requests (spawn worker). Default chords come
from ARCHITECTURE.md §Config (`region = "Ctrl+Shift+Print"`,
`fullscreen = "Ctrl+Alt+Print"`). This is X11/macOS/Windows only;
Wayland's portal-based hotkeys are Phase 6 — the daemon must detect
the unsupported platform and log a clear "configure your compositor
keybind to run `flintshot region`" notice instead of failing.

## Acceptance criteria

- [ ] `flintshot-core::config::HotkeyConfig` with `region` and
      `fullscreen` chord fields, `#[serde(default)]`, missing section
      → defaults; `Config::save`/`merge_into_existing` handle the new
      table (comment-preserving, like `[ui]`/`[capture]`).
- [ ] `flintshot-app::hotkeys` parses chord strings →
      `global_hotkey::HotKey`; registration failures (bad chord,
      already-bound) are logged and skipped, not fatal.
- [ ] `region` hotkey → dispatch `CaptureRegion`; `fullscreen` hotkey →
      dispatch `CaptureFullscreen`; both emit the normal `Event` stream.
- [ ] On Wayland (or wherever `global-hotkey` reports unsupported), the
      daemon logs the compositor-keybind workaround and continues; tray
      + IPC still work.
- [ ] `cargo run -- config` prints the `[hotkeys]` section.

## Verification

- [ ] Standing gate passes.
- [ ] `cargo test -p flintshot-core config::hotkeys` — parse known
      chords; missing section defaults; unknown chord surfaces a clear
      error.
- [ ] Manual (TEST_MATRIX): on X11 (and macOS/Windows if available),
      press the configured chord → editor opens; change the chord in
      config, restart, new chord works.
- [ ] Manual (Wayland): daemon logs the workaround notice; hotkey does
      not fire; tray + capture still work.

## Dependencies

Task 2 (global-hotkey dep), Task 3 (loop + handler to route through).
(Task 6's dispatch ideally, but a handler shim from Task 3 suffices;
full event broadcast finalises once 006 lands — order 009 after 006 if
building sequentially.)

## Files likely touched

- `crates/flintshot-core/src/config.rs` (`HotkeyConfig` + merge)
- `crates/flintshot-app/src/hotkeys.rs` (new)
- `crates/flintshot-app/src/daemon.rs` (register on start, route fires)
- `crates/flintshot-app/src/main.rs` (wire module)

## Size

M
