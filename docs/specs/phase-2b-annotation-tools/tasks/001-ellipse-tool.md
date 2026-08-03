# Task 001: Ellipse tool end-to-end (egui + ImageCanvas `draw_ellipse` impls)

**Description:** Complete the Ellipse vertical slice. The core is
already done — `render_ann` dispatches `Annotation::Ellipse` to
`canvas.draw_ellipse` (`render.rs:251`), `begin_annotation` /
`update_annotation` handle the drag (`editor.rs:337` / `:407`),
`contains` hit-tests the bounding rect (`editor.rs:1183`), and
`MockCanvas::draw_ellipse` records the call (`render.rs:1293`). The
`finish_pointer_up` degenerate-guard already covers Ellipse
(`editor.rs:938`). The two gaps are both **rasterizer backends**:
`EguiCanvas::draw_ellipse` is a no-op (`canvas.rs:208`) and
`ImageCanvas::draw_ellipse` is a no-op (`render.rs:743`) — the latter
silently drops ellipses from saved/copied PNGs. Add a real outline +
fill impl to both, expose an optional fill (mirroring Rect), and add
the toolbar entry. This unblocks Task 004 (Number's circular bubble
reuses `draw_ellipse`).

**Acceptance criteria:**
- [ ] `EguiCanvas::draw_ellipse` (canvas.rs) renders the outline via
      `egui::Painter` and, when `fill` is `Some(color)`, fills the
      ellipse first. Non-empty primitive sequence (no longer a no-op).
- [ ] `ImageCanvas::draw_ellipse` (render.rs) rasterizes outline + fill
      into the `image::RgbaImage` so `compose_final` includes ellipses.
      Use the same aa-free coverage approach the existing
      `ImageCanvas::draw_rect_*` methods use (parity with the save
      path's visual style).
- [ ] A filled `Annotation::Ellipse` round-trips through
      `compose_final` and colors pixels inside its inscribed ellipse
      with the fill color (pixel-level assertion in a new test).
- [ ] Toolbar exposes an "Ellipse" entry (`app.rs`, alongside Rect);
      selecting it, dragging, and releasing commits an
      `Annotation::Ellipse`; `Ctrl+Z` restores the prior state.
- [ ] Optional fill exposed in the toolbar with the same UX as Rect
      (whatever pattern Rect uses for its `fill: Option<Color>` —
      audit and mirror).
- [ ] WYSIWYG holds: the pending ellipse preview during the drag
      matches the committed form (both route through `render()`).
- [ ] Existing `render_ann_dispatches_each_variant` MockCanvas test
      still passes (Ellipse arm already covered at `render.rs:1556`).

**Verification:**
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-targets` — incl. new
      `compose_final` ellipse pixel test (core) and the egui
      non-empty-primitive assertion.
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual: `cargo run -- region`, draw an outlined ellipse and a
      filled ellipse, `Ctrl+S` to save, reopen the PNG — both ellipses
      present (verifies the `ImageCanvas` fix). Confirm `Ctrl+Z` undoes.

**Dependencies:** None (foundation task).

**Files likely touched:**
- `crates/flintshot-editor-egui/src/canvas.rs` — real `draw_ellipse` impl (replace no-op at `:208`); update the stale module doc at `:23-25` (it claims `draw_text` is still a stub — false — and omits that `draw_ellipse` is now real).
- `crates/flintshot-core/src/render.rs` — real `ImageCanvas::draw_ellipse` impl (replace no-op at `:743`); new `compose_final` ellipse pixel test.
- `crates/flintshot-editor-egui/src/app.rs` — toolbar entry + optional-fill control (mirror Rect's fill UX).

**Docs:**
- The stale module doc comment at `canvas.rs:23-25` is a code home — fix it inline (remove the "stubs until Tasks 12-13" line; describe what's actually implemented). No `docs/gotchas.md` entry needed.
- If the ellipse fill algorithm has a non-obvious gotcha (e.g. coverage threshold, parity quirk), a `// GOTCHA:` comment at the impl site.

**Estimated scope:** Medium (3 files; the two rasterizer impls are the substantive work, toolbar entry is small).
