//! Pixel-zoom magnifier inset — a toolkit-owned widget that shows the
//! screenshot pixels under the cursor at high zoom.
//!
//! Per `docs/ARCHITECTURE.md` §What stays toolkit-specific, the
//! magnifier gets **no core abstraction**: it lives entirely in the
//! egui shell and reads `EditorState::image` directly. An iced / slint
//! shell implements its own (the pure sampling helper below is the only
//! piece a future shell *might* want, and it's cheap to re-derive).
//!
//! ## Why the sampling math is a pure helper
//!
//! egui types don't appear in its signature, so the geometry it
//! produces is verifiable headless — the unit tests below pin the
//! center-sample and edge-clamp behaviour without spinning up a live
//! window. The egui widget (`Magnifier::show`) is a thin layer over it:
//! sample → crop sub-image → upload as a one-frame texture → paint.

use flintshot_core::{Point, Rect};

/// On-screen inset edge length in logical px. The magnifier paints a
/// square of this size; higher = larger loupe but more GPU fill.
const INSET_PX: f32 = 96.0;

/// Zoom factor applied to the sampled source region. At `4.0`, each
/// source pixel covers a 4×4 block of inset pixels, so the user sees
/// chunky, individually-addressable pixels — the point of the tool.
const ZOOM: f32 = 4.0;

/// Compute the source-physical [`Rect`] to sample from the image so
/// that, stretched to `inset_px`, it appears `zoom`× magnified.
///
/// `inset_px / zoom` is the source edge length. The rect is centered on
/// `cursor_phys` but **clamped to the image bounds**: when the cursor
/// is near an edge the rect shifts inward so no out-of-bounds pixels
/// are requested. If the image is smaller than the source edge (e.g. a
/// tiny screenshot at high zoom), the rect collapses to the full image
/// — the inset then shows the whole image at less than the nominal
/// zoom, which is the least-surprising fallback.
///
/// `zoom` is clamped to a minimum of `1.0`: a magnifier should magnify,
/// never minify, and values below 1.0 would sample *more* pixels than
/// the inset (unbounded work). This is the "inset is bounded" guard
/// from the task spec — with `inset_px` fixed and `zoom >= 1.0`, the
/// source edge never exceeds `inset_px`.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "image dims ≤ 2^24 are exact in f32"
)]
pub fn sample_zoomed_region(
    image_dims: (u32, u32),
    cursor_phys: Point,
    zoom: f32,
    inset_px: f32,
) -> Rect {
    let (w, h) = image_dims;
    let dim_x = w as f32;
    let dim_y = h as f32;
    // Clamp zoom >= 1 so the source edge is bounded by inset_px.
    let z = zoom.max(1.0);
    let source_edge = (inset_px / z).min(dim_x).min(dim_y);
    let half = source_edge / 2.0;

    // Centre on the cursor, then shift back inside [0, dim] on each
    // axis independently. Computing each axis separately keeps the
    // rect rectangular even when the cursor is in a corner.
    let min_x = clamp_centered(cursor_phys.x, half, dim_x);
    let min_y = clamp_centered(cursor_phys.y, half, dim_y);

    Rect::from_xywh(min_x, min_y, source_edge, source_edge)
}

/// Place a span of length `2 * half` (centred on `centre`) inside
/// `[0, dim]`: returns the span's min. If the span is wider than `dim`,
/// returns `0.0` (the caller then builds a span capped at `dim`).
fn clamp_centered(centre: f32, half: f32, dim: f32) -> f32 {
    let span = 2.0 * half;
    if span >= dim {
        return 0.0;
    }
    let ideal_min = centre - half;
    let ideal_max = centre + half;
    if ideal_min < 0.0 {
        0.0
    } else if ideal_max > dim {
        dim - span
    } else {
        ideal_min
    }
}

/// Crop a sub-image out of `src` covering `region` (physical px).
///
/// Out-of-bounds regions are clipped to the image; a region fully
/// outside yields an empty `1×1` image as a degenerate-but-non-panicking
/// fallback (the source rect from `sample_zoomed_region` is already
/// in-bounds, so this is purely defensive).
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "region is clamped to image bounds before casting; dims ≤ 2^24 are exact in f32"
)]
fn crop_subimage(src: &image::RgbaImage, region: Rect) -> image::RgbaImage {
    let (w, h) = src.dimensions();
    let x0 = region.min.x.clamp(0.0, w as f32).floor() as u32;
    let y0 = region.min.y.clamp(0.0, h as f32).floor() as u32;
    let x1 = region.max.x.clamp(0.0, w as f32).ceil() as u32;
    let y1 = region.max.y.clamp(0.0, h as f32).ceil() as u32;
    let rw = x1.saturating_sub(x0).max(1);
    let rh = y1.saturating_sub(y0).max(1);
    let view = image::imageops::crop_imm(src, x0, y0, rw, rh).to_image();
    if view.dimensions() == (0, 0) {
        image::RgbaImage::new(1, 1)
    } else {
        view
    }
}

/// Cached magnifier state held by the editor app across frames.
///
/// The `TextureHandle` is created lazily on first `show` and reused
/// thereafter via `TextureHandle::set`, which updates the GPU buffer
/// in place rather than reallocating. This is the "cache the inset
/// texture across frames while held" mitigation from the task spec:
/// per-frame work stays a buffer update + one paint call, not a fresh
/// texture allocation. Keeping the handle across show/hide cycles (not
/// just within a single hold) is intentional — re-showing is then a
/// `set` rather than a `load_texture`, and a `96×96` RGBA buffer is
/// negligible footprint to retain.
#[derive(Default)]
pub struct Magnifier {
    texture: Option<egui::TextureHandle>,
}

impl Magnifier {
    /// Construct a magnifier with no cached texture yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sample `image` around `cursor_phys`, upload the zoomed pixels,
    /// and paint the inset at a fixed offset from the cursor.
    ///
    /// `canvas_rect` is the screen-logical rect of the canvas panel
    /// (used to keep the inset on-screen). `ppp` is `pixels_per_point`.
    /// `cursor_phys` is in the same physical-px space as `image`.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "inset logical offsets and cursor coords are non-negative within the canvas"
    )]
    pub fn show(
        &mut self,
        painter: &egui::Painter,
        ctx: &egui::Context,
        image: &image::RgbaImage,
        cursor_phys: Point,
        canvas_rect: egui::Rect,
        ppp: f32,
    ) {
        let src = sample_zoomed_region(image.dimensions(), cursor_phys, ZOOM, INSET_PX);
        let sub = crop_subimage(image, src);

        // GOTCHA: `load_texture` allocates a fresh GPU buffer + entry
        // in egui's named-texture map each call. The blur path
        // (`canvas.rs` `draw_image_region`) now sidesteps this via its
        // content-keyed region-texture cache (Phase 5 Task 008); the
        // magnifier can't — its pixels change on every pointer event —
        // so we cache the handle and route updates through `set`,
        // which uploads into the existing buffer.
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [sub.width() as usize, sub.height() as usize],
            sub.as_raw(),
        );
        let handle = if let Some(h) = &self.texture {
            // `TextureHandle` is a cheap shared ref into egui's
            // texture manager (clone bumps a refcount, not a GPU
            // copy), so cloning here then `set`-ing updates the
            // same underlying buffer the cached entry points at.
            let mut h = h.clone();
            h.set(color_image, egui::TextureOptions::NEAREST);
            h
        } else {
            let h = ctx.load_texture(
                "flintshot-magnifier",
                color_image,
                egui::TextureOptions::NEAREST,
            );
            self.texture = Some(h.clone());
            h
        };

        // Inset on-screen size, in logical px.
        let inset_logical = INSET_PX / ppp;
        // Offset the inset down-and-right of the cursor so the cursor
        // itself stays visible; clamp to keep it inside the canvas.
        let cursor_logical = egui::pos2(
            cursor_phys.x / ppp + canvas_rect.min.x,
            cursor_phys.y / ppp + canvas_rect.min.y,
        );
        let offset = 16.0;
        let mut dest_min = cursor_logical + egui::vec2(offset, offset);
        // If the inset would run off the right/bottom edge, flip it to
        // the opposite side of the cursor so it stays on-canvas.
        if dest_min.x + inset_logical > canvas_rect.max.x {
            dest_min.x = cursor_logical.x - offset - inset_logical;
        }
        if dest_min.y + inset_logical > canvas_rect.max.y {
            dest_min.y = cursor_logical.y - offset - inset_logical;
        }
        let dest_min = egui::pos2(
            dest_min.x.max(canvas_rect.min.x),
            dest_min.y.max(canvas_rect.min.y),
        );
        let dest_rect =
            egui::Rect::from_min_size(dest_min, egui::vec2(inset_logical, inset_logical));

        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        painter.image(handle.id(), dest_rect, uv, egui::Color32::WHITE);
        // 1-px frame so the inset is distinguishable from the image
        // underneath, especially over uniform-color regions.
        painter.rect_stroke(
            dest_rect,
            egui::CornerRadius::ZERO,
            egui::Stroke::new(1.0, egui::Color32::from_black_alpha(180)),
            egui::StrokeKind::Outside,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flintshot_core::{Point, Rect};

    /// Centre of a 100×100 image, 4× zoom, 96 inset → 24×24 source
    /// centred on the cursor.
    #[test]
    fn sample_centred_on_cursor() {
        let r = sample_zoomed_region((100, 100), Point::new(50.0, 50.0), 4.0, 96.0);
        assert!((r.width() - 24.0).abs() < 1e-3, "width: {}", r.width());
        assert!((r.height() - 24.0).abs() < 1e-3, "height: {}", r.height());
        // Centred: min = 50 - 12, max = 50 + 12.
        assert!((r.min.x - 38.0).abs() < 1e-3, "min.x: {}", r.min.x);
        assert!((r.max.x - 62.0).abs() < 1e-3, "max.x: {}", r.max.x);
        assert!((r.min.y - 38.0).abs() < 1e-3, "min.y: {}", r.min.y);
        assert!((r.max.y - 62.0).abs() < 1e-3, "max.y: {}", r.max.y);
    }

    /// Cursor near the top-left corner: the rect shifts inward and
    /// stays fully in-bounds (no negative coords).
    #[test]
    fn sample_clamps_at_top_left_corner() {
        let r = sample_zoomed_region((100, 100), Point::new(5.0, 5.0), 4.0, 96.0);
        assert!(r.min.x >= 0.0, "min.x negative: {}", r.min.x);
        assert!(r.min.y >= 0.0, "min.y negative: {}", r.min.y);
        assert!(r.max.x <= 100.0, "max.x oob: {}", r.max.x);
        assert!(r.max.y <= 100.0, "max.y oob: {}", r.max.y);
        // Clamped to the corner: min is 0 on both axes (the rect
        // couldn't centre without going off the top-left edge).
        assert!((r.min.x - 0.0).abs() < 1e-3);
        assert!((r.min.y - 0.0).abs() < 1e-3);
        assert!((r.width() - 24.0).abs() < 1e-3);
    }

    /// Cursor near the bottom-right corner: rect shifts inward.
    #[test]
    fn sample_clamps_at_bottom_right_corner() {
        let r = sample_zoomed_region((100, 100), Point::new(95.0, 95.0), 4.0, 96.0);
        assert!(r.min.x >= 0.0);
        assert!(r.min.y >= 0.0);
        assert!(r.max.x <= 100.0, "max.x oob: {}", r.max.x);
        assert!(r.max.y <= 100.0, "max.y oob: {}", r.max.y);
        // Clamped to bottom-right: max is 100 on both axes.
        assert!((r.max.x - 100.0).abs() < 1e-3, "max.x: {}", r.max.x);
        assert!((r.max.y - 100.0).abs() < 1e-3, "max.y: {}", r.max.y);
        assert!((r.width() - 24.0).abs() < 1e-3);
    }

    /// The returned rect never extends past the image on either axis,
    /// regardless of cursor position — the property the render path
    /// relies on to avoid sampling out of bounds.
    #[test]
    fn sample_never_out_of_bounds() {
        for &cx in &[0.0_f32, 1.0, 50.0, 99.0, 100.0, -5.0] {
            for &cy in &[0.0_f32, 1.0, 50.0, 99.0, 100.0, -5.0] {
                let r = sample_zoomed_region((100, 100), Point::new(cx, cy), 4.0, 96.0);
                assert!(r.min.x >= 0.0, "cx={cx} cy={cy} min.x={}", r.min.x);
                assert!(r.min.y >= 0.0, "cx={cx} cy={cy} min.y={}", r.min.y);
                assert!(r.max.x <= 100.0, "cx={cx} cy={cy} max.x={}", r.max.x);
                assert!(r.max.y <= 100.0, "cx={cx} cy={cy} max.y={}", r.max.y);
            }
        }
    }

    /// Zoom below 1.0 is clamped to 1.0 — the magnifier never minifies,
    /// and the source edge is bounded by the inset size.
    #[test]
    fn zoom_below_one_is_clamped() {
        let r = sample_zoomed_region((100, 100), Point::new(50.0, 50.0), 0.5, 96.0);
        // Source edge = inset / max(zoom, 1) = 96 / 1 = 96.
        assert!((r.width() - 96.0).abs() < 1e-3, "width: {}", r.width());
    }

    /// An image smaller than the source edge: the rect collapses to
    /// the full image rather than reporting a region larger than it.
    #[test]
    fn sample_on_image_smaller_than_inset() {
        let r = sample_zoomed_region((10, 10), Point::new(5.0, 5.0), 4.0, 96.0);
        // source_edge = min(96/4, 10, 10) = 10.
        assert!((r.width() - 10.0).abs() < 1e-3, "width: {}", r.width());
        assert!((r.min.x - 0.0).abs() < 1e-3);
        assert!((r.max.x - 10.0).abs() < 1e-3);
    }

    /// `crop_subimage` returns only the pixels inside the region.
    #[test]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "test image is 10×10; x,y fit in u8"
    )]
    fn crop_subimage_extracts_region() {
        let mut img = image::RgbaImage::new(10, 10);
        for x in 0..10u32 {
            for y in 0..10u32 {
                img.put_pixel(x, y, image::Rgba([x as u8, y as u8, 0, 255]));
            }
        }
        let sub = crop_subimage(&img, Rect::from_xywh(2.0, 2.0, 4.0, 4.0));
        assert_eq!(sub.dimensions(), (4, 4));
        let px = sub.get_pixel(0, 0);
        assert_eq!(px.0, [2, 2, 0, 255]);
    }
}
