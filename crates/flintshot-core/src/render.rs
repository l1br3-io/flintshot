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
/// The background texture handle is **cached** in `bg`: the first call
/// uploads `state.image` and stores the handle; subsequent calls blit
/// from the cached handle without re-uploading. The shell owns `bg`
/// (see `editor.rs`'s `EditorState` doc — the toolkit-specific handle
/// does *not* live in `EditorState`).
///
/// Callers must drop the cache (`*bg = None`) when `state.image` is
/// replaced (e.g. Task 14's crop replaces the source image with the
/// cropped sub-image). For Phase 1 Task 4 there is no such mutation
/// path, so the cache lives for the editor's lifetime.
///
/// # Errors
/// Propagates [`crate::FlintshotError`] if `canvas.upload_image` fails.
pub fn render<C: Canvas>(
    state: &EditorState,
    canvas: &mut C,
    bg: &mut Option<C::Image>,
) -> crate::Result<()> {
    if bg.is_none() {
        let handle = canvas.upload_image(&state.image)?;
        *bg = Some(handle);
    }
    let rect = image_rect(&state.image);
    if let Some(handle) = bg.as_ref() {
        canvas.draw_image(handle, rect);
    }
    // Phase 1 Task 7+ iterates `state.anns` and dispatches per variant
    // here. Background blit stays first; annotations draw on top.
    Ok(())
}

/// The full-image rect in physical pixels: top-left `(0, 0)`, size =
/// `image.dimensions()`. Used as the blit destination for the
/// background screenshot.
///
/// The `as f32` casts are precision-safe by orders of magnitude: f32's
/// 24-bit mantissa exactly represents integers up to 2^24 ≈ 16M, and
/// no plausible screenshot dimension approaches that.
#[allow(
    clippy::cast_precision_loss,
    reason = "image dims ≤ 2^24 are exact in f32"
)]
fn image_rect(image: &image::RgbaImage) -> Rect {
    Rect::from_xywh(0.0, 0.0, image.width() as f32, image.height() as f32)
}

#[cfg(test)]
mod tests {
    //! Task 4's tests use a minimal inline `MockCanvas` that records
    //! only the `draw_image` calls the background-blit path exercises.
    //! Task 7 promotes this to a public `MockCanvas` with a full
    //! `DrawCall` enum covering every `Canvas` method, used by snapshot
    //! tests across the annotation tools.
    use super::*;
    use crate::editor::EditorState;
    use crate::geom::Point;

    #[derive(Debug, Clone, PartialEq)]
    enum DrawCall {
        /// Recorded blit. Rect is the destination in physical px.
        DrawImage { rect: Rect },
        // Task 7 adds DrawRectOutline / DrawRectFilled / DrawLine / etc.
    }

    #[derive(Default)]
    struct MockCanvas {
        calls: Vec<DrawCall>,
    }

    impl Canvas for MockCanvas {
        type Image = ();
        fn upload_image(&mut self, _img: &image::RgbaImage) -> crate::Result<Self::Image> {
            Ok(())
        }
        fn draw_image(&mut self, _img: &Self::Image, rect: Rect) {
            self.calls.push(DrawCall::DrawImage { rect });
        }
        fn draw_rect_outline(&mut self, _: Rect, _: &Stroke) {}
        fn draw_rect_filled(&mut self, _: Rect, _: Color) {}
        fn draw_ellipse(&mut self, _: Rect, _: &Stroke, _: Option<Color>) {}
        fn draw_line(&mut self, _: Point, _: Point, _: &Stroke) {}
        fn draw_arrow(&mut self, _: Point, _: Point, _: &Stroke, _: f32) {}
        fn draw_text(&mut self, _: Point, _: &str, _: f32, _: Color) {}
    }

    #[test]
    fn render_blits_background_image_once() {
        let state = EditorState::new(image::RgbaImage::new(10, 20));
        let mut canvas = MockCanvas::default();
        let mut bg = None;
        render(&state, &mut canvas, &mut bg).expect("render should succeed");

        assert_eq!(canvas.calls.len(), 1, "exactly one draw_image call");
        match &canvas.calls[0] {
            DrawCall::DrawImage { rect } => {
                assert_eq!(rect.min, Point::new(0.0, 0.0));
                assert_eq!(rect.max, Point::new(10.0, 20.0));
            }
        }
    }

    #[test]
    fn render_caches_background_handle_across_frames() {
        let state = EditorState::new(image::RgbaImage::new(2, 2));
        let mut canvas = MockCanvas::default();
        let mut bg = None;

        render(&state, &mut canvas, &mut bg).expect("first render");
        assert!(bg.is_some(), "first render populates the cache");
        let calls_after_first = canvas.calls.len();

        render(&state, &mut canvas, &mut bg).expect("second render");
        assert_eq!(
            canvas.calls.len(),
            calls_after_first + 1,
            "second render blits but does not re-upload"
        );
    }

    #[test]
    fn render_reuploads_after_cache_invalidated() {
        // Models Task 14's crop path: shell drops the cache when
        // state.image is replaced, render re-uploads on next frame.
        let state = EditorState::new(image::RgbaImage::new(4, 4));
        let mut canvas = MockCanvas::default();
        let mut bg = None;

        render(&state, &mut canvas, &mut bg).expect("first render");
        bg = None; // simulate cache invalidation
        render(&state, &mut canvas, &mut bg).expect("second render after drop");

        // Two draw_image calls (one per render), cache re-populated.
        assert_eq!(canvas.calls.len(), 2);
        assert!(bg.is_some());
    }
}
