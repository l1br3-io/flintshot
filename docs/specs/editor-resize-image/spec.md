---
status: active
started: 2026-09-20
---

# Spec — Editor: Resize Image

Destructive, crop-style resize of the editor canvas: scale the
screenshot and every annotation on it to smaller pixel dimensions,
undoable like any other edit. HiDPI captures ("my 2x grab is huge for
the docs") are the driving use case.

For product context, see [`../../VISION.md`](../../VISION.md); for
the boundary rules that govern this code, see
[`../../../AGENTS.md`](../../../AGENTS.md).

## Objective

A user finishes annotating, realizes the 2x capture is too large for
its destination, clicks **Resize** in the export island, picks 50%
(or a custom width), and the whole composition — image, strokes, font
sizes, everything — scales down uniformly. The saved output is
pixel-identical to what the editor now shows. Ctrl+Z restores the
full-resolution original.

Accepted design stance (from brainstorm + ADR-0015):

- **Destructive, not export-time.** Resize mutates `EditorState`
  exactly like `crop` does. The live view keeps matching saved output.
- **Never upscale.** Targets cap at current dimensions. Presets only
  offer ≤ 100%; the custom field clamps to image width.
- **Uniform scaling.** Annotations scale with the image (geometry,
  stroke widths, font sizes, head sizes, diameters) so the rendered
  composition is unchanged, just smaller.

### User stories

- As a docs writer, I downscale a 3840×2160 HiDPI grab to 1920×1080
  with one click so it embeds sanely.
- As any user, I can enter an exact width (e.g. `800`) and the height
  follows the aspect ratio.
- As any user, Ctrl+Z after resizing gets my original pixels and
  annotations back, exactly.

### Out of scope

- Upscaling (refused, not warned-about)
- Export-time / save-dialog scaling (non-destructive variant)
- Overlay-mode UI (editor window only)
- Config keys, CLI flags, keybinds
- Percentage-agnostic "max dimension" presets (custom width covers it)

## Technologies

No new dependencies. Existing: `image` (`imageops::resize`,
`FilterType::Triangle` — already used for thumbnails in
`flintshot-editor-egui/src/home.rs`), `thiserror`, `serde` untouched.

## Commands

From repo root (same gates as AGENTS.md):

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
cargo run -- app        # manual: open an image, resize, undo, save
```

## Project structure

| Change                                    | File                                                        |
| ----------------------------------------- | ----------------------------------------------------------- |
| Pixel math                                | `crates/flintshot-core/src/image_ops.rs` — `resize()`       |
| Annotation transform                      | `crates/flintshot-core/src/editor.rs` — `Annotation::scale()` |
| State transition + undo                   | `crates/flintshot-core/src/editor.rs` — `EditorState::resize()` |
| Resize entry + popover UI            | `crates/flintshot-editor-egui/src/topbar.rs` (+ `app.rs` routing) |
| Unit tests                                | beside the code, `#[cfg(test)]` per repo convention         |

No new files or modules. UI is `topbar.rs` (+ `app.rs` routing);
`overlay.rs` and `app_window.rs` are untouched.

### API sketch

```rust
// image_ops.rs — thin, deterministic, toolkit-free
/// Scale `source` to `target` (never upscaling: each axis clamps to
/// the source dimension). No-op clone when the clamped target equals
/// the source dimensions or a target axis is 0.
#[must_use]
pub fn resize(source: &RgbaImage, target: (u32, u32)) -> RgbaImage;

// editor.rs — Annotation impl
/// Scale all geometry and style magnitudes (points, rects, stroke
/// widths, font sizes, head sizes, diameters, text-outline widths,
/// blur strength rounded to ≥ 1) by `factor`. `factor` must be
/// finite and > 0.
pub fn scale(&mut self, factor: f32);

// editor.rs — EditorState impl; mirrors crop()'s exact sequence
/// Resize the image to `target_w` px wide (height derived from the
/// aspect ratio, rounded, min 1). Clamps `target_w` to [1, width];
/// no-op at current width. Scales every annotation by the applied
/// factor. Pushes `Snapshot::full`, clears redo / pending / moves.
pub fn resize(&mut self, target_w: u32);
```

## Code style

Repo conventions apply (doc comments on public items, `#[must_use]`,
`clippy::pedantic` with `reason` strings, no `unwrap` outside tests,
physical-px f32 geometry). Example of the expected shape:

```rust
/// Scale `source` to `target` using a Triangle filter (best
/// quality-per-cost for downscale), never upscaling: each axis
/// clamps to its source dimension first.
///
/// Deterministic: same input + target ⇒ same output. A clamped
/// target equal to the source (or a zero axis) returns a clone.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "aspect-derived height is rounded before the u32 cast; dims ≤ 2^24"
)]
#[must_use]
pub fn resize(source: &RgbaImage, target: (u32, u32)) -> RgbaImage {
    let (sw, sh) = source.dimensions();
    let tw = target.0.clamp(1, sw).max(1);
    let th = target.1.clamp(1, sh).max(1);
    if (tw, th) == (sw, sh) {
        return source.clone();
    }
    image::imageops::resize(source, tw, th, image::imageops::FilterType::Triangle)
}
```

Percent semantics (UI layer): `new_w = ((w * pct) as f32).round().max(1)`
as u32, clamped to `w`. Custom width: integer px, clamped to
`[1, w]`; derived height `= (h * new_w / w).round().max(1)`.

## UI design

**Resize entry in the topbar's overflow menu** (the `⋯` menu holding
Save / Upload / Paste — the editor surface's actual export cluster;
the panel.rs "bottom cluster" module docs are stale, export actions
moved to the topbar). Editor surface only — Home / Settings / Loading
surfaces and the overlay get nothing. The item opens a small popover:

- Preset chips: **25% · 50% · 75%** — each shows the resulting
  `WxH` on hover; clicking applies immediately and closes.
- Custom row: a width field (integer, max = current width) with the
  derived `× H` label beside it; **Enter applies**. Width ≥ current
  disables apply (no upscale).
- Aspect ratio always locked; no height field.
- Entry disabled when the image is ≤ 1 px wide (nothing to shrink).

After apply: the view re-fits automatically (fit is computed from
image dims each frame — same path crop already exercises; no
`app.rs` changes expected).

## Testing strategy

All levels are existing repo patterns; no new frameworks.

- **Unit (core, `image_ops`):** resize returns clamped dims; identity
  target returns clone-equivalent pixels; zero/degenerate axes safe;
  deterministic output dims for odd aspect ratios (e.g. 101×51 @ 50%).
- **Unit (core, `editor`):** `Annotation::scale` touches every
  variant's magnitudes (stroke width, font size, head size, marker
  width, number diameter scale; points/rects multiply); non-finite /
  ≤ 0 factor rejected (no-op or assert in debug — pick per repo
  pattern). `EditorState::resize`: dims shrink, anns scale by the
  exact applied factor, `Snapshot::full` pushed; **undo restores the
  original image and annotations exactly**; redo replays the resize;
  no-op at current width (no snapshot pushed); selection/pending
  cleared like crop.
- **Unit (egui, `panel`):** chip renders in export island; popover
  preset click routes to `EditorState::resize` with the computed
  width; custom field clamps input ≥ image width; apply disabled at
  ≥ current width.
- **Manual:** `cargo run -- app` → open image → 50% → verify fit,
  magnifier, save output dims; Ctrl+Z roundtrip.

## Boundaries

- **Always:** core math stays toolkit-free (the
  `no_gui_types_in_core.rs` test must keep passing); resize routes
  through `EditorState` (no direct `image` mutation from UI); run the
  four gate commands before declaring done; update `CHANGELOG.md`
  under `## [Unreleased]`.
- **Ask first:** new dependencies (there should be none); touching
  `app_window.rs` or `overlay.rs`; changing `Snapshot` semantics;
  export-time resize variants (explicitly out of scope — that's an
  ADR-0015 conversation).
- **Never:** upscale paths; egui types in `flintshot-core` public
  API; `unwrap`/`expect` outside tests; `#[allow]` without a
  `reason`.

## Success criteria

- [x] 100×80 image with one Rect (stroke 4) + one Text (font 16):
      resize to width 50 ⇒ 50×40 image; stroke 2; font 8; points at
      half coordinates
      (`resize_scales_image_and_annotations`)
- [x] Undo after resize restores the 100×80 pixels and original
      annotation values bit-for-bit
      (`resize_is_undoable_and_redoable`, `resize_twice_then_undo_twice_restores_original`)
- [x] Any target ≥ current width is a no-op / clamped — no path
      produces a larger image
      (`resize_at_or_above_width_is_noop`, `resize_upscale_target_clamps_to_identity`,
      `resize_popover_refuses_upscale_targets`)
- [x] Editor topbar overflow shows Resize (editor surface only);
      Home / Settings / Loading and the overlay have no new UI
      (`overflow_shows_and_fires_resize_on_editor_surface`,
      `home_overflow_omits_editor_items`, `enable_matrix_matches_the_spec`;
      `overlay.rs` untouched)
- [x] `cargo fmt --check`, `clippy -D warnings`, both test commands
      pass

## Open questions

- None blocking. (Popover visual details follow existing
  `popover.rs`/`floating.rs` styling conventions; match the Crop
  popover's look.)
