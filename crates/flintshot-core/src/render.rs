//! The immediate-mode drawing trait that UI toolkits implement.
//!
//! `Canvas` exposes a minimal set of primitives sufficient to render
//! annotations. The associated [`Canvas::Image`] type lets each toolkit
//! pass its own native texture handle (`egui::TextureHandle`, etc.)
//! through without leaking that type into core.
//!
//! The shared [`render`] free function walks an [`crate::EditorState`]
//! and emits draw calls via a `Canvas`. All toolkits produce identical
//! output because all decisions (geometry, blur/pixelate computation)
//! live here.

use crate::{
    editor::EditorState,
    geom::{Color, Point, Rect, Stroke},
};

/// Immediate-mode drawing primitives for annotations.
///
/// Non-object-safe by design (associated [`Image`] type). We always know
/// the concrete `Canvas` impl at the call site, so this costs us
/// nothing. See ADR-0012.
///
/// [`Image`]: Canvas::Image
pub trait Canvas {
    /// Toolkit-specific texture handle for an uploaded image.
    type Image;

    /// Upload a source-image into the toolkit's texture cache.
    /// Called once per source screenshot; the returned handle is reused.
    ///
    /// # Errors
    /// Implementations surface upload failures via [`crate::FlintshotError`].
    fn upload_image(&mut self, img: &image::RgbaImage) -> crate::Result<Self::Image>;

    /// Blit a previously-uploaded image into the given rectangle.
    fn draw_image(&mut self, img: &Self::Image, rect: Rect);

    /// Outline a rectangle with `stroke`.
    fn draw_rect_outline(&mut self, rect: Rect, stroke: &Stroke);

    /// Fill a rectangle with a solid `color`.
    fn draw_rect_filled(&mut self, rect: Rect, color: Color);

    /// Outline and (optionally) fill an ellipse inscribed in `rect`.
    fn draw_ellipse(&mut self, rect: Rect, stroke: &Stroke, fill: Option<Color>);

    /// Stroke a line segment from `a` to `b`.
    fn draw_line(&mut self, a: Point, b: Point, stroke: &Stroke);

    /// Stroke an arrow from `a` (tail) to `b` (head), with barbs of `head_size`.
    fn draw_arrow(&mut self, a: Point, b: Point, stroke: &Stroke, head_size: f32);

    /// Draw left-to-right text starting at `pos`. (Shaping stays
    /// toolkit-specific; core only specifies the run.)
    fn draw_text(&mut self, pos: Point, text: &str, size: f32, color: Color);
}

/// Render `state` (background image + all annotations) via `canvas`.
///
/// This is the single shared render path used by every shell. Phase 0
/// leaves the body a stub; Phase 1 fills it in as the annotation
/// variants land.
///
/// # Errors
/// Propagates [`crate::FlintshotError`] if `canvas.upload_image` fails.
pub fn render<C: Canvas>(_state: &EditorState, _canvas: &mut C) -> crate::Result<()> {
    // Phase 1: upload state.image, iterate state.anns, dispatch per variant.
    Ok(())
}
