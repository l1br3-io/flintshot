# Task 001: Centered digit anchor in the shared render path

**Description:** Fix the digit mis-alignment. In `render.rs`'s
`render_ann`, the `Annotation::Number` arm draws the label with
`draw_text(*pos, …)` — but `pos` is the bubble center and the `Canvas`
contract anchors text top-left, so the digit renders offset toward the
bubble's bottom-right. Measure the label via the existing
`canvas.measure_text` trait method and draw at
`pos − ½·(width, height)` so the digit is centered. The fix lives in
the shared arm only — `EguiCanvas` and `ImageCanvas` both route
committed annotations through `render_ann`, so editor view and
saved/copied output fix together (WYSIWYG contract, ADR-0015 lineage).
No canvas trait or implementation changes; the Text tool's top-left
anchoring is untouched.

**Acceptance criteria:**
- [ ] The `Annotation::Number` arm computes the label's top-left
      anchor as bubble center minus half the measured label size and
      passes that anchor to `draw_text`; bubble disc (ellipse call)
      and digit font size (`diameter * 0.6`) unchanged.
- [ ] MockCanvas render test: a committed one-digit `Number` issues
      `DrawText` at `pos − ½·measure("1")` (mock metrics:
      `width = chars · size · 0.6`, `height = size · line_spacing`).
- [ ] MockCanvas render test: a two-digit label ("12") centers the
      same way at default diameter 28.
- [ ] MockCanvas render test: a non-default diameter (e.g. 64) also
      centers — anchors scale with the measured size, not a constant.
- [ ] No `EguiCanvas` / `ImageCanvas` / `Canvas` trait changes; no
      behavior change in any other tool's render arm.

**Verification:**
- [ ] `cargo test -p flintshot-core --quiet` — new tests pass, no
      regressions in the existing Task-004 Number render tests
      (bubble-disc assertions unchanged; any that pinned the old
      off-center `DrawText` position are updated to the centered
      anchor).
- [ ] `cargo clippy -p flintshot-core --all-targets -- -D warnings`
- [ ] Manual: `cargo run -- editor` → Number tool → place bubbles
      with 1- and 2-digit labels → digits sit centered in the editor;
      Save → the PNG matches the editor.

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-core/src/render.rs` (Number arm + MockCanvas tests)

**Docs:**
- Rationale lives in the arm's existing comment block; extend it with
  the centering note. No ADR — localized fix, no decision worth a
  record beyond the spec.

**Estimated scope:** Small (1 file)
