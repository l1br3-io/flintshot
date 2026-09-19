//! Image-level operations computed in core (blur, pixelate, crop,
//! resize).
//!
//! These are toolkit-independent: they operate on [`image::RgbaImage`]
//! directly and produce results that any [`crate::Canvas`] can blit.
//! Per ADR-0015 the live editor view and the saved/copied image must
//! agree, so the *computation* lives here rather than in any one shell.

use crate::editor::BlurKind;
use image::{GenericImageView, RgbaImage};

/// Apply a blur / pixelate of the given `kind` and `strength` to
/// `region`, returning a new image the same size as the input.
///
/// `strength` is interpreted per `kind`:
/// - [`BlurKind::Gaussian`]: sigma in physical px. Realistic range 1–20;
///   values outside that still work but are rarely useful.
/// - [`BlurKind::Pixelate`]: block size in physical px (each block is
///   averaged to a single colour). Realistic range 2–32.
///
/// Deterministic: the same input + parameters always produce the same
/// output (Gaussian uses `imageproc`'s separable filter; pixelate is a
/// straightforward block average).
#[must_use]
pub fn blur(region: &RgbaImage, kind: BlurKind, strength: u8) -> RgbaImage {
    match kind {
        BlurKind::Gaussian => gaussian_blur(region, f32::from(strength)),
        BlurKind::Pixelate => pixelate(region, u32::from(strength).max(1)),
    }
}

/// Gaussian blur via `imageproc::filter::gaussian_blur_f32`.
///
/// `imageproc`'s `Image<P>` is a type alias for `image::ImageBuffer<P,
/// Vec<_>>`, so `&RgbaImage` passes directly with no conversion. A
/// sigma ≤ 0 is a no-op (return a clone) — `imageproc` panics on
/// non-positive sigma, so we guard before the call.
fn gaussian_blur(region: &RgbaImage, sigma: f32) -> RgbaImage {
    if sigma <= 0.0 {
        return region.clone();
    }
    imageproc::filter::gaussian_blur_f32(region, sigma)
}

/// Pixelate by averaging each `block × block` cell to its mean colour.
///
/// Edges whose size is not a multiple of `block` are averaged over
/// their remaining partial block. A block size of 1 is a no-op (each
/// pixel is its own block).
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    reason = "channel sums are divided by count before the u8 cast; w/h/x/y/dx/dy are idiomatic raster-loop counters"
)]
fn pixelate(region: &RgbaImage, block: u32) -> RgbaImage {
    if block <= 1 {
        return region.clone();
    }
    let (w, h) = region.dimensions();
    let mut out = RgbaImage::new(w, h);
    let mut y = 0;
    while y < h {
        let by = block.min(h - y);
        let mut x = 0;
        while x < w {
            let bx = block.min(w - x);
            // Accumulate per-channel sums; averaging four u8 channels
            // over a block can't exceed u64, and the mean fits in u8.
            let mut sums = [0u64; 4];
            let mut count = 0u64;
            for dy in 0..by {
                for dx in 0..bx {
                    let p = region.get_pixel(x + dx, y + dy).0;
                    sums[0] += u64::from(p[0]);
                    sums[1] += u64::from(p[1]);
                    sums[2] += u64::from(p[2]);
                    sums[3] += u64::from(p[3]);
                    count += 1;
                }
            }
            let avg = image::Rgba([
                (sums[0] / count) as u8,
                (sums[1] / count) as u8,
                (sums[2] / count) as u8,
                (sums[3] / count) as u8,
            ]);
            for dy in 0..by {
                for dx in 0..bx {
                    out.put_pixel(x + dx, y + dy, avg);
                }
            }
            x += block;
        }
        y += block;
    }
    out
}

/// Crop `source` to `rect` (clamped to image bounds), returning a new
/// owned sub-image. Used by the blur render path to extract the region
/// before blurring.
///
/// Out-of-bounds rects are clamped to the nearest valid sub-rect; a
/// rect entirely outside the image returns an empty 0×0 image.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "coords clamped to [0, image_dim] and rounded before cast; dims ≤ 2^24"
)]
#[must_use]
pub fn crop_region(source: &RgbaImage, rect: crate::Rect) -> RgbaImage {
    let (w, h) = source.dimensions();
    let n = rect.normalized();
    let (w_f, h_f) = (w as f32, h as f32);
    let x0 = (n.min.x.round().clamp(0.0, w_f) as u32).min(w);
    let y0 = (n.min.y.round().clamp(0.0, h_f) as u32).min(h);
    let x1 = (n.max.x.round().clamp(0.0, w_f) as u32).min(w).max(x0);
    let y1 = (n.max.y.round().clamp(0.0, h_f) as u32).min(h).max(y0);
    let cw = x1.saturating_sub(x0);
    let ch = y1.saturating_sub(y0);
    if cw == 0 || ch == 0 {
        return RgbaImage::new(0, 0);
    }
    source.view(x0, y0, cw, ch).to_image()
}

/// Scale `source` to `target` using a Triangle filter (best
/// quality-per-cost for downscale), never upscaling: each target axis
/// clamps to `[1, source_axis]` first.
///
/// A clamped target equal to the source dims (or a zero-axis source)
/// returns a clone — no resample. Deterministic: same input + target
/// ⇒ same output. This is the pixel primitive behind
/// [`crate::EditorState::resize`].
#[must_use]
pub fn resize(source: &RgbaImage, target: (u32, u32)) -> RgbaImage {
    let (sw, sh) = source.dimensions();
    if sw == 0 || sh == 0 {
        return source.clone();
    }
    let tw = target.0.clamp(1, sw);
    let th = target.1.clamp(1, sh);
    if (tw, th) == (sw, sh) {
        return source.clone();
    }
    image::imageops::resize(source, tw, th, image::imageops::FilterType::Triangle)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Downscale produces exactly the requested dims; every pixel is
    /// sampled (gradient survives, so it isn't a blank/solid output).
    #[test]
    fn resize_downscale_dims_exact() {
        let mut img = RgbaImage::new(100, 80);
        for y in 0..80 {
            for x in 0..100 {
                #[allow(
                    clippy::cast_possible_truncation,
                    reason = "x, y are 0..100, fit in u8"
                )]
                let (x8, y8) = (x as u8, y as u8);
                img.put_pixel(x, y, image::Rgba([x8, y8, 128, 255]));
            }
        }
        let out = resize(&img, (50, 40));
        assert_eq!(out.dimensions(), (50, 40));
        assert_ne!(out.get_pixel(0, 0).0, out.get_pixel(49, 39).0);
    }

    /// Upscale targets clamp to the source dims (identity, no
    /// resample) — no input produces a larger image.
    #[test]
    fn resize_upscale_target_clamps_to_identity() {
        let img = RgbaImage::from_pixel(100, 80, image::Rgba([7, 8, 9, 255]));
        let out = resize(&img, (200, 160));
        assert_eq!(out.dimensions(), (100, 80));
        assert_eq!(out.get_pixel(42, 17).0, [7, 8, 9, 255]);
    }

    /// Zero / degenerate target axes clamp to 1, never 0.
    #[test]
    fn resize_zero_axes_clamp_to_one() {
        let img = RgbaImage::from_pixel(10, 10, image::Rgba([1, 2, 3, 255]));
        assert_eq!(resize(&img, (0, 0)).dimensions(), (1, 1));
        assert_eq!(resize(&img, (0, 4)).dimensions(), (1, 4));
        assert_eq!(resize(&img, (4, 0)).dimensions(), (4, 1));
    }

    /// A zero-axis source is passed through untouched (nothing to
    /// sample; a 1-px target would be meaningless).
    #[test]
    fn resize_empty_source_returns_clone() {
        let img = RgbaImage::new(0, 0);
        assert_eq!(resize(&img, (10, 10)).dimensions(), (0, 0));
    }

    /// Odd aspect dims resize to the exact requested odd target.
    #[test]
    fn resize_odd_dims_exact() {
        let img = RgbaImage::from_pixel(101, 51, image::Rgba([200, 100, 50, 255]));
        let out = resize(&img, (50, 25));
        assert_eq!(out.dimensions(), (50, 25));
    }

    /// Deterministic: same input + target ⇒ byte-identical output.
    #[test]
    fn resize_deterministic() {
        let mut img = RgbaImage::new(64, 32);
        for y in 0..32 {
            for x in 0..64 {
                #[allow(clippy::cast_possible_truncation, reason = "x, y are 0..64, fit in u8")]
                let (x8, y8) = (x as u8, y as u8);
                img.put_pixel(x, y, image::Rgba([x8 * 3, y8 * 5, 7, 255]));
            }
        }
        let a = resize(&img, (21, 11));
        let b = resize(&img, (21, 11));
        assert_eq!(a.dimensions(), (21, 11));
        assert_eq!(a.as_raw(), b.as_raw());
    }

    /// Half-black / half-white image: blurring across the boundary
    /// must produce intermediate grey pixels at the seam.
    #[test]
    fn gaussian_blur_smooths_sharp_boundary() {
        let mut img = RgbaImage::new(20, 10);
        for x in 0..10 {
            for y in 0..10 {
                img.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
            }
        }
        for x in 10..20 {
            for y in 0..10 {
                img.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
            }
        }
        let blurred = blur(&img, BlurKind::Gaussian, 3);
        let mid = blurred.get_pixel(10, 5).0;
        assert!(mid[0] > 20 && mid[0] < 235, "boundary pixel was {}", mid[0]);
    }

    /// Pixelate collapses each block to a single colour; the output
    /// has at most `(w/block)·(h/block)` distinct colours.
    #[test]
    fn pixelate_averages_blocks() {
        let mut img = RgbaImage::new(4, 2);
        for x in 0..2 {
            for y in 0..2 {
                img.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
            }
        }
        for x in 2..4 {
            for y in 0..2 {
                img.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
            }
        }
        let pix = blur(&img, BlurKind::Pixelate, 2);
        assert_eq!(pix.get_pixel(0, 0).0, [0, 0, 0, 255]);
        assert_eq!(pix.get_pixel(1, 0).0, [0, 0, 0, 255]);
        assert_eq!(pix.get_pixel(2, 0).0, [255, 255, 255, 255]);
        assert_eq!(pix.get_pixel(3, 0).0, [255, 255, 255, 255]);
    }

    /// Gaussian with sigma 0 is a no-op (returns a clone).
    #[test]
    fn gaussian_blur_zero_sigma_is_noop() {
        let img = RgbaImage::from_pixel(4, 4, image::Rgba([10, 20, 30, 255]));
        let out = blur(&img, BlurKind::Gaussian, 0);
        assert_eq!(out.dimensions(), (4, 4));
        assert_eq!(out.get_pixel(0, 0).0, [10, 20, 30, 255]);
    }

    /// `crop_region` extracts the sub-image at the rect, clamped.
    #[test]
    fn crop_region_extracts_subimage() {
        let mut img = RgbaImage::new(10, 10);
        for y in 0..10 {
            for x in 0..10 {
                #[allow(clippy::cast_possible_truncation, reason = "x, y are 0..10, fit in u8")]
                let (x8, y8) = (x as u8, y as u8);
                img.put_pixel(x, y, image::Rgba([x8, y8, 0, 255]));
            }
        }
        let cropped = crop_region(&img, crate::Rect::from_xywh(2.0, 3.0, 4.0, 5.0));
        assert_eq!(cropped.dimensions(), (4, 5));
        assert_eq!(cropped.get_pixel(0, 0).0, [2, 3, 0, 255]);
        assert_eq!(cropped.get_pixel(3, 4).0, [5, 7, 0, 255]);
    }
}
