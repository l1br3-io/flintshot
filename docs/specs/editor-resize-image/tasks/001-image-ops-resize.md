# Task 001: `image_ops::resize` — clamped Triangle downscale

**Description:** Add the pure pixel-math primitive to
`crates/flintshot-core/src/image_ops.rs`: scale an `RgbaImage` to a
target size using `image::imageops::resize` with
`FilterType::Triangle`, clamping each target axis to its source
dimension (never upscale). This is the foundation `EditorState::resize`
calls; it is toolkit-free and deterministic.

**Acceptance criteria:**

- [ ] `pub fn resize(source: &RgbaImage, target: (u32, u32)) -> RgbaImage`
      exists, is `#[must_use]`, doc-commented per repo style
- [ ] Each target axis clamps to `[1, source_axis]` — no input can
      produce a larger image
- [ ] Clamped target == source dims returns a clone (no resample)
- [ ] Triangle filter used for the resample
- [ ] Unit tests: downscale dims correct; upscale target clamps to
      identity; zero/degenerate target axes clamp to 1; odd dims
      (e.g. 101×51 → 50×25) exact; same input ⇒ same output

**Verification:**

- [ ] `cargo test -p flintshot-core image_ops`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo fmt --all --check`

**Dependencies:** None (parallelizable with Task 002)

**Files likely touched:**

- `crates/flintshot-core/src/image_ops.rs` (fn + `#[cfg(test)]` tests)

**Docs:**

- Module doc line stays accurate ("blur, pixelate, crop, resize").

**Estimated scope:** XS — 1 file
