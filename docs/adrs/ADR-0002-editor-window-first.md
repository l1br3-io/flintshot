# ADR-0002: Editor-window first, live overlay later

**Context.** Flameshot's signature UX is the live overlay — fullscreen
transparent window where you select and annotate over the actual screen.
This is hard cross-platform:

- On Wayland, compositors won't let arbitrary clients grab fullscreen
  input.
- On macOS, it requires the screen-recording permission path plus
  careful window level management.
- On Windows, it requires opaque fullscreen windows with layered
  drawing.

The editor-window model (Satty-style: capture → open normal window
containing the image → annotate → save) works everywhere with no special
privileges.

**Decision.** Ship editor-window as the only interaction model in
Phase 1. Add live overlay in Phase 5 with explicit Wayland fallback.

**Consequences.**
- Phase 1 ships on every platform with no platform-specific surprises
- Same `EditorState` model serves both modes — Phase 5 is mostly a
  different surface, not a different feature set
- Phase 1 UX is less "magical" than Flameshot
- Some users will request live overlay and we have to communicate
  "Phase 5" politely
