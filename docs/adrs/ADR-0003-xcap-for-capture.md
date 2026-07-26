# ADR-0003: `xcap` for screen capture

**Context.** Screen capture options in Rust:

- `xcap` — most adopted, supports X11/Wayland/macOS/Windows, maintained
  by the `pot-app` team
- `screenshots` (formerly `screenshots-rs`) — predecessor to `xcap`,
  deprecated in favor of it
- `scrap` — lower-level, more about continuous capture for streaming
- Custom per-platform bindings via `x11rb`/`xcb`, `core-graphics`,
  `DXGI` — maximum control, maximum work

**Decision.** Use `xcap`. Re-evaluate if its ~25ms-per-capture latency
on Wayland becomes a user-visible problem.

**Consequences.**
- Cross-platform capture out of the box
- Active maintenance, broad adoption
- On Wayland, capture goes through `xdg-desktop-portal`, which shows a
  permission dialog on first use
- We're at the mercy of `xcap`'s update cadence for new platform quirks
