# Task 002: Line tool wiring (toolbar entry + degenerate-pointer-up guard)

**Description:** The Line tool's core is fully implemented —
`begin_annotation` constructs `Annotation::Line` (`editor.rs:342`),
`update_annotation` tracks the head endpoint (`editor.rs:409`),
`render_ann` dispatches to `canvas.draw_line` (`render.rs:254`),
`contains` hit-tests via point-to-segment distance with stroke-width +
tolerance (`editor.rs:1187`), and both egui `draw_line`
(`canvas.rs:210`) and `ImageCanvas::draw_line` (`render.rs:750`) are
real impls. Two small gaps remain: (a) the toolbar has no Line entry
(`app.rs:557-561`), and (b) `finish_pointer_up`'s degenerate-cancel
guard covers Rect/Ellipse/Blur (zero-area rect) and Arrow (short
length) but **not Line** (`editor.rs:944-949`) — a click-without-drag
on Line would commit an invisible zero-length line.

**Acceptance criteria:**
- [ ] Toolbar exposes a "Line" entry; selecting it, dragging, and
      releasing commits an `Annotation::Line`; `Ctrl+Z` restores.
- [ ] Hit-test selects a committed line by clicking near it (reuses
      existing `distance_to_segment` tolerance at `editor.rs:1188`).
- [ ] `finish_pointer_up` treats a sub-1px-length `Annotation::Line`
      as degenerate and cancels (no commit) — mirrors the existing
      Arrow guard at `editor.rs:944-948`.
- [ ] Unit test: `Annotation::Line` with `a == b` (or sub-px apart)
      passed through `finish_pointer_up` leaves `state.anns` unchanged
      and `state.pending == None`.
- [ ] MockCanvas test: a committed `Annotation::Line` emits exactly
      one `DrawLine` call (template: `render_pending_marker_emits_segment_lines`
      at `render.rs:1851`).

**Verification:**
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-targets` — incl. new degenerate-
      cancel unit test (core editor) and the single-`DrawLine`
      MockCanvas test.
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual: `cargo run -- region`, draw a line, confirm it commits;
      click near it to select; click-without-drag produces nothing.

**Dependencies:** None (independent of Task 001; ordered here for
coherent toolbar grouping and the Phase 2b.B checkpoint).

**Files likely touched:**
- `crates/flintshot-editor-egui/src/app.rs` — toolbar entry.
- `crates/flintshot-core/src/editor.rs` — extend `finish_pointer_up`
  degenerate match at `:944-949` to cover `Annotation::Line` (group it
  with the existing `Annotation::Arrow` length check).
- `crates/flintshot-core/src/render.rs` — MockCanvas `DrawLine` test
  (and verify `render_ann_dispatches_each_variant` at `:1518` already
  covers Line; it does at `:1527`).

**Docs:**
- Inline comment at the extended degenerate guard explaining Line now
  mirrors Arrow (the "why" — both are two-point tools; a click without
  a drag is degenerate). No external doc.

**Estimated scope:** Small (2 files; ~20 LOC of logic + tests).
