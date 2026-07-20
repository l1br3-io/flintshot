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
//!
//! [`MockCanvas`] + [`DrawCall`] provide a snapshot-test infrastructure
//! for the render path: tests construct an `EditorState`, run `render`
//! against a `MockCanvas`, and assert the recorded `Vec<DrawCall>`.

use crate::{
    editor::{Annotation, EditorState},
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
/// After the background blit, this walks `state.anns` in draw order
/// and dispatches each annotation via [`render_ann`]. If `state.pending`
/// is set it is drawn on top (last) so the user sees live feedback
/// during a drag.
///
/// # Errors
/// Propagates [`crate::FlintshotError`] if `canvas.upload_image` fails
/// (only on the first call, when the cache is cold).
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
    for ann in &state.anns {
        render_ann(canvas, ann);
    }
    if let Some(pending) = &state.pending {
        render_ann(canvas, pending);
    }
    Ok(())
}

/// Dispatch a single annotation to the appropriate `Canvas` primitive(s).
///
/// Public so future tool-specific render paths (e.g. Task 13's blur
/// pre-computation) can reuse the per-variant mapping. Pending
/// annotations call this same path; the only difference is whether
/// they live in `EditorState::anns` or `EditorState::pending`.
pub fn render_ann<C: Canvas>(canvas: &mut C, ann: &Annotation) {
    match ann {
        Annotation::Rect { rect, stroke, fill } => {
            let n = rect.normalized();
            if let Some(color) = fill {
                canvas.draw_rect_filled(n, *color);
            }
            canvas.draw_rect_outline(n, stroke);
        }
        Annotation::Ellipse { rect, stroke, fill } => {
            canvas.draw_ellipse(rect.normalized(), stroke, *fill);
        }
        Annotation::Line { a, b, stroke } => canvas.draw_line(*a, *b, stroke),
        Annotation::Arrow {
            a,
            b,
            stroke,
            head_size,
        } => canvas.draw_arrow(*a, *b, stroke, *head_size),
        Annotation::Marker {
            points,
            color,
            width,
        }
        | Annotation::Pencil {
            points,
            color,
            width,
        } => {
            let stroke = Stroke::new(*color, *width);
            for w in points.windows(2) {
                canvas.draw_line(w[0], w[1], &stroke);
            }
        }
        Annotation::Text {
            pos,
            text,
            font_size,
            color,
        } => canvas.draw_text(*pos, text, *font_size, *color),
        Annotation::Blur { rect, .. } => {
            // Placeholder until Task 13 adds `image_ops::blur`. Visible
            // grey overlay so annotations don't appear "missing" during
            // the Task 7 → Task 13 gap; warn so a session trace shows
            // that real blur is not yet being applied.
            tracing::warn!(
                ?rect,
                "blur annotation rendered as placeholder; real blur lands in Task 13"
            );
            canvas.draw_rect_filled(rect.normalized(), Color::rgb(64, 64, 64));
        }
        Annotation::Number {
            pos,
            n,
            color,
            diameter,
        } => {
            // Bubble: filled circle approximated as a square rect;
            // fine for hit-testing and visible feedback, refined when
            // Task 11 / 12 add real primitives.
            let half = *diameter / 2.0;
            let bbox = Rect::from_xywh(pos.x - half, pos.y - half, *diameter, *diameter);
            canvas.draw_rect_filled(bbox, *color);
            let label = n.to_string();
            // Contrast text: white on dark fills, black on light. We
            // don't compute luminance here; pick white as a default —
            // refined in Task 15 when pickers land.
            canvas.draw_text(*pos, &label, *diameter * 0.6, Color::rgb(255, 255, 255));
        }
    }
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

/// Record of a single [`Canvas`] call, used by [`MockCanvas`] for
/// snapshot tests.
///
/// One variant per `Canvas` method. Field names mirror the method
/// parameter names so test failures read clearly. Owned data
/// (`String` for text) so the recording outlives the call.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawCall {
    /// `Canvas::upload_image` — no payload recorded (the mock discards bytes).
    UploadImage,
    /// `Canvas::draw_image` — destination rect in physical px.
    DrawImage {
        /// Blit destination.
        rect: Rect,
    },
    /// `Canvas::draw_rect_outline`.
    DrawRectOutline {
        /// Geometry (normalized).
        rect: Rect,
        /// Stroke style.
        stroke: Stroke,
    },
    /// `Canvas::draw_rect_filled`.
    DrawRectFilled {
        /// Geometry (normalized).
        rect: Rect,
        /// Fill color.
        color: Color,
    },
    /// `Canvas::draw_ellipse`.
    DrawEllipse {
        /// Bounding rect (normalized).
        rect: Rect,
        /// Outline style.
        stroke: Stroke,
        /// Optional fill.
        fill: Option<Color>,
    },
    /// `Canvas::draw_line`.
    DrawLine {
        /// Endpoint A.
        a: Point,
        /// Endpoint B.
        b: Point,
        /// Stroke style.
        stroke: Stroke,
    },
    /// `Canvas::draw_arrow`.
    DrawArrow {
        /// Tail endpoint.
        a: Point,
        /// Head endpoint.
        b: Point,
        /// Stroke style.
        stroke: Stroke,
        /// Barb length in physical px.
        head_size: f32,
    },
    /// `Canvas::draw_text`.
    DrawText {
        /// Anchor (top-left).
        pos: Point,
        /// Text content.
        text: String,
        /// Font size in physical px.
        size: f32,
        /// Text color.
        color: Color,
    },
}

/// Test-only [`Canvas`] that records every call into a public
/// [`Vec<DrawCall>`](Vec).
///
/// Used by snapshot tests across `flintshot-core` (render path,
/// annotation dispatch) and by the editor state machine's render-touch
/// assertions. Real shells (`EguiCanvas`) implement the same trait
/// with actual painting side effects.
///
/// `Image` is `()` because `MockCanvas` keeps no texture state — each
/// `upload_image` call is just recorded, and subsequent `draw_image`
/// calls receive `&()` (the mock doesn't dereference it).
#[derive(Debug, Default)]
pub struct MockCanvas {
    /// Recorded draw calls, in the order they were issued.
    pub calls: Vec<DrawCall>,
}

impl MockCanvas {
    /// Construct an empty `MockCanvas`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Canvas for MockCanvas {
    type Image = ();

    fn upload_image(&mut self, _img: &image::RgbaImage) -> crate::Result<Self::Image> {
        self.calls.push(DrawCall::UploadImage);
        Ok(())
    }

    fn draw_image(&mut self, _img: &Self::Image, rect: Rect) {
        self.calls.push(DrawCall::DrawImage { rect });
    }

    fn draw_rect_outline(&mut self, rect: Rect, stroke: &Stroke) {
        self.calls.push(DrawCall::DrawRectOutline {
            rect,
            stroke: *stroke,
        });
    }

    fn draw_rect_filled(&mut self, rect: Rect, color: Color) {
        self.calls.push(DrawCall::DrawRectFilled { rect, color });
    }

    fn draw_ellipse(&mut self, rect: Rect, stroke: &Stroke, fill: Option<Color>) {
        self.calls.push(DrawCall::DrawEllipse {
            rect,
            stroke: *stroke,
            fill,
        });
    }

    fn draw_line(&mut self, a: Point, b: Point, stroke: &Stroke) {
        self.calls.push(DrawCall::DrawLine {
            a,
            b,
            stroke: *stroke,
        });
    }

    fn draw_arrow(&mut self, a: Point, b: Point, stroke: &Stroke, head_size: f32) {
        self.calls.push(DrawCall::DrawArrow {
            a,
            b,
            stroke: *stroke,
            head_size,
        });
    }

    fn draw_text(&mut self, pos: Point, text: &str, size: f32, color: Color) {
        self.calls.push(DrawCall::DrawText {
            pos,
            text: text.to_owned(),
            size,
            color,
        });
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    //! Render-path tests using the public `MockCanvas`. Task 4 added
    //! the background-blit coverage (3 tests below); Task 7 adds
    //! annotation dispatch coverage (1 test for committed + pending).
    use super::*;
    use crate::editor::{Annotation, EditorState, Tool};
    use crate::geom::{Color, Point, Rect, Stroke};

    #[test]
    fn render_blits_background_image_once() {
        let state = EditorState::new(image::RgbaImage::new(10, 20));
        let mut canvas = MockCanvas::default();
        let mut bg = None;
        render(&state, &mut canvas, &mut bg).expect("render should succeed");

        assert_eq!(canvas.calls.len(), 2, "upload + blit");
        assert!(matches!(canvas.calls[0], DrawCall::UploadImage));
        match &canvas.calls[1] {
            DrawCall::DrawImage { rect } => {
                assert_eq!(rect.min, Point::new(0.0, 0.0));
                assert_eq!(rect.max, Point::new(10.0, 20.0));
            }
            other => panic!("expected DrawImage, got {other:?}"),
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
        // No `UploadImage` in the second frame's delta.
        assert!(
            !canvas.calls[calls_after_first..]
                .iter()
                .any(|c| matches!(c, DrawCall::UploadImage)),
            "no re-upload on cached frame"
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

        // Two uploads + two blits across the two frames.
        assert_eq!(canvas.calls.len(), 4);
        assert!(bg.is_some());
    }

    /// TODO Task 7 acceptance: render an `EditorState` with one
    /// committed Rect ann and one pending Rect; assert recorded
    /// `DrawCall` sequence.
    #[test]
    fn render_draws_committed_and_pending_rects() {
        let mut state = EditorState::new(image::RgbaImage::new(100, 100));
        state.tool = Tool::Rect;
        // Committed rect (already normalized — `commit_annotation`
        // would normalize; we set it directly to keep the test focused
        // on the render dispatch contract).
        state.anns.push(Annotation::Rect {
            rect: Rect::from_xywh(10.0, 10.0, 30.0, 30.0),
            stroke: Stroke::new(Color::RED, 3.0),
            fill: None,
        });
        // Pending rect (un-normalized during drag, like begin→update).
        state.pending = Some(Annotation::Rect {
            rect: Rect::new(Point::new(80.0, 80.0), Point::new(20.0, 20.0)),
            stroke: Stroke::new(Color::rgb(0, 0, 255), 2.0),
            fill: None,
        });

        let mut canvas = MockCanvas::default();
        let mut bg = None;
        render(&state, &mut canvas, &mut bg).expect("render");

        // Expected sequence: UploadImage, DrawImage (bg), DrawRectOutline
        // (committed), DrawRectOutline (pending, normalized).
        assert_eq!(canvas.calls.len(), 4);
        assert!(matches!(canvas.calls[0], DrawCall::UploadImage));
        assert!(matches!(canvas.calls[1], DrawCall::DrawImage { .. }));

        // Committed rect: outline only (fill=None).
        match &canvas.calls[2] {
            DrawCall::DrawRectOutline { rect, stroke } => {
                assert_eq!(*rect, Rect::from_xywh(10.0, 10.0, 30.0, 30.0));
                assert_eq!(stroke.color, Color::RED);
            }
            other => panic!("committed should be outline, got {other:?}"),
        }
        // Pending rect: same primitive, normalized geometry, blue stroke.
        match &canvas.calls[3] {
            DrawCall::DrawRectOutline { rect, stroke } => {
                assert_eq!(rect.min, Point::new(20.0, 20.0));
                assert_eq!(rect.max, Point::new(80.0, 80.0));
                assert_eq!(stroke.color, Color::rgb(0, 0, 255));
            }
            other => panic!("pending should be outline, got {other:?}"),
        }
    }

    /// `render_ann` is the per-variant dispatch helper. Verify it
    /// forwards each shape to the right Canvas primitive without
    /// going through `render()`'s background-blit path.
    #[test]
    fn render_ann_dispatches_each_variant() {
        let mut canvas = MockCanvas::default();
        let stroke = Stroke::new(Color::RED, 2.0);

        render_ann(
            &mut canvas,
            &Annotation::Line {
                a: Point::new(0.0, 0.0),
                b: Point::new(10.0, 10.0),
                stroke,
            },
        );
        assert!(matches!(
            canvas.calls.last(),
            Some(DrawCall::DrawLine { .. })
        ));

        render_ann(
            &mut canvas,
            &Annotation::Arrow {
                a: Point::new(0.0, 0.0),
                b: Point::new(10.0, 10.0),
                stroke,
                head_size: 8.0,
            },
        );
        assert!(matches!(
            canvas.calls.last(),
            Some(DrawCall::DrawArrow { .. })
        ));

        render_ann(
            &mut canvas,
            &Annotation::Ellipse {
                rect: Rect::from_xywh(0.0, 0.0, 10.0, 10.0),
                stroke,
                fill: None,
            },
        );
        assert!(matches!(
            canvas.calls.last(),
            Some(DrawCall::DrawEllipse { .. })
        ));

        render_ann(
            &mut canvas,
            &Annotation::Text {
                pos: Point::new(5.0, 5.0),
                text: "hi".to_owned(),
                font_size: 12.0,
                color: Color::RED,
            },
        );
        match canvas.calls.last() {
            Some(DrawCall::DrawText { text, .. }) => assert_eq!(text, "hi"),
            other => panic!("expected DrawText, got {other:?}"),
        }
    }

    /// Pending marker renders as a polyline of consecutive `draw_line`
    /// calls — one per segment.
    #[test]
    fn render_pending_marker_emits_segment_lines() {
        let mut canvas = MockCanvas::default();
        render_ann(
            &mut canvas,
            &Annotation::Marker {
                points: vec![
                    Point::new(0.0, 0.0),
                    Point::new(5.0, 5.0),
                    Point::new(10.0, 0.0),
                ],
                color: Color::RED,
                width: 4.0,
            },
        );
        // 3 points → 2 segments → 2 draw_line calls.
        let line_count = canvas
            .calls
            .iter()
            .filter(|c| matches!(c, DrawCall::DrawLine { .. }))
            .count();
        assert_eq!(line_count, 2);
    }
}
