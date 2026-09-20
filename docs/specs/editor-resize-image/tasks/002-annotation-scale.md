# Task 002: `Annotation::scale` — per-variant transform

**Description:** Add a `pub fn scale(&mut self, factor: f32)` to
`Annotation` in `crates/flintshot-core/src/editor.rs`, beside
`translate` / `crop_and_translate` (editor.rs:2166-2259). Multiplies
every geometry field and every px magnitude by `factor` so the
rendered composition is unchanged at the new size. Rejects (no-op
with a documented contract) non-finite or ≤ 0 factors.

**Field map (per variant):**

- `Rect`/`Ellipse`/`Blur`: `rect` min+max × f; `Blur.strength` (u8)
  scales rounded, clamped to ≥ 1
- `Line`/`Arrow`: `a`, `b` × f; `Arrow.head_size` × f
- `Marker`/`Pencil`: every point × f; `width` × f
- `Text`: `pos` × f; `font_size` × f; `TextOutline::width` × f when
  present; `line_spacing` is a dimensionless multiplier — untouched
- `Number`: `pos` × f; `diameter` × f (`n` is content, untouched)

**Acceptance criteria:**

- [ ] `scale` exists, doc-commented, handles all 9 variants
- [ ] Every px magnitude above scales exactly by `factor`
- [ ] Non-finite or ≤ 0 factor is a documented no-op (assert-free —
      matches how `crop` guards degenerates)
- [ ] Unit tests: each variant constructed with known values, scaled
      by 0.5, every field asserted; factor 1.0 is identity; blur
      strength 3 @ 0.5 → 2, strength 1 @ 0.25 → stays 1; Text with
      outline scales `font_size` + `outline.width`, leaves
      `line_spacing`

**Verification:**

- [ ] `cargo test -p flintshot-core annotation::scale` (or module
      test run)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo fmt --all --check`

**Dependencies:** None (parallelizable with Task 001)

**Files likely touched:**

- `crates/flintshot-core/src/editor.rs` (method + tests in the
  existing `#[cfg(test)]` module)

**Docs:**

- None beyond the doc comment.

**Estimated scope:** S — 1 file
