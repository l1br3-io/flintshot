# ADR-0012: Abstract UI toolkit via `Canvas` + `EditorShell` traits

**Context.** The default UI toolkit is egui (per ADR-0001). But egui is
not the only option — iced (used by libcosmic), slint, and gtk-rs all
exist. Without an abstraction, swapping egui for another toolkit would
require rewriting every annotation tool, the editor state model, and
the pin window. Worse, the data model would leak egui types (`Pos2`,
`Color32`, `Rect`, `Stroke`) into non-UI code, coupling capture/IPC
logic to a particular GUI library.

We considered:

1. No abstraction — egui types in core, accept the coupling (rejected —
   user explicitly asked for swappability; locks us in)
2. Higher-level "feature" trait (`EditorUI::run(state)`) — too coarse,
   doesn't really abstract anything (rejected)
3. Full immediate-mode canvas trait — re-invents a 2D drawing API,
   leaky (rejected as primary; partially adopted for annotations only)
4. Two-trait bridge: own types + logic in core; `Canvas` for drawing
   primitives; `EditorShell` for window lifecycle (chosen)

**Decision.** Define geometry types (`Point`, `Rect`, `Color`,
`Stroke`, `Key`) in `flintshot-core::geom`. All shared data model types
(`Annotation`, `EditorState`) use these, never egui's. Define two
traits in core:

- `Canvas` — drawing primitives for annotations; has an associated
  `Image` type for toolkit-specific texture handles
- `EditorShell` — lifecycle ("open editor with this state, return
  outcome")

`flintshot-editor-egui` implements both. `flintshot-app` picks the
implementation via Cargo feature flag (`default = ["egui"]`). Blur and
pixelate are computed in core and emitted to the canvas as image-blit
instructions, so all toolkits produce identical output.

**Consequences.**
- Swapping egui for iced/slint is purely additive: implement two
  traits, flip a feature flag
- Annotation logic (hit-testing, undo/redo, blur) lives in one place —
  bug fixes propagate to all toolkits
- Geometry types are minimal — no `RotatedRect`, `Affine`, `Path`
  until needed; grow as features demand
- The associated `Image` type on `Canvas` makes the trait
  non-object-safe (no `dyn Canvas`); acceptable, we always know the
  concrete type at the call site
- Some toolkit-specific features (egui's per-frame texture caching)
  need care to map cleanly; mitigated by making `upload_image`
  explicit
- Text rendering stays toolkit-specific (each toolkit uses its own
  text shaping); the `Canvas::draw_text` trait method takes plain
  text + position + size + color
