# ADR-0001: Pure-Rust stack (eframe/egui) over Tauri

**Context.** The project's initial `tech.md` declared
`rust + typescript + react`, which pointed at Tauri. Tauri is mature and
battle-tested. However:

- The user wants a Flameshot-like tool with real-time annotation drawing
  over a captured screen.
- A live overlay with low-latency pen/mouse input on a transparent
  fullscreen surface is fundamentally awkward in HTML5 canvas — feasible
  but not native.
- The user explicitly chose to drop TS/React and update `tech.md` to
  pure Rust.

**Decision.** Use `eframe` (egui + winit + glow) for the editor and
overlay UI. No web frontend, no JavaScript toolchain.

**Consequences.**
- Single-language codebase, easier contribution
- Immediate-mode GUI is a natural fit for drawing tools and per-frame
  overlays
- Smaller binary size (no webview runtime)
- Loses the React/TS ecosystem (rich text editing components,
  accessibility widgets)
- egui's text rendering quality requires `cosmic-text` to be wired in
- egui's look-and-feel is distinctive; users wanting a native GTK/Qt
  look will notice
