# ADR-0015: `compose_final` is a separate render path from live `render()`

**Context.** Save and Copy need the annotations baked into the pixel
buffer. Two options: (a) screenshot the egui canvas, or (b) render
annotations into a clone of `state.image` directly. Option (a) is
toolkit-dependent (couples output to egui), resolution-capped to the
window size, and grabs `pending` plus toolbar chrome. Option (b) is
toolkit-independent and deterministic.

**Decision.** `compose_final(state: &EditorState) -> RgbaImage` lives in
`flintshot-core::render`. It clones `state.image`, walks `state.anns`
through the existing per-variant `render_ann` dispatch into a private
`ImageCanvas` (which implements `Canvas` directly on top of an
`RgbaImage`), and returns the result. It excludes `state.pending` —
in-flight annotations are not part of the saved output. `ImageCanvas`
is private; `compose_final` is the only public entry point. This
function is the source of truth for Save / Copy and for Phase 4 upload
bytes — not a canvas screenshot.

**Consequences.**

- One per-variant mapping (`render_ann`) is shared between live display
  and saved output; a bug fix propagates to both.
- Output is toolkit-independent and deterministic (no fontconfig /
  subpixel variance leaks into saved geometry until Task 12 text
  shaping lands).
- `ImageCanvas` is private; promotable to `pub` later (e.g. a Phase 4
  thumbnail path with different geometry) without a breaking change.
- Each new annotation primitive (Arrow / Text / Blur) must be
  implemented on `ImageCanvas` as well as `EguiCanvas` to be baked into
  saved output. Until then, `compose_final` drops that variant with a
  `tracing::warn!` so a session trace shows what is missing.
