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

use std::collections::{HashMap, HashSet};

use crate::{
    editor::{Annotation, BlurKind, EditorState},
    freehand::smooth_polyline,
    geom::{Color, Point, Rect, Stroke},
    text::{TextMetrics, TextStyle},
};

/// Catmull-Rom tension applied to freehand strokes at render time. The
/// default (0.5) is the classic Catmull-Rom spline; lower flattens,
/// higher exaggerates. Applied identically to pending and committed
/// polylines so WYSIWYG holds (ADR-0022).
const FREEHAND_TENSION: f32 = 0.5;

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

    /// Blit a raw [`image::RgbaImage`] into `dest` (physical px).
    ///
    /// Unlike [`draw_image`](Self::draw_image), this takes raw pixels
    /// rather than a pre-uploaded handle. Used by the blur render path
    /// (Task 13) to emit a freshly-computed sub-image without a
    /// separate upload / cache step: the toolkit uploads internally
    /// (egui) or copies pixels directly (`ImageCanvas`).
    fn draw_image_region(&mut self, img: &image::RgbaImage, dest: Rect);

    /// Outline a rectangle with `stroke`.
    fn draw_rect_outline(&mut self, rect: Rect, stroke: &Stroke);

    /// Fill a rectangle with a solid `color`.
    fn draw_rect_filled(&mut self, rect: Rect, color: Color);

    /// Outline and (optionally) fill an ellipse inscribed in `rect`.
    fn draw_ellipse(&mut self, rect: Rect, stroke: &Stroke, fill: Option<Color>);

    /// Stroke a line segment from `a` to `b`.
    fn draw_line(&mut self, a: Point, b: Point, stroke: &Stroke);

    /// Stroke a connected polyline through `points` as a single stroke.
    ///
    /// Used by the freehand tools (Marker / Pencil). Backends that
    /// composite per-pixel (the [`ImageCanvas`] save path) MUST blend
    /// each covered pixel at most once across the whole polyline so a
    /// translucent marker stays uniformly translucent — re-stamping
    /// overlapping segment caps would compound the alpha toward opaque
    /// (ADR-0022). GPU backends (egui) can rely on their tessellator's
    /// single-coverage path. The default impl loops [`draw_line`] per
    /// segment, which is correct for opaque strokes but compounds for
    /// translucent ones; backends that render freehand strokes should
    /// override it.
    fn draw_polyline(&mut self, points: &[Point], stroke: &Stroke) {
        for w in points.windows(2) {
            self.draw_line(w[0], w[1], stroke);
        }
    }

    /// Stroke an arrow from `a` (tail) to `b` (head), with barbs of `head_size`.
    fn draw_arrow(&mut self, a: Point, b: Point, stroke: &Stroke, head_size: f32);

    /// Draw a text run starting at `pos` (top-left anchor), styled by
    /// `style`. Shaping, multiline layout, and any outline / background
    /// are the implementer's concern; core only specifies the run.
    /// `style` is a bundle (not a growing positional list) so new text
    /// properties don't widen this signature.
    fn draw_text(&mut self, pos: Point, text: &str, style: &TextStyle);

    /// Layout measurements of `text` under `style`, in physical px. Used
    /// by the shared render path to place the caret / bounding box for
    /// an in-flight text draft ([`EditorState::pending`]).
    /// Implementations must use the same font metrics as their
    /// [`draw_text`](Self::draw_text) so the caret lands at the visible
    /// end of the typed run.
    ///
    /// For an empty run the widths are `0.0` (caret sits at `pos.x`)
    /// and `height` is one line height (`size * line_spacing`).
    #[must_use]
    fn measure_text(&self, text: &str, style: &TextStyle) -> TextMetrics;
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
/// Committed `Annotation::Blur` entries are cached in `blur_cache` so
/// the expensive Gaussian / pixelate pass runs once per annotation,
/// not once per frame. Without this, a few blur regions saturate the
/// CPU at 60 fps and the editor freezes.
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
    blur_cache: &mut BlurCache,
) -> crate::Result<()> {
    if bg.is_none() {
        let handle = canvas.upload_image(&state.image)?;
        *bg = Some(handle);
    }
    let rect = image_rect(&state.image);
    if let Some(handle) = bg.as_ref() {
        canvas.draw_image(handle, rect);
    }

    // Committed annotations. Blur annotations are special-cased to use
    // the blur cache — without it, every committed blur recomputes its
    // Gaussian / pixelate pass every frame, which saturates the CPU
    // after a few regions and freezes the editor.
    blur_cache.prune(state.anns.len());
    for (i, ann) in state.anns.iter().enumerate() {
        match ann {
            Annotation::Blur {
                rect,
                kind,
                strength,
            } => {
                let n = rect.normalized();
                if n.width().abs() < 1.0 || n.height().abs() < 1.0 {
                    continue;
                }
                let img = blur_cache.get_or_compute(i, *rect, *kind, *strength, &state.image);
                canvas.draw_image_region(img, n);
            }
            _ => render_ann(canvas, ann, &state.image),
        }
    }

    if let Some(pending) = &state.pending {
        render_ann(canvas, pending, &state.image);
        // Text indicators (bounding box + block caret) for an in-flight
        // `Tool::Text` draft. Only drawn for `pending` (never for
        // committed `anns`), so the saved/copied output via
        // `compose_final` — which doesn't go through this path — never
        // sees them. Without these the user has no feedback where the
        // typed text will land, because `draw_text` with an empty string
        // (or any string between keystrokes) draws nothing.
        if let Annotation::Text { pos, text, .. } = pending {
            draw_text_indicators(canvas, *pos, text, state.text_cursor, &pending.text_style());
        }
    }

    // Blur outlines: live-view-only affordance so the user can locate
    // blur regions even when the underlying pixels are uniform. Not
    // drawn by `render_ann`, so `compose_final` output excludes them.
    for ann in &state.anns {
        if let Annotation::Blur { rect, .. } = ann {
            draw_blur_outline(canvas, *rect);
        }
    }
    if let Some(Annotation::Blur { rect, .. }) = &state.pending {
        draw_blur_outline(canvas, *rect);
    }

    if let Some(sel) = state.crop_region {
        let n = sel.normalized();
        if n.width().abs() >= 1.0 && n.height().abs() >= 1.0 {
            canvas.draw_rect_outline(n, &SELECTION_STROKE);
        }
    }

    if let Some(m) = state.marquee {
        let n = m.normalized();
        if n.width().abs() >= 1.0 && n.height().abs() >= 1.0 {
            draw_dashed_rect(canvas, n, MARQUEE_STROKE);
        }
    }
    Ok(())
}

/// Draw an axis-aligned rect as a marching-ants dashed outline:
/// each edge dashed independently (edges start on a dash; corners
/// may end on a gap). Shared rhythm with the shells' hover chrome.
fn draw_dashed_rect<C: Canvas>(canvas: &mut C, rect: Rect, stroke: Stroke) {
    for (a, b) in [
        (rect.min, Point::new(rect.max.x, rect.min.y)),
        (
            Point::new(rect.max.x, rect.min.y),
            Point::new(rect.max.x, rect.max.y),
        ),
        (rect.max, Point::new(rect.min.x, rect.max.y)),
        (Point::new(rect.min.x, rect.max.y), rect.min),
    ] {
        let length = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
        let (dash, gap) = (6.0, 4.0);
        let step = dash + gap;
        let mut travelled = 0.0;
        while travelled < length {
            let t0 = travelled;
            let t1 = (travelled + dash).min(length);
            let f = |t: f32| {
                Point::new(
                    a.x + (b.x - a.x) * t / length,
                    a.y + (b.y - a.y) * t / length,
                )
            };
            canvas.draw_line(f(t0), f(t1), &stroke);
            travelled += step;
        }
    }
}

/// Draw a thin outline around a blur region. No-op on degenerate rects
/// (< 1 px). See [`BLUR_OUTLINE_STROKE`] for styling rationale.
fn draw_blur_outline<C: Canvas>(canvas: &mut C, rect: Rect) {
    let n = rect.normalized();
    if n.width().abs() >= 1.0 && n.height().abs() >= 1.0 {
        canvas.draw_rect_outline(n, &BLUR_OUTLINE_STROKE);
    }
}

/// Stroke style for the in-flight `Tool::Crop` region. Distinct from
/// any annotation color so the user can tell the crop rectangle
/// apart from a committed `Annotation::Rect`. Blue is picked to match
/// conventional region-selection UX in screenshot tools.
const SELECTION_STROKE: Stroke = Stroke::new(Color::rgb(0, 120, 215), 2.0);

/// Stroke style for the in-flight `Tool::Select` marquee (the rubber
/// band). Same blue family as [`SELECTION_STROKE`] (the Crop region
/// outline) but dashed, so the two live drags read apart at a glance.
const MARQUEE_STROKE: Stroke = Stroke::new(Color::rgb(0, 120, 215), 1.5);

/// Outline drawn around the in-flight `Tool::Text` block so the user
/// can see the text's extent. Semi-transparent mid-grey reads on both
/// bright and dark screenshots without competing with the text colour.
/// Live-view-only — `render_ann` doesn't draw it, so saved / copied
/// output via `compose_final` excludes it.
const TEXT_BBOX_STROKE: Stroke = Stroke::new(Color::rgba(60, 60, 60, 170), 1.0);

/// Padding around the laid-out text block for the bounding-box
/// indicator, in physical px. Keeps the box from hugging glyph edges
/// (ascenders / descenders would otherwise touch the outline).
const TEXT_BBOX_PAD: f32 = 2.0;

/// Width of the solid block caret, as a fraction of font size and
/// clamped so it stays visible at small sizes. Terminal-style: a filled
/// rectangle the user reads as the insertion point.
fn block_caret_width(size: f32) -> f32 {
    (size * 0.12).max(2.0)
}

/// Outline drawn around blur regions in the live editor view so the
/// user can locate them even when the underlying pixels are uniform
/// (Gaussian blur of a flat colour is indistinguishable from the
/// original). Semi-transparent white is visible on both dark and light
/// backgrounds. This is a live-view-only affordance — `render_ann`
/// does not draw it, so saved / copied output via `compose_final`
/// excludes it.
const BLUR_OUTLINE_STROKE: Stroke = Stroke::new(Color::rgba(255, 255, 255, 180), 1.5);

/// Dispatch a single annotation to the appropriate `Canvas` primitive(s).
///
/// `source` is the background screenshot (`state.image`); it's only
/// consulted by [`Annotation::Blur`], which crops the region from it,
/// runs [`crate::image_ops::blur`], and blits the result. Every other
/// variant ignores `source` and renders from its own fields.
///
/// Public so future tool-specific render paths can reuse the per-variant
/// mapping. Pending annotations call this same path; the only difference
/// is whether they live in `EditorState::anns` or `EditorState::pending`.
pub fn render_ann<C: Canvas>(canvas: &mut C, ann: &Annotation, source: &image::RgbaImage) {
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
            // GOTCHA: smoothing re-runs every frame on the pending
            // preview. Catmull-Rom is O(n) in the raw sample count and
            // the per-span subdivision is bounded, so this is negligible
            // for typical freehand strokes; revisit only if very long
            // strokes show up in profiles. Applied here (not at update
            // / commit) so the pending preview and the committed form
            // pass through the identical smoothing pass and WYSIWYG
            // holds between the live editor and saved/copied output
            // (ADR-0022).
            let smoothed = smooth_polyline(points, FREEHAND_TENSION);
            canvas.draw_polyline(&smoothed, &stroke);
        }
        Annotation::Text { pos, text, .. } => {
            canvas.draw_text(*pos, text, &ann.text_style());
        }
        Annotation::Blur {
            rect,
            kind,
            strength,
            ..
        } => {
            // Computed in core (ADR: blur/pixelate lives here, not in
            // the shell). Crop → blur → blit; both the live egui view
            // and compose_final's ImageCanvas hit this same path so
            // WYSIWYG holds (ADR-0015).
            let n = rect.normalized();
            // Skip degenerate rects. `crop_region` returns a 0×0
            // `RgbaImage` for zero-area input, and uploading a
            // zero-dimension texture panics wgpu ("Dimension X is
            // zero"). The pending blur hits this on the first frame
            // of every drag — `begin_annotation` seeds the rect as
            // `Rect::new(at, at)` (zero area) before the user moves
            // the pointer. The < 1 px threshold matches the
            // degenerate-cancel contract in `EditorState::finish_pointer_up`
            // and the selection-rect guard in `render()` below.
            if n.width().abs() < 1.0 || n.height().abs() < 1.0 {
                return;
            }
            let region = crate::image_ops::crop_region(source, n);
            let blurred = crate::image_ops::blur(&region, *kind, *strength);
            canvas.draw_image_region(&blurred, n);
        }
        Annotation::Number {
            pos,
            n,
            color,
            diameter,
        } => {
            // Filled circular bubble: a circle is an ellipse inscribed
            // in a square bounding rect, so equal width/height +
            // `draw_ellipse` with `Some(fill)` reuses the Task 001 impl
            // verbatim. A 0-width stroke means no outline band — the
            // bubble reads as a solid disc (the bubble's "edge" is
            // just the fill boundary). Replaces the previous
            // `draw_rect_filled` square placeholder.
            let half = *diameter / 2.0;
            let bbox = Rect::from_xywh(pos.x - half, pos.y - half, *diameter, *diameter);
            canvas.draw_ellipse(bbox, &Stroke::ZERO, Some(*color));
            let label = n.to_string();
            // Contrast-aware digit colour: white on dark fills, black
            // on light. Picked by Rec. 601 luminance — see
            // `contrast_text_color`.
            let text = TextStyle::simple(*diameter * 0.6, contrast_text_color(*color));
            // Centre the digit: `pos` is the bubble's centre, but the
            // `Canvas` contract anchors a text run's top-left at the
            // draw position — drawing at `pos` would sit the digit
            // toward the bubble's bottom-right. Measure the laid-out
            // label and offset by half its size so the glyph block is
            // centred in the disc. Fixed here, in the shared path, so
            // `EguiCanvas` and `ImageCanvas` center together (WYSIWYG),
            // each by its own font metrics.
            let m = canvas.measure_text(&label, &text);
            let anchor = Point::new(pos.x - m.width / 2.0, pos.y - m.height / 2.0);
            canvas.draw_text(anchor, &label, &text);
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

/// Luminance threshold for [`contrast_text_color`]. Colours with
/// luminance below this get white text; at-or-above get black.
///
/// Rec. 601 luminance (`Y = 0.299R + 0.587G + 0.114B`, range 0–255)
/// approximates perceived brightness closely enough for a binary
/// black-or-white text pick. The 128 mid-point is the conventional
/// threshold used by GitHub, Bootstrap, and most contrast-pick
/// helpers; common annotation colours (red, blue, yellow, white,
/// black) all land clearly on one side. WCAG's relative-luminance
/// formula would be more principled but requires gamma-decoding each
/// channel; for a binary choice the linear approximation is close
/// enough and cheaper.
const CONTRAST_LUMINANCE_THRESHOLD: f32 = 128.0;

/// Pick a text colour that contrasts with `bg`: white on dark fills,
/// black on light. Used by the Number tool's bubble label so the digit
/// is readable regardless of the user's chosen bubble colour.
///
/// See [`CONTRAST_LUMINANCE_THRESHOLD`] for the formula and threshold
/// rationale.
#[must_use]
fn contrast_text_color(bg: Color) -> Color {
    let y = 0.299 * f32::from(bg.r) + 0.587 * f32::from(bg.g) + 0.114 * f32::from(bg.b);
    if y < CONTRAST_LUMINANCE_THRESHOLD {
        Color::rgb(255, 255, 255)
    } else {
        Color::rgb(0, 0, 0)
    }
}

/// cos(30°) — barb half-angle from the reversed shaft direction.
const ARROW_BARB_COS30: f32 = 0.866_025_4;
/// sin(30°).
const ARROW_BARB_SIN30: f32 = 0.5;

/// Compute the two barb endpoints for an arrow whose shaft runs from
/// `a` (tail) to `b` (head). Each barb is `head_size` long and angled
/// 30° off the reversed shaft direction — the conventional "V" shape.
///
/// Shared between [`Canvas`] implementations so the live editor view
/// (egui) and the saved / copied image ([`ImageCanvas`] /
/// [`compose_final`]) agree pixel-for-pixel. Per ADR-0015 the two
/// render paths must produce identical output; centralising the barb
/// geometry here is what enforces that for arrows.
///
/// Returns `[Point { x: b.x, y: b.y }; 2]` (a degenerate "both barbs
/// at the head" pair) when the shaft is shorter than 1 px — callers
/// naturally skip drawing in that case because the shaft itself is
/// invisible.
#[must_use]
pub fn arrow_barbs(a: Point, b: Point, head_size: f32) -> [Point; 2] {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1.0 {
        return [b, b];
    }
    let len = len_sq.sqrt();
    let ux = dx / len;
    let uy = dy / len;
    // Reverse direction (-ux, -uy) rotated by ±30° via the standard
    // 2D rotation matrix R(θ) · (x, y) = (x·cosθ − y·sinθ, x·sinθ + y·cosθ).
    let left_x = -ux * ARROW_BARB_COS30 + uy * ARROW_BARB_SIN30;
    let left_y = -ux * ARROW_BARB_SIN30 - uy * ARROW_BARB_COS30;
    let right_x = -ux * ARROW_BARB_COS30 - uy * ARROW_BARB_SIN30;
    let right_y = ux * ARROW_BARB_SIN30 - uy * ARROW_BARB_COS30;
    [
        Point::new(b.x + head_size * left_x, b.y + head_size * left_y),
        Point::new(b.x + head_size * right_x, b.y + head_size * right_y),
    ]
}

/// Draw the in-flight text indicators: a hollow bounding box around
/// the laid-out text block and a solid block caret at the insertion
/// point (tracked by `cursor`, a char index into `text`).
///
/// Both are live-view-only — drawn solely for `pending`
/// `Annotation::Text` via [`render`]; the saved / copied image excludes
/// `pending`, so neither reaches disk or the clipboard. Without these
/// the user has no feedback where typed text will land, because
/// `draw_text` with an empty string (or between keystrokes) draws
/// nothing.
///
/// The caret's line and horizontal offset are derived from `cursor`:
/// the char index maps to a (line, column) pair, and `measure_text`
/// sizes the prefix on that line so the caret lands at the right spot
/// even after a `Shift+Enter` moved it onto a new line, or after an
/// arrow key moved it mid-string. Alignment shifts the whole line block
/// so a centred / right-aligned caret still tracks its glyph.
#[allow(clippy::cast_precision_loss, reason = "line count is tiny")]
fn draw_text_indicators<C: Canvas>(
    canvas: &mut C,
    pos: Point,
    text: &str,
    cursor: usize,
    style: &TextStyle,
) {
    let metrics = canvas.measure_text(text, style);
    let pad = TEXT_BBOX_PAD;
    let block_w = block_caret_width(style.size);
    // Caret offset from `pos` (text origin) and 0-based line index.
    let (caret_dx, caret_line) = caret_coords(text, cursor, canvas, style);
    let line_h = style.size * style.line_spacing;
    // Bounding box: spans the widest line, extended to cover the caret
    // (which sits just past the cursor's column on its line).
    let bbox_w = metrics.width.max(caret_dx + block_w);
    let bbox = Rect::from_xywh(
        pos.x - pad,
        pos.y - pad,
        bbox_w + pad * 2.0,
        metrics.height + pad * 2.0,
    );
    canvas.draw_rect_outline(bbox, &TEXT_BBOX_STROKE);
    // Solid block caret at the insertion point.
    let caret = Rect::from_xywh(
        pos.x + caret_dx,
        pos.y + caret_line as f32 * line_h,
        block_w,
        style.size,
    );
    canvas.draw_rect_filled(caret, style.color);
}

/// Resolve the caret's horizontal offset from the text origin and its
/// line index (0-based) from a char-indexed `cursor` into `text`.
///
/// Splits `text` at the cursor: everything before determines the line
/// and the prefix-on-this-line; everything after extends to the line's
/// end. `measure_text` sizes both so the caret's x accounts for the
/// line's alignment offset (centred / right lines shift as a block).
fn caret_coords<C: Canvas>(
    text: &str,
    cursor: usize,
    canvas: &mut C,
    style: &TextStyle,
) -> (f32, usize) {
    let before: String = text.chars().take(cursor).collect();
    let after: String = text.chars().skip(cursor).collect();
    let caret_line = before.matches('\n').count();
    // Prefix on the current line = text after the last '\n' in `before`.
    let prefix = before.rsplit('\n').next().unwrap_or(&before);
    // Rest of the current line up to the next '\n' (or end).
    let rest = after.split('\n').next().unwrap_or("");
    let prefix_w = canvas.measure_text(prefix, style).width;
    // The full current line's width, needed to apply alignment.
    let mut full_line = String::with_capacity(prefix.len() + rest.len());
    full_line.push_str(prefix);
    full_line.push_str(rest);
    let line_w = canvas.measure_text(&full_line, style).width;
    let max_w = canvas.measure_text(text, style).width;
    let align_offset = match style.align {
        crate::text::TextAlign::Left => 0.0,
        crate::text::TextAlign::Center => (max_w - line_w) / 2.0,
        crate::text::TextAlign::Right => max_w - line_w,
    };
    (prefix_w + align_offset, caret_line)
}

/// Cache of precomputed blurred images for committed `Annotation::Blur`
/// entries, keyed by annotation index.
///
/// Without this cache, [`render`] recomputes the Gaussian / pixelate
/// pass for every committed blur on every frame (60 fps). A few large
/// blur regions saturate the CPU and the editor freezes. The cache
/// computes each blur once and reuses the result across frames.
///
/// The shell owns the cache and passes it to [`render`] each frame.
/// It must call [`BlurCache::clear`] when the source image changes
/// (crop replaces `state.image`) so stale entries don't survive.
///
/// Cache invalidation is parameter-based: each entry stores the
/// `(rect, kind, strength)` it was computed with. On lookup, if the
/// annotation's params differ (e.g. after undo / redo / crop changed
/// the rect), the entry is recomputed. Annotation index is the primary
/// key, so adding new annotations doesn't invalidate existing entries.
pub struct BlurCache {
    entries: HashMap<usize, BlurCacheEntry>,
}

struct BlurCacheEntry {
    rect: Rect,
    kind: BlurKind,
    strength: u8,
    image: image::RgbaImage,
}

impl BlurCache {
    /// Construct an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Drop all cached entries. Call when the source image changes
    /// (crop) so stale precomputed blurs don't survive.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Remove entries whose annotation index is ≥ `len`. Called each
    /// frame so undo (which shrinks `anns`) doesn't leak memory.
    fn prune(&mut self, len: usize) {
        self.entries.retain(|&k, _| k < len);
    }

    /// Return the cached blurred image for annotation `index`, or
    /// compute and cache it on miss. Parameter check on every hit means
    /// undo / redo / crop (which change annotation rects or the source
    /// image) naturally invalidate stale entries.
    fn get_or_compute(
        &mut self,
        index: usize,
        rect: Rect,
        kind: BlurKind,
        strength: u8,
        source: &image::RgbaImage,
    ) -> &image::RgbaImage {
        let needs_compute = self
            .entries
            .get(&index)
            .is_none_or(|e| e.rect != rect || e.kind != kind || e.strength != strength);
        if needs_compute {
            let n = rect.normalized();
            let region = crate::image_ops::crop_region(source, n);
            let blurred = crate::image_ops::blur(&region, kind, strength);
            self.entries.insert(
                index,
                BlurCacheEntry {
                    rect,
                    kind,
                    strength,
                    image: blurred,
                },
            );
        }
        &self
            .entries
            .get(&index)
            .expect("entry was just inserted")
            .image
    }
}

impl Default for BlurCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Compose the final image: a clone of `state.image` with every
/// committed annotation rendered into it. Excludes `state.pending`
/// (in-flight annotations are not part of the saved output — the
/// shell commits or cancels the pending annotation before invoking
/// this path).
///
/// This is the source of truth for Save / Copy / Upload outputs: it
/// goes through the same per-variant dispatch ([`render_ann`]) as the
/// live editor, so what the user sees in the editor window matches
/// what gets written to disk or the clipboard. No toolkit-specific
/// "screenshot the canvas" path is involved.
///
/// # Implementation note
///
/// Drawing happens into a private [`ImageCanvas`] that implements
/// [`Canvas`] on top of an [`image::RgbaImage`]. Phase 1 Task 9 filled
/// in the rect primitives; Task 11 added line + arrow. Phase 2b added
/// `draw_ellipse`. Text was completed in Phase 2.
#[must_use]
pub fn compose_final(state: &EditorState) -> image::RgbaImage {
    let mut canvas = ImageCanvas::new(state.image.clone());
    for ann in &state.anns {
        render_ann(&mut canvas, ann, &state.image);
    }
    canvas.into_inner()
}

/// Compose the final image (background + annotations), then crop to
/// `rect`. The source of truth for selection-aware outputs: Copy with
/// an active `Tool::Crop` region emits just the region's pixels,
/// with annotations baked in (clipped at the region boundary by the
/// canvas's pixel-level bounds).
///
/// Pure / non-mutating: `state.image`, `state.anns`, and
/// `state.crop_region` are untouched. This is the key difference from
/// [`EditorState::crop`](crate::EditorState::crop), which *replaces*
/// `state.image` with the cropped sub-image. Use `compose_final_region`
/// when the caller wants a one-shot cropped buffer (e.g. placing bytes
/// on the clipboard without disturbing the editor); use
/// `EditorState::crop` when the user wants to permanently narrow the
/// editor to the selection.
///
/// Returns a 0×0 image when `rect` is degenerate (< 1 px in either
/// dimension) or falls entirely outside the image (see
/// [`crop_region`](crate::image_ops::crop_region)). Callers that need
/// a non-empty result should guard against degenerate rects before
/// calling, typically with the same `width/height >= 1.0` check the
/// render path uses for the in-flight crop-region outline.
#[must_use]
pub fn compose_final_region(state: &EditorState, rect: Rect) -> image::RgbaImage {
    let full = compose_final(state);
    crate::image_ops::crop_region(&full, rect)
}

/// Headless [`Canvas`] that draws into an [`image::RgbaImage`]. Used
/// by [`compose_final`] to render annotations into the saved / copied
/// output without going through a GUI toolkit.
///
/// `Image = ()` because the canvas *is* the image: `upload_image` and
/// `draw_image` are no-ops (the image data is already in place at
/// construction time). Only the per-annotation primitives actually
/// mutate pixels.
struct ImageCanvas {
    image: image::RgbaImage,
}

impl ImageCanvas {
    /// Wrap an image for annotation rendering. The canvas takes
    /// ownership; use [`ImageCanvas::into_inner`] to recover it.
    fn new(image: image::RgbaImage) -> Self {
        Self { image }
    }

    /// Recover the underlying image.
    fn into_inner(self) -> image::RgbaImage {
        self.image
    }
}

impl Canvas for ImageCanvas {
    type Image = ();

    fn upload_image(&mut self, _img: &image::RgbaImage) -> crate::Result<Self::Image> {
        // No-op: the canvas already *is* the image. The texture-handle
        // concept doesn't apply to direct pixel manipulation.
        Ok(())
    }

    fn draw_image(&mut self, _img: &Self::Image, _rect: Rect) {
        // No-op: the background image is already in place as the
        // canvas's starting content (compose_final clones state.image
        // at construction). No separate blit step.
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "coords clamped and rounded before cast; dims ≤ 2^24"
    )]
    fn draw_image_region(&mut self, img: &image::RgbaImage, dest: Rect) {
        // Copy the source sub-image into the canvas at `dest`,
        // clamped to canvas bounds. Used by the blur render path to
        // overwrite a region with its blurred version.
        let n = dest.normalized();
        let (cw, ch) = img.dimensions();
        let (w_f, h_f) = (self.image.width() as f32, self.image.height() as f32);
        let x0 = n.min.x.round().clamp(0.0, w_f) as u32;
        let y0 = n.min.y.round().clamp(0.0, h_f) as u32;
        let (canvas_w, canvas_h) = self.image.dimensions();
        for dy in 0..ch {
            for dx in 0..cw {
                let px = x0.saturating_add(dx);
                let py = y0.saturating_add(dy);
                if px < canvas_w && py < canvas_h {
                    self.image.put_pixel(px, py, *img.get_pixel(dx, dy));
                }
            }
        }
    }

    fn draw_rect_outline(&mut self, rect: Rect, stroke: &Stroke) {
        // StrokeKind::Inside, matching `EguiCanvas::draw_rect_outline`:
        // the stroke band sits inside the dragged rect, so the outer
        // edge coincides with the rect geometry — "what you dragged is
        // what you get". Four filled rectangles (one per edge) overlap
        // at the corners; the overlap is harmless because we're
        // painting a single opaque colour.
        let n = rect.normalized();
        let w = stroke.width;
        if w <= 0.0 || n.width() <= 0.0 || n.height() <= 0.0 {
            return;
        }
        fill_rect(
            &mut self.image,
            n.min.x,
            n.min.y,
            n.max.x,
            n.min.y + w,
            stroke.color,
        );
        fill_rect(
            &mut self.image,
            n.min.x,
            n.max.y - w,
            n.max.x,
            n.max.y,
            stroke.color,
        );
        fill_rect(
            &mut self.image,
            n.min.x,
            n.min.y,
            n.min.x + w,
            n.max.y,
            stroke.color,
        );
        fill_rect(
            &mut self.image,
            n.max.x - w,
            n.min.y,
            n.max.x,
            n.max.y,
            stroke.color,
        );
    }

    fn draw_rect_filled(&mut self, rect: Rect, color: Color) {
        let n = rect.normalized();
        if n.width() <= 0.0 || n.height() <= 0.0 {
            return;
        }
        fill_rect(&mut self.image, n.min.x, n.min.y, n.max.x, n.max.y, color);
    }

    fn draw_ellipse(&mut self, rect: Rect, stroke: &Stroke, fill: Option<Color>) {
        paint_ellipse(&mut self.image, rect, *stroke, fill);
    }

    fn draw_line(&mut self, a: Point, b: Point, stroke: &Stroke) {
        stroke_segment(&mut self.image, a, b, *stroke);
    }

    fn draw_polyline(&mut self, points: &[Point], stroke: &Stroke) {
        stamp_polyline(&mut self.image, points, *stroke);
    }

    fn draw_arrow(&mut self, a: Point, b: Point, stroke: &Stroke, head_size: f32) {
        stroke_segment(&mut self.image, a, b, *stroke);
        let [left, right] = arrow_barbs(a, b, head_size);
        stroke_segment(&mut self.image, b, left, *stroke);
        stroke_segment(&mut self.image, b, right, *stroke);
    }

    fn draw_text(&mut self, pos: Point, text: &str, style: &TextStyle) {
        // Background highlight: a solid filled rect behind the whole
        // laid-out block (matches the egui view, which draws the same
        // rect via `painter.rect_filled`).
        if let Some(bg) = style.background {
            let m = self.measure_text(text, style);
            fill_rect(
                &mut self.image,
                pos.x,
                pos.y,
                pos.x + m.width,
                pos.y + m.height,
                bg,
            );
        }
        let Some(font) = font_for(style.family, style.weight, style.style) else {
            tracing::warn!(
                target: "flintshot::render::image_canvas",
                pos = ?pos, text,
                "no system font available; text annotation omitted from saved/copied output"
            );
            return;
        };
        // Outline: redraw the run at the four cardinal offsets in the
        // outline colour, then the fill pass on top centres the
        // glyphs. Mirrors the egui view's faux-outline approach so the
        // two paths stay WYSIWYG without a separate stroke rasteriser.
        if let Some(outline) = style.outline {
            let w = outline.width;
            let outlined = crate::text::TextStyle {
                color: outline.color,
                ..*style
            };
            for (dx, dy) in [(w, 0.0), (-w, 0.0), (0.0, w), (0.0, -w)] {
                rasterize_text(
                    &mut self.image,
                    Point::new(pos.x + dx, pos.y + dy),
                    text,
                    &outlined,
                    &font,
                );
            }
        }
        rasterize_text(&mut self.image, pos, text, style, &font);
    }

    /// Mirror `rasterize_text`'s horizontal advance using the same
    /// resolved font + scale. Kept in lockstep so the live editor view
    /// (egui) and the saved/copied image agree on caret placement for
    /// the brief moment before commit; `compose_final` doesn't call
    /// this since `pending` is excluded from saved output.
    fn measure_text(&self, text: &str, style: &TextStyle) -> TextMetrics {
        use ab_glyph::{Font, ScaleFont};
        let Some(font) = font_for(style.family, style.weight, style.style) else {
            return TextMetrics::default();
        };
        let scaled = font.as_scaled(ab_glyph::PxScale::from(style.size));
        let line_width = |line: &str| -> f32 {
            line.chars()
                .map(|ch| scaled.h_advance(scaled.glyph_id(ch)))
                .sum()
        };
        // `split('\n')` yields one entry for a single-line run, so the
        // common case keeps `width == last_line_width`.
        let mut width = 0.0_f32;
        let mut last_line_width = 0.0_f32;
        let mut line_count = 0_usize;
        for line in text.split('\n') {
            let w = line_width(line);
            width = width.max(w);
            last_line_width = w;
            line_count += 1;
        }
        #[allow(
            clippy::cast_precision_loss,
            reason = "line count is bounded by the draft length; ≤ 2^24, exact in f32"
        )]
        let height = line_count as f32 * style.size * style.line_spacing;
        TextMetrics {
            width,
            height,
            last_line_width,
        }
    }
}

/// Stroke a thick line segment from `a` to `b` by stamping a filled
/// square of side `stroke.width` (centred on the line) at ~1-px
/// intervals along the segment.
///
/// This is the [`ImageCanvas`] equivalent of egui's `Painter::line_segment`:
/// a connected, opaque stroke band. Stamping a square per sample
/// approximates a `StrokeKind::Inside`, round-cap stroke closely
/// enough that saved output matches the on-screen arrow / line.
/// Phase 1 has no anti-aliasing in the save path; that's acceptable
/// for a screenshot annotation tool (the source pixels are opaque, and
/// the editor view itself is AA'd by egui so the user sees a clean
/// preview).
///
/// Sub-pixel segments (`len < 0.5`) draw nothing — the caller (e.g.
/// [`ImageCanvas::draw_arrow`]) is responsible for deciding whether a
/// single-pixel dot is appropriate, which it does via [`arrow_barbs`]'s
/// degenerate-return contract.
fn stroke_segment(img: &mut image::RgbaImage, a: Point, b: Point, stroke: Stroke) {
    let w = stroke.width;
    if w <= 0.0 {
        return;
    }
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 0.25 {
        return;
    }
    let len = len_sq.sqrt();
    // One stamp per pixel of length guarantees a connected band: the
    // gap between consecutive centrerpoints is ≤ 1 px, and each stamp
    // covers ±(width/2) around its centre, so adjacent stamps overlap
    // for any width ≥ 1. ceil() ensures even a 0.6-px segment gets one
    // step; the +1 includes the `b` endpoint.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "len ≥ 0.5 (filtered), ≤ image diagonal ≤ 2^24"
    )]
    let steps = len.ceil() as usize;
    let half = w / 2.0;
    for i in 0..=steps {
        #[allow(
            clippy::cast_precision_loss,
            reason = "loop index ≤ steps+1 ≤ 2^24, exact in f32"
        )]
        let t = i as f32 / steps as f32;
        let cx = a.x + dx * t;
        let cy = a.y + dy * t;
        fill_rect(
            img,
            cx - half,
            cy - half,
            cx + half,
            cy + half,
            stroke.color,
        );
    }
}

/// Stamp a whole polyline as one stroke, blending each covered pixel
/// exactly once.
///
/// This is the [`ImageCanvas`] freehand path used by Marker / Pencil.
/// It computes the union of the per-segment stamp squares (the same
/// coverage [`stroke_segment`] would produce) into a set, then
/// source-over blends the stroke colour into each covered pixel once.
/// Without the union step the overlapping stamps at every smoothed
/// joint would re-blend the same pixel ~`width` times, compounding a
/// translucent marker's alpha toward opaque and breaking the "text
/// visible through it" exit criterion (ADR-0022).
///
/// For an opaque stroke (`color.a == 255`) source-over degenerates to
/// a hard replace, so Pencil output is identical to per-segment
/// stamping — only the translucent (Marker) case behaves differently,
/// and only in the direction of correctness.
fn stamp_polyline(img: &mut image::RgbaImage, points: &[Point], stroke: Stroke) {
    if points.len() < 2 || stroke.width <= 0.0 {
        return;
    }
    let mut covered: HashSet<(u32, u32)> = HashSet::new();
    let dim = img.dimensions();
    for w in points.windows(2) {
        collect_stamp_pixels(dim, w[0], w[1], stroke.width, &mut covered);
    }
    blend_stamp(img, &covered, stroke.color);
}

/// Collect the integer pixel coordinates covered by stamping a
/// `stroke.width`-side square at ~1-px steps along `a -> b` into `out`.
///
/// Mirrors [`stroke_segment`]'s loop and [`fill_rect`]'s clamp+round
/// so an opaque polyline covers the same pixels as the per-segment
/// path. Degenerate segments (`len < 0.5`) contribute nothing, again
/// matching [`stroke_segment`].
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "coords clamped to image bounds and rounded before cast; \
              len/indices ≤ image diagonal ≤ 2^24, exact in f32"
)]
fn collect_stamp_pixels(
    (img_w, img_h): (u32, u32),
    a: Point,
    b: Point,
    width: f32,
    out: &mut HashSet<(u32, u32)>,
) {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 0.25 {
        return;
    }
    let len = len_sq.sqrt();
    let steps = len.ceil() as usize;
    let half = width / 2.0;
    let max_x = img_w as f32;
    let max_y = img_h as f32;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let cx = a.x + dx * t;
        let cy = a.y + dy * t;
        let x0 = (cx - half).clamp(0.0, max_x).round() as u32;
        let x1 = (cx + half).clamp(0.0, max_x).round() as u32;
        let y0 = (cy - half).clamp(0.0, max_y).round() as u32;
        let y1 = (cy + half).clamp(0.0, max_y).round() as u32;
        for py in y0..y1 {
            for px in x0..x1 {
                out.insert((px, py));
            }
        }
    }
}

/// Source-over blend `color` into every pixel in `covered`. Each pixel
/// is written at most once, so this is the single compositing pass for
/// a whole polyline stroke. Opaque `color` collapses to a hard replace
/// (mathematically identical to [`fill_rect`]); translucent `color`
/// produces the uniform blend a marker highlight needs.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "composited values clamped to [0, 255] before the u8 cast"
)]
fn blend_stamp(img: &mut image::RgbaImage, covered: &HashSet<(u32, u32)>, color: Color) {
    let alpha = f32::from(color.a) / 255.0;
    let inv = 1.0 - alpha;
    for &(px, py) in covered {
        let bg = img.get_pixel(px, py);
        let out_r = f32::from(color.r) * alpha + f32::from(bg.0[0]) * inv;
        let out_g = f32::from(color.g) * alpha + f32::from(bg.0[1]) * inv;
        let out_b = f32::from(color.b) * alpha + f32::from(bg.0[2]) * inv;
        // Keep the background alpha: screenshots are opaque, and
        // compositing the source alpha would make the highlight itself
        // transparent (matching `blend_pixel` for text).
        img.put_pixel(
            px,
            py,
            image::Rgba([
                out_r.round().clamp(0.0, 255.0) as u8,
                out_g.round().clamp(0.0, 255.0) as u8,
                out_b.round().clamp(0.0, 255.0) as u8,
                bg.0[3],
            ]),
        );
    }
}

/// Cache of resolved system fonts keyed by family + weight + style.
///
/// `OnceLock<Mutex<HashMap<…>>>` so the fontdb scan + font read happen
/// at most once per distinct `(family, weight, style)` triple per
/// process — `compose_final` may render many text annotations, and
/// re-scanning fontconfig each time would dominate save time. Each
/// value is `Option<FontArc>` (`None` cached when fontconfig found
/// nothing for that triple, so we don't retry). Cloning a `FontArc` is
/// a cheap `Arc` bump, so callers take an owned copy per call.
static SYSTEM_FONTS: std::sync::OnceLock<
    std::sync::Mutex<HashMap<FontKey, Option<ab_glyph::FontArc>>>,
> = std::sync::OnceLock::new();

/// Cache key for [`SYSTEM_FONTS`]. The four text enums are `Copy` and
/// cheap; `Hash`/`Eq` derive from the underlying field.
#[derive(Hash, PartialEq, Eq, Clone, Copy)]
struct FontKey {
    family: crate::text::FontFamily,
    weight: crate::text::FontWeight,
    style: crate::text::FontStyle,
}

/// Resolve (and cache) a system [`ab_glyph::FontArc`] for the given
/// family / weight / style, or `None` if fontconfig found nothing.
///
/// Falls back gracefully: an unknown `FontFamily`/`FontWeight`/
/// `FontStyle` variant (the enums are `#[non_exhaustive]`) maps to the
/// sans / normal / normal default, so future variants never panic.
fn font_for(
    family: crate::text::FontFamily,
    weight: crate::text::FontWeight,
    style: crate::text::FontStyle,
) -> Option<ab_glyph::FontArc> {
    let key = FontKey {
        family,
        weight,
        style,
    };
    let cache = SYSTEM_FONTS.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let mut guard = cache.lock().expect("font cache mutex poisoned");
    if let Some(existing) = guard.get(&key) {
        return existing.clone();
    }
    let resolved = query_system_font(key);
    guard.insert(key, resolved.clone());
    resolved
}

/// One-time fontdb scan for a single triple. See [`font_for`].
fn query_system_font(key: FontKey) -> Option<ab_glyph::FontArc> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let family = match key.family {
        crate::text::FontFamily::Serif => fontdb::Family::Serif,
        crate::text::FontFamily::Monospace => fontdb::Family::Monospace,
        _ => fontdb::Family::SansSerif,
    };
    let weight = match key.weight {
        crate::text::FontWeight::Bold => fontdb::Weight::BOLD,
        _ => fontdb::Weight::NORMAL,
    };
    let style = match key.style {
        crate::text::FontStyle::Italic => fontdb::Style::Italic,
        _ => fontdb::Style::Normal,
    };
    let id = db.query(&fontdb::Query {
        families: &[family],
        weight,
        stretch: fontdb::Stretch::Normal,
        style,
    })?;
    let face = db.face(id)?;
    match &face.source {
        fontdb::Source::Binary(data) => {
            let bytes: &[u8] = (**data).as_ref();
            ab_glyph::FontArc::try_from_vec(bytes.to_vec()).ok()
        }
        fontdb::Source::File(path) => std::fs::read(path)
            .ok()
            .and_then(|bytes| ab_glyph::FontArc::try_from_vec(bytes).ok()),
        fontdb::Source::SharedFile(_path, data) => {
            let bytes: &[u8] = (**data).as_ref();
            ab_glyph::FontArc::try_from_vec(bytes.to_vec()).ok()
        }
    }
}

/// Rasterize `text` into `img` at `pos` (the top-left anchor) using
/// `font` per `style`. Each `\n` starts a new line whose baseline is
/// `size * line_spacing` below the previous one. Lines are aligned per
/// `style.align` relative to the widest line's advance. Glyphs are
/// source-over alpha-composited so the text renders correctly over any
/// background (annotation over screenshot).
///
/// Out-of-bounds glyph pixels are clipped (`ab_glyph` emits them for
/// ascenders / descenders past the image edge); this is a no-op for
/// those pixels, not an error.
///
/// Background and outline are handled by the caller ([`ImageCanvas::draw_text`])
/// so the same measurement / offset math is shared with the egui view.
#[allow(
    clippy::cast_precision_loss,
    reason = "glyph pixel offsets (gx, gy) are ≤ image dimensions ≤ 2^24, exact in f32"
)]
fn rasterize_text(
    img: &mut image::RgbaImage,
    pos: Point,
    text: &str,
    style: &TextStyle,
    font: &ab_glyph::FontArc,
) {
    use ab_glyph::{Font, ScaleFont};
    let size = style.size;
    let color = style.color;
    let scale = ab_glyph::PxScale::from(size);
    let scaled = font.as_scaled(scale);
    let line_height = size * style.line_spacing;
    let line_advance = |line: &str| -> f32 {
        line.chars()
            .map(|ch| scaled.h_advance(scaled.glyph_id(ch)))
            .sum()
    };
    // Widest line drives the alignment box; computed up front so each
    // line's left-edge offset is consistent (Left = 0, Center = half the
    // slack, Right = full slack).
    let lines: Vec<&str> = text.split('\n').collect();
    let max_w = lines
        .iter()
        .map(|l| line_advance(l))
        .fold(0.0_f32, f32::max);
    let align_offset = |line_w: f32| -> f32 {
        let slack = (max_w - line_w).max(0.0);
        match style.align {
            crate::text::TextAlign::Center => slack / 2.0,
            crate::text::TextAlign::Right => slack,
            _ => 0.0,
        }
    };
    // `pos` is the top-left; ab_glyph positions glyphs by the pen point
    // at the baseline. Lift `y` by the ascent so the first line's glyph
    // tops land at `pos.y`.
    let mut baseline_y = pos.y + scaled.ascent();
    for line in lines.iter().copied() {
        let mut pen_x = pos.x + align_offset(line_advance(line));
        for ch in line.chars() {
            let glyph_id = scaled.glyph_id(ch);
            let glyph = ab_glyph::Glyph {
                id: glyph_id,
                scale,
                position: ab_glyph::point(pen_x, baseline_y),
            };
            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                let min_x = bounds.min.x;
                let min_y = bounds.min.y;
                outlined.draw(|gx, gy, coverage| {
                    #[allow(
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss,
                        reason = "glyph px coords are non-negative and within image after the bounds check"
                    )]
                    let px = min_x + gx as f32;
                    #[allow(
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss,
                        reason = "see above"
                    )]
                    let py = min_y + gy as f32;
                    blend_pixel(img, px, py, color, coverage);
                });
            }
            pen_x += scaled.h_advance(glyph_id);
        }
        baseline_y += line_height;
    }
}

/// Source-over alpha-blend `color` into `img` at `(x, y)` with the
/// given glyph `coverage` (0.0..=1.0). Coordinates outside the image
/// are dropped silently; coordinates are rounded to the nearest pixel.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "composited values are clamped to [0, 255] before the \
              u8 cast; image dims ≤ 2^24 are exact in f32"
)]
fn blend_pixel(img: &mut image::RgbaImage, x: f32, y: f32, color: Color, coverage: f32) {
    if coverage <= 0.0 {
        return;
    }
    let (w, h) = img.dimensions();
    let (w_f, h_f) = (w as f32, h as f32);
    let px = x.clamp(0.0, w_f - 1.0).round() as u32;
    let py = y.clamp(0.0, h_f - 1.0).round() as u32;
    let bg = img.get_pixel(px, py);
    // Source-over composite: out = src·α + dst·(1−α). Inputs are u8
    // (exact in f32 via From); output is clamped back to u8.
    let alpha = coverage;
    let out_r = f32::from(color.r) * alpha + f32::from(bg.0[0]) * (1.0 - alpha);
    let out_g = f32::from(color.g) * alpha + f32::from(bg.0[1]) * (1.0 - alpha);
    let out_b = f32::from(color.b) * alpha + f32::from(bg.0[2]) * (1.0 - alpha);
    // Keep the background's alpha — screenshots are opaque, and
    // compositing the source alpha would make text transparent.
    img.put_pixel(
        px,
        py,
        image::Rgba([
            out_r.round().clamp(0.0, 255.0) as u8,
            out_g.round().clamp(0.0, 255.0) as u8,
            out_b.round().clamp(0.0, 255.0) as u8,
            bg.0[3],
        ]),
    );
}

/// Fill `img` with `color` in the rectangle from `(x0,y0)` to
/// `(x1,y1)` (half-open: `x1` and `y1` are exclusive). Coordinates are
/// physical px, clamped to image bounds, rounded to the nearest pixel.
///
/// Callers should pass already-normalized rects (`min` top-left); the
/// helper does not sort. Out-of-bounds coordinates are clamped, not
/// clipped, so partially-on-canvas rects still render their visible
/// portion.
///
/// The `f32 → u32` casts are sign/truncation-safe by construction:
/// each value is clamped to `[0.0, image_dim]` before the cast, and
/// `round()` keeps precision exact for realistic screenshot dims
/// (≤ 2^24, the f32 mantissa limit).
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "values clamped to image bounds and rounded before cast; \
              image dims ≤ 2^24 are exact in f32"
)]
fn fill_rect(img: &mut image::RgbaImage, x0: f32, y0: f32, x1: f32, y1: f32, color: Color) {
    let (w, h) = img.dimensions();
    let (w_f, h_f) = (w as f32, h as f32);
    let x0 = x0.clamp(0.0, w_f).round() as u32;
    let x1 = x1.clamp(0.0, w_f).round() as u32;
    let y0 = y0.clamp(0.0, h_f).round() as u32;
    let y1 = y1.clamp(0.0, h_f).round() as u32;
    let pixel = image::Rgba([color.r, color.g, color.b, color.a]);
    for y in y0..y1 {
        for x in x0..x1 {
            img.put_pixel(x, y, pixel);
        }
    }
}

/// Paint an ellipse inscribed in `rect`: optionally fill the interior,
/// then stroke the outline band (`StrokeKind::Inside`, matching
/// [`ImageCanvas::draw_rect_outline`]).
///
/// Anti-aliasing-free coverage — every pixel whose centre falls inside
/// the implicit ellipse is painted, matching the save path's existing
/// rect / line raster style. The stroke band is the set of pixels
/// inside the outer ellipse (radii `rx`, `ry`) but outside a concentric
/// inner ellipse with radii shrunk by `stroke.width`. This makes the
/// stroke width vary slightly around the perimeter (exact `w` on the
/// axes, marginally less at diagonal angles), which is acceptable for
/// an aa-free screenshot annotation and indistinguishable at typical
/// stroke widths (2–6 px).
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::similar_names,
    reason = "coords clamped/rounded before cast (dims ≤ 2^24 exact in f32); x/y suffix pairs are inherent to 2D geometry, matching the cw/ch convention in draw_image_region"
)]
fn paint_ellipse(img: &mut image::RgbaImage, rect: Rect, stroke: Stroke, fill: Option<Color>) {
    let n = rect.normalized();
    let rw = n.width();
    let rh = n.height();
    if rw <= 0.0 || rh <= 0.0 {
        return;
    }
    let cx = n.min.x + rw / 2.0;
    let cy = n.min.y + rh / 2.0;
    let rx = rw / 2.0;
    let ry = rh / 2.0;

    let sw = stroke.width;
    let has_stroke = sw > 0.0;
    // StrokeKind::Inside: stroke band sits between the outer ellipse
    // boundary and a concentric inner ellipse shrunk by stroke width.
    let inner_rx = (rx - sw).max(0.0);
    let inner_ry = (ry - sw).max(0.0);
    let has_inner = inner_rx > 0.0 && inner_ry > 0.0;

    let (img_w, img_h) = img.dimensions();
    let (iw_f, ih_f) = (img_w as f32, img_h as f32);
    let x0 = n.min.x.floor().clamp(0.0, iw_f) as u32;
    let x1 = n.max.x.ceil().clamp(0.0, iw_f) as u32;
    let y0 = n.min.y.floor().clamp(0.0, ih_f) as u32;
    let y1 = n.max.y.ceil().clamp(0.0, ih_f) as u32;

    let inv_rx2 = 1.0 / (rx * rx);
    let inv_ry2 = 1.0 / (ry * ry);
    let inv_irx2 = if has_inner {
        1.0 / (inner_rx * inner_rx)
    } else {
        0.0
    };
    let inv_iry2 = if has_inner {
        1.0 / (inner_ry * inner_ry)
    } else {
        0.0
    };

    let stroke_px = image::Rgba([
        stroke.color.r,
        stroke.color.g,
        stroke.color.b,
        stroke.color.a,
    ]);
    let fill_px = fill.map(|c| image::Rgba([c.r, c.g, c.b, c.a]));

    for py in y0..y1 {
        let dy = py as f32 + 0.5 - cy;
        let dy_outer = dy * dy * inv_ry2;
        for px in x0..x1 {
            let dx = px as f32 + 0.5 - cx;
            if dy_outer + dx * dx * inv_rx2 > 1.0 {
                continue;
            }
            // Inside outer ellipse — stroke band or fill?
            let in_stroke =
                has_stroke && (!has_inner || dy * dy * inv_iry2 + dx * dx * inv_irx2 > 1.0);
            if in_stroke {
                img.put_pixel(px, py, stroke_px);
            } else if let Some(fp) = fill_px {
                img.put_pixel(px, py, fp);
            }
        }
    }
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
    /// `Canvas::draw_image_region` — raw sub-image blit (blur path).
    DrawImageRegion {
        /// Source image width in physical px.
        width: u32,
        /// Source image height in physical px.
        height: u32,
        /// Destination rect (normalized, physical px).
        dest: Rect,
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
    /// `Canvas::draw_polyline` — a connected freehand stroke.
    DrawPolyline {
        /// Smoothed polyline points.
        points: Vec<Point>,
        /// Stroke style (alpha reduced for `Tool::Marker`).
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
        /// Full text style bundle.
        style: TextStyle,
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

    fn draw_image_region(&mut self, img: &image::RgbaImage, dest: Rect) {
        let (width, height) = img.dimensions();
        self.calls.push(DrawCall::DrawImageRegion {
            width,
            height,
            dest,
        });
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

    fn draw_polyline(&mut self, points: &[Point], stroke: &Stroke) {
        self.calls.push(DrawCall::DrawPolyline {
            points: points.to_vec(),
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

    fn draw_text(&mut self, pos: Point, text: &str, style: &TextStyle) {
        self.calls.push(DrawCall::DrawText {
            pos,
            text: text.to_owned(),
            style: *style,
        });
    }

    /// Deterministic monospace-style estimate: `len * size * 0.6` per
    /// line. Real shells use their shaping backend; the mock only needs
    /// a predictable value so caret / bbox assertions in render tests
    /// can compute the expected geometry without re-implementing
    /// `ab_glyph` / egui's layouter. Multi-line runs split on `\n`;
    /// `width` is the widest line, `last_line_width` is where the
    /// caret sits.
    #[allow(
        clippy::cast_precision_loss,
        reason = "char count is bounded by the draft string length; realistic screenshots have ≤ 2^24 chars"
    )]
    fn measure_text(&self, text: &str, style: &TextStyle) -> TextMetrics {
        let line_width = |line: &str| line.chars().count() as f32 * style.size * 0.6;
        let mut width = 0.0_f32;
        let mut last_line_width = 0.0_f32;
        let mut line_count = 0_usize;
        for line in text.split('\n') {
            let w = line_width(line);
            width = width.max(w);
            last_line_width = w;
            line_count += 1;
        }
        TextMetrics {
            width,
            height: line_count as f32 * style.size * style.line_spacing,
            last_line_width,
        }
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
    use crate::text::DEFAULT_LINE_SPACING;

    /// Expected `DrawText` anchor for a Number-tool label centred in
    /// its bubble, under the `MockCanvas` metrics
    /// (`width = chars · size · 0.6`, `height = size · line_spacing`):
    /// bubble centre minus half the measured label size. Mirrors the
    /// mock's evaluation order (`count · size · 0.6`) so the f32 bits
    /// match exactly and `assert_eq!` is deterministic.
    fn centered_anchor(pos: Point, char_count: f32, font: f32) -> Point {
        let width = char_count * font * 0.6;
        let height = font * DEFAULT_LINE_SPACING;
        Point::new(pos.x - width / 2.0, pos.y - height / 2.0)
    }

    #[test]
    fn render_blits_background_image_once() {
        let state = EditorState::new(image::RgbaImage::new(10, 20));
        let mut canvas = MockCanvas::default();
        let mut bg = None;
        render(&state, &mut canvas, &mut bg, &mut BlurCache::default())
            .expect("render should succeed");

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

        render(&state, &mut canvas, &mut bg, &mut BlurCache::default()).expect("first render");
        assert!(bg.is_some(), "first render populates the cache");
        let calls_after_first = canvas.calls.len();

        render(&state, &mut canvas, &mut bg, &mut BlurCache::default()).expect("second render");
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

        render(&state, &mut canvas, &mut bg, &mut BlurCache::default()).expect("first render");
        bg = None; // simulate cache invalidation
        render(&state, &mut canvas, &mut bg, &mut BlurCache::default())
            .expect("second render after drop");

        // Two uploads + two blits across the two frames.
        assert_eq!(canvas.calls.len(), 4);
        assert!(bg.is_some());
    }

    /// TODO Task 11 acceptance: `arrow_barbs` returns the two ±30°
    /// barb endpoints for a given shaft. Locks the shared geometry
    /// contract that both `ImageCanvas` and `EguiCanvas` render against
    /// — a horizontal shaft from (10, 50) to (90, 50) with `head_size`
    /// 10 produces barbs at (81.34, 45) and (81.34, 55), i.e. each
    /// barb is 10 px long angled 30° off the reversed shaft.
    #[test]
    fn arrow_barbs_returns_30_degree_endpoints() {
        let a = Point::new(10.0, 50.0);
        let b = Point::new(90.0, 50.0);
        let [left, right] = arrow_barbs(a, b, 10.0);

        // Reverse shaft direction is (-1, 0); rotated ±30° gives
        // (-cos30, ∓sin30) = (-0.866, ∓0.5). Each barb is 10 px.
        // left (+30°) → (90 - 8.66, 50 - 5) = (81.34, 45).
        assert!((left.x - 81.34).abs() < 0.01, "left.x = {}", left.x);
        assert!((left.y - 45.0).abs() < 0.01, "left.y = {}", left.y);
        // right (-30°) → (90 - 8.66, 50 + 5) = (81.34, 55).
        assert!((right.x - 81.34).abs() < 0.01, "right.x = {}", right.x);
        assert!((right.y - 55.0).abs() < 0.01, "right.y = {}", right.y);
    }

    /// `arrow_barbs` on a sub-pixel shaft returns both barbs at the
    /// head point (degenerate — caller skips drawing barbs).
    #[test]
    fn arrow_barbs_degenerate_shaft_returns_head_twice() {
        let a = Point::new(50.0, 50.0);
        let b = Point::new(50.4, 50.0); // 0.4 px shaft
        let [left, right] = arrow_barbs(a, b, 10.0);
        assert_eq!(left, b);
        assert_eq!(right, b);
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
        render(&state, &mut canvas, &mut bg, &mut BlurCache::default()).expect("render");

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
    /// going through `render()`'s background-blit path. (`Blur` is
    /// exercised separately in `render_blur_emits_draw_image_region`
    /// — it needs a real `source` image to crop from.)
    #[test]
    fn render_ann_dispatches_each_variant() {
        let mut canvas = MockCanvas::default();
        let stroke = Stroke::new(Color::RED, 2.0);
        // `source` is only consulted by `Blur`; the other variants
        // ignore it. A 1×1 placeholder keeps the test focused.
        let source = image::RgbaImage::new(1, 1);

        render_ann(
            &mut canvas,
            &Annotation::Line {
                a: Point::new(0.0, 0.0),
                b: Point::new(10.0, 10.0),
                stroke,
            },
            &source,
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
            &source,
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
            &source,
        );
        assert!(matches!(
            canvas.calls.last(),
            Some(DrawCall::DrawEllipse { .. })
        ));

        render_ann(
            &mut canvas,
            &Annotation::text(
                Point::new(5.0, 5.0),
                "hi",
                TextStyle::simple(12.0, Color::RED),
            ),
            &source,
        );
        match canvas.calls.last() {
            Some(DrawCall::DrawText { text, .. }) => assert_eq!(text, "hi"),
            other => panic!("expected DrawText, got {other:?}"),
        }

        // Marker / Pencil dispatch through the smoothed-polyline path,
        // not per-segment draw_line (ADR-0022).
        render_ann(
            &mut canvas,
            &Annotation::Marker {
                points: vec![Point::new(0.0, 0.0), Point::new(5.0, 5.0)],
                color: Color::RED,
                width: 4.0,
            },
            &source,
        );
        match canvas.calls.last() {
            Some(DrawCall::DrawPolyline { points, .. }) => {
                assert!(
                    points.len() > 2,
                    "polyline carries the smoothed points, not the raw 2"
                );
            }
            other => panic!("expected DrawPolyline for marker, got {other:?}"),
        }
    }

    /// `Annotation::Blur` dispatches through `image_ops::blur` and
    /// emits a single `draw_image_region` call carrying the blurred
    /// sub-image. Locks the dispatch contract (Task 13): the recorded
    /// call's dest matches the (normalized) blur rect, and the source
    /// dimensions match the cropped region.
    #[test]
    fn render_blur_emits_draw_image_region() {
        // Half-black / half-white 20×10 source so the blur produces a
        // sub-image with deterministic dimensions.
        let mut source = image::RgbaImage::new(20, 10);
        for y in 0..10 {
            for x in 0..10 {
                source.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
            }
            for x in 10..20 {
                source.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
            }
        }
        let ann = Annotation::Blur {
            rect: Rect::from_xywh(5.0, 2.0, 10.0, 6.0),
            kind: crate::editor::BlurKind::Gaussian,
            strength: 2,
        };

        let mut canvas = MockCanvas::default();
        render_ann(&mut canvas, &ann, &source);

        match &canvas.calls[..] {
            [DrawCall::DrawImageRegion {
                width,
                height,
                dest,
            }] => {
                assert_eq!(*width, 10, "cropped region width");
                assert_eq!(*height, 6, "cropped region height");
                assert_eq!(*dest, Rect::from_xywh(5.0, 2.0, 10.0, 6.0));
            }
            other => panic!("expected single DrawImageRegion, got {other:?}"),
        }
    }

    /// Regression: a zero-area (or sub-pixel) `Annotation::Blur` must
    /// not emit `draw_image_region`. The blur render path used to
    /// unconditionally crop → blur → blit, which on the first frame
    /// of every drag (`begin_annotation` seeds `Rect::new(at, at)`)
    /// produced a 0×0 texture and crashed wgpu with
    /// "Dimension X is zero". The guard in `render_ann` returns early
    /// before touching the canvas, so this is the direct crash repro.
    #[test]
    fn render_blur_skips_degenerate_rect() {
        let source = image::RgbaImage::new(20, 10);
        // Zero-area rect — exactly what `begin_annotation` produces
        // on PointerDown before the user drags.
        let zero = Annotation::Blur {
            rect: Rect::new(Point::new(5.0, 5.0), Point::new(5.0, 5.0)),
            kind: crate::editor::BlurKind::Gaussian,
            strength: 2,
        };
        // Sub-1px rect — must also be skipped per the threshold
        // contract shared with `finish_pointer_up` and the selection
        // rect guard.
        let thin = Annotation::Blur {
            rect: Rect::from_xywh(5.0, 5.0, 0.4, 8.0),
            kind: crate::editor::BlurKind::Gaussian,
            strength: 2,
        };

        for ann in [zero, thin] {
            let mut canvas = MockCanvas::default();
            render_ann(&mut canvas, &ann, &source);
            assert!(
                canvas.calls.is_empty(),
                "degenerate blur should emit no draw calls, got {:?}",
                canvas.calls
            );
        }
    }

    /// A pending `Annotation::Text` renders the text run followed by
    /// a single `DrawLine` for the caret indicator. The caret's x
    /// equals `pos.x + measure_text(text, font_size)`, so it sits at
    /// the visible end of the typed run. Locks the caret contract:
    /// without this `Tool::Text` shows no feedback where the typed
    /// text will land (regression flagged against the Task 012
    /// acceptance "Clicking with `Tool::Text` places a text input caret").
    #[test]
    fn render_pending_text_emits_caret_at_end_of_run() {
        let mut state = EditorState::new(image::RgbaImage::new(100, 40));
        let pos = Point::new(10.0, 5.0);
        let font_size = 16.0;
        let color = Color::rgb(0, 0, 0);
        state.pending = Some(Annotation::text(
            pos,
            "hi",
            TextStyle::simple(font_size, color),
        ));
        // Cursor at end of the typed run ("hi" = 2 chars).
        state.text_cursor = 2;

        let mut canvas = MockCanvas::default();
        let mut bg = None;
        render(&state, &mut canvas, &mut bg, &mut BlurCache::default()).expect("render");

        // Upload + blit + DrawText (the run) + DrawLine (the caret).
        let mut it = canvas.calls.iter();
        assert!(matches!(it.next(), Some(DrawCall::UploadImage)));
        assert!(matches!(it.next(), Some(DrawCall::DrawImage { .. })));
        match it.next() {
            Some(DrawCall::DrawText {
                pos: p,
                text,
                style,
            }) => {
                assert_eq!(*p, pos);
                assert_eq!(text, "hi");
                assert_eq!(style.size, font_size);
                assert_eq!(style.color, color);
            }
            other => panic!("expected DrawText for pending run, got {other:?}"),
        }
        // MockCanvas::measure_text returns `len * size * 0.6`, so
        // advance for "hi" at size 16.0 = 19.2 px.
        let advance = 19.2_f32;
        let block_w = block_caret_width(font_size);
        // Bounding-box outline around the laid-out text block.
        match it.next() {
            Some(DrawCall::DrawRectOutline { rect, stroke }) => {
                let want = Rect::from_xywh(
                    pos.x - TEXT_BBOX_PAD,
                    pos.y - TEXT_BBOX_PAD,
                    advance + block_w + TEXT_BBOX_PAD * 2.0,
                    font_size * crate::text::DEFAULT_LINE_SPACING + TEXT_BBOX_PAD * 2.0,
                );
                assert_eq!(*rect, want, "bbox geometry");
                assert_eq!(*stroke, TEXT_BBOX_STROKE, "bbox stroke");
            }
            other => panic!("expected DrawRectOutline bbox, got {other:?}"),
        }
        // Solid block caret at the insertion point.
        match it.next() {
            Some(DrawCall::DrawRectFilled { rect, color: c }) => {
                let want = Rect::from_xywh(pos.x + advance, pos.y, block_w, font_size);
                assert_eq!(*rect, want, "block caret geometry");
                assert_eq!(*c, color, "caret uses text colour");
            }
            other => panic!("expected DrawRectFilled block caret, got {other:?}"),
        }
        assert!(it.next().is_none(), "no further draw calls");
    }

    /// An empty pending text draft still emits a caret — at `pos.x`
    /// (advance is 0). This is the just-clicked-before-typing state,
    /// and is precisely the case where the user needs the indicator
    /// most, since there's no visible text yet to anchor against.
    #[test]
    fn render_pending_empty_text_emits_caret_at_pos() {
        let mut state = EditorState::new(image::RgbaImage::new(100, 40));
        let pos = Point::new(33.0, 12.0);
        let font_size = 20.0;
        state.pending = Some(Annotation::text(
            pos,
            "",
            TextStyle::simple(font_size, Color::RED),
        ));

        let mut canvas = MockCanvas::default();
        let mut bg = None;
        render(&state, &mut canvas, &mut bg, &mut BlurCache::default()).expect("render");

        // The empty run still emits a DrawText (the dispatcher doesn't
        // special-case it), followed by a block caret at pos.x (advance 0).
        let block_w = block_caret_width(font_size);
        let has_caret_at_pos = canvas.calls.iter().any(|c| match c {
            DrawCall::DrawRectFilled { rect, .. } => {
                *rect == Rect::from_xywh(pos.x, pos.y, block_w, font_size)
            }
            _ => false,
        });
        assert!(
            has_caret_at_pos,
            "empty draft must still draw block caret at pos.x"
        );
    }

    /// A *committed* `Annotation::Text` must NOT emit a caret — the
    /// indicator is a live-editor affordance for `pending` only, and
    /// must never reach `compose_final`'s output. Guards against a
    /// regression that wires caret drawing into `render_ann` instead
    /// of the `pending`-only branch in `render()`.
    #[test]
    fn render_committed_text_emits_no_caret() {
        let mut state = EditorState::new(image::RgbaImage::new(100, 40));
        state.anns.push(Annotation::text(
            Point::new(10.0, 5.0),
            "committed",
            TextStyle::simple(16.0, Color::rgb(0, 0, 0)),
        ));

        let mut canvas = MockCanvas::default();
        let mut bg = None;
        render(&state, &mut canvas, &mut bg, &mut BlurCache::default()).expect("render");

        // The committed text emits exactly one DrawText — no block
        // caret (DrawRectFilled) and no bounding box (DrawRectOutline),
        // since the indicators are a `pending`-only affordance.
        assert!(
            canvas
                .calls
                .iter()
                .any(|c| matches!(c, DrawCall::DrawText { .. })),
            "committed text should render"
        );
        assert!(
            !canvas
                .calls
                .iter()
                .any(|c| matches!(c, DrawCall::DrawRectFilled { .. })),
            "committed text must not emit a block caret"
        );
        assert!(
            !canvas
                .calls
                .iter()
                .any(|c| matches!(c, DrawCall::DrawRectOutline { .. })),
            "committed text must not emit a bounding box"
        );
    }

    /// `compose_final` bakes a blur into the saved image: the blurred
    /// region's pixels differ from the original source. Half-black /
    /// half-white → Gaussian blur at the boundary produces grey.
    #[test]
    fn compose_final_blurs_region() {
        let mut source = image::RgbaImage::new(20, 10);
        for y in 0..10 {
            for x in 0..10 {
                source.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
            }
            for x in 10..20 {
                source.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
            }
        }
        let mut state = EditorState::new(source);
        state.anns.push(Annotation::Blur {
            rect: Rect::from_xywh(0.0, 0.0, 20.0, 10.0),
            kind: crate::editor::BlurKind::Gaussian,
            strength: 3,
        });

        let out = compose_final(&state);
        // Boundary pixel must be intermediate grey after Gaussian.
        let mid = out.get_pixel(10, 5).0;
        assert!(
            mid[0] > 20 && mid[0] < 235,
            "boundary blurred to {}",
            mid[0]
        );
        // State image not mutated.
        assert_eq!(
            state.image.get_pixel(10, 5).0,
            [255, 255, 255, 255],
            "state.image unchanged"
        );
    }

    /// A pending marker renders as a single smoothed polyline via one
    /// `draw_polyline` call — not per-segment `draw_line` calls. Locks
    /// the freehand dispatch contract (ADR-0022): smoothing lives in
    /// `render_ann`, the canvas sees the already-smoothed points, and a
    /// 3-point raw input is subdivided into more rendered samples.
    #[test]
    fn render_pending_marker_emits_single_smoothed_polyline() {
        let mut canvas = MockCanvas::default();
        let source = image::RgbaImage::new(1, 1);
        let pts = vec![
            Point::new(0.0, 0.0),
            Point::new(5.0, 5.0),
            Point::new(10.0, 0.0),
        ];
        render_ann(
            &mut canvas,
            &Annotation::Marker {
                points: pts.clone(),
                color: Color::RED,
                width: 4.0,
            },
            &source,
        );
        // No per-segment DrawLine — the whole stroke is one polyline.
        let line_count = canvas
            .calls
            .iter()
            .filter(|c| matches!(c, DrawCall::DrawLine { .. }))
            .count();
        assert_eq!(line_count, 0, "marker must not emit per-segment DrawLine");

        // Exactly one DrawPolyline carrying the smoothed points.
        let polys: Vec<_> = canvas
            .calls
            .iter()
            .filter_map(|c| match c {
                DrawCall::DrawPolyline { points, stroke } => Some((points.clone(), *stroke)),
                _ => None,
            })
            .collect();
        assert_eq!(polys.len(), 1, "exactly one DrawPolyline");
        let (smoothed, stroke) = &polys[0];
        assert_eq!(stroke.color, Color::RED);
        assert_eq!(stroke.width, 4.0);
        assert!(
            smoothed.len() > pts.len(),
            "smoothing must subdivide: {} raw -> {} rendered",
            pts.len(),
            smoothed.len()
        );
        // Catmull-Rom passes through the control points, so the
        // smoothed polyline's endpoints match the raw input.
        assert_eq!(smoothed.first(), pts.first());
        assert_eq!(smoothed.last(), pts.last());
    }

    /// A committed `Annotation::Line` renders as exactly one `DrawLine`
    /// call through the full `render()` path — not a polyline of
    /// segments (that's Marker/Pencil) and not zero (would mean the
    /// dispatch dropped it). Locks the single-segment contract so a
    /// regression that fragments Line into per-pixel segments, or
    /// drops it entirely, is caught.
    #[test]
    fn render_committed_line_emits_single_draw_line() {
        let mut state = EditorState::new(image::RgbaImage::new(40, 40));
        state.anns.push(Annotation::Line {
            a: Point::new(5.0, 5.0),
            b: Point::new(35.0, 35.0),
            stroke: Stroke::new(Color::RED, 2.0),
        });

        let mut canvas = MockCanvas::default();
        let mut bg = None;
        render(&state, &mut canvas, &mut bg, &mut BlurCache::default()).expect("render");

        let line_count = canvas
            .calls
            .iter()
            .filter(|c| matches!(c, DrawCall::DrawLine { .. }))
            .count();
        assert_eq!(line_count, 1, "committed line emits exactly one DrawLine");
    }

    /// TODO Task 9 acceptance: `compose_final` renders `anns` into a
    /// clone of `state.image`. Pixel-level assertions on a known
    /// fixture state — see Task 9 close-out for why we chose pixel
    /// assertions over a checked-in PNG fixture.
    #[test]
    fn compose_final_renders_rect_outline_into_clone() {
        // Background: white 100x100.
        let bg = image::RgbaImage::from_pixel(100, 100, image::Rgba([255, 255, 255, 255]));
        let mut state = EditorState::new(bg);
        state.anns.push(Annotation::Rect {
            // Outline-only rect at (10,10)-(40,40), red, 3-px stroke.
            rect: Rect::from_xywh(10.0, 10.0, 30.0, 30.0),
            stroke: Stroke::new(Color::RED, 3.0),
            fill: None,
        });

        let out = compose_final(&state);

        // Stroke is drawn inside the rect (3-px band starting at the
        // edge). Pixel on the corner is stroke-coloured.
        assert_eq!(
            out.get_pixel(10, 10),
            &image::Rgba([255, 0, 0, 255]),
            "top-left corner should be stroke-coloured"
        );
        // Mid-edge of the stroke band.
        assert_eq!(
            out.get_pixel(25, 11),
            &image::Rgba([255, 0, 0, 255]),
            "top edge inside stroke band should be stroke-coloured"
        );
        // Inside the rect, past the 3-px stroke: background.
        assert_eq!(
            out.get_pixel(25, 25),
            &image::Rgba([255, 255, 255, 255]),
            "centre (past stroke) should be background-coloured"
        );
        // Outside the rect: background unchanged.
        assert_eq!(
            out.get_pixel(80, 80),
            &image::Rgba([255, 255, 255, 255]),
            "outside-rect pixel should be background"
        );
        // Background image not mutated.
        assert_eq!(
            state.image.get_pixel(10, 10),
            &image::Rgba([255, 255, 255, 255]),
            "state.image must be unchanged (compose_final clones)"
        );
    }

    /// `compose_final` bakes a filled rect: fill colour inside, stroke
    /// colour on the outline band.
    #[test]
    fn compose_final_renders_rect_fill_and_outline() {
        let bg = image::RgbaImage::from_pixel(100, 100, image::Rgba([255, 255, 255, 255]));
        let mut state = EditorState::new(bg);
        state.anns.push(Annotation::Rect {
            rect: Rect::from_xywh(20.0, 20.0, 40.0, 40.0),
            stroke: Stroke::new(Color::rgb(0, 0, 255), 2.0), // blue
            fill: Some(Color::rgb(0, 255, 0)),               // green
        });

        let out = compose_final(&state);

        // Centre is fill.
        assert_eq!(
            out.get_pixel(40, 40),
            &image::Rgba([0, 255, 0, 255]),
            "centre should be fill colour"
        );
        // Edge is stroke (drawn after fill, on top).
        assert_eq!(
            out.get_pixel(20, 20),
            &image::Rgba([0, 0, 255, 255]),
            "corner should be stroke colour"
        );
    }

    /// `compose_final` bakes a filled ellipse: fill colour inside the
    /// inscribed ellipse, stroke colour in the outline band, background
    /// outside. This is the Task 001 acceptance test — before the
    /// `ImageCanvas::draw_ellipse` impl, the no-op silently dropped
    /// ellipses from saved/copied PNGs.
    #[test]
    fn compose_final_renders_filled_ellipse() {
        // Background: white 100×100.
        let bg = image::RgbaImage::from_pixel(100, 100, image::Rgba([255, 255, 255, 255]));
        let mut state = EditorState::new(bg);
        // Ellipse bounding rect (20,30)-(80,70): rx=30, ry=20, centre
        // (50,50). Blue 2-px stroke, green fill.
        state.anns.push(Annotation::Ellipse {
            rect: Rect::from_xywh(20.0, 30.0, 60.0, 40.0),
            stroke: Stroke::new(Color::rgb(0, 0, 255), 2.0),
            fill: Some(Color::rgb(0, 255, 0)),
        });

        let out = compose_final(&state);

        // Centre is inside the inner ellipse → fill.
        assert_eq!(
            out.get_pixel(50, 50),
            &image::Rgba([0, 255, 0, 255]),
            "centre should be fill colour"
        );
        // Left edge of the ellipse at centre height → stroke band.
        // Pixel (21,50): centre (21.5,50.5), dx=−28.5 → outside the
        // inner ellipse (rx−2=28) but inside the outer (rx=30).
        assert_eq!(
            out.get_pixel(21, 50),
            &image::Rgba([0, 0, 255, 255]),
            "left stroke band should be stroke colour"
        );
        // Bounding-rect corner is outside the inscribed ellipse →
        // background unchanged.
        assert_eq!(
            out.get_pixel(20, 30),
            &image::Rgba([255, 255, 255, 255]),
            "corner outside ellipse should be background"
        );
        // Well outside the ellipse → background.
        assert_eq!(
            out.get_pixel(90, 90),
            &image::Rgba([255, 255, 255, 255]),
            "far-outside pixel should be background"
        );
        // Background image not mutated.
        assert_eq!(
            state.image.get_pixel(50, 50),
            &image::Rgba([255, 255, 255, 255]),
            "state.image must be unchanged (compose_final clones)"
        );
    }

    /// A committed marker is a translucent highlight: in the saved /
    /// copied PNG its pixels are a source-over blend of the marker
    /// colour over the background, not a hard replace. Pins the marker
    /// alpha + per-pixel-once compositing contract (ADR-0022): a
    /// regression that drops alpha, restamps overlapping segments, or
    /// skips the polyline path would make the centreline pixel either
    /// pure-yellow (0) or pure-white (255) on the blue channel.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "test-only: value clamped to [0,255] before the u8 cast"
    )]
    #[test]
    fn compose_final_marker_blends_translucently_with_background() {
        let bg = image::RgbaImage::from_pixel(100, 100, image::Rgba([255, 255, 255, 255]));
        let mut state = EditorState::new(bg);
        state.color = Color::rgb(255, 255, 0); // yellow
        state.stroke_width = 8.0;
        state.tool = Tool::Marker;
        // Straight horizontal stroke — `begin_annotation` stores the
        // marker colour with MARKER_ALPHA already applied.
        state.begin_annotation(Point::new(20.0, 50.0));
        state.update_annotation(Point::new(80.0, 50.0));
        state.commit_annotation();

        let out = compose_final(&state);

        let centre = out.get_pixel(50, 50);
        // Source-over keeps the opaque screenshot alpha.
        assert_eq!(centre.0[3], 255, "composited pixel keeps opaque alpha");
        // Red + green: marker and background are both 255 → stays 255.
        assert_eq!(centre.0[0], 255, "red unchanged");
        assert_eq!(centre.0[1], 255, "green unchanged");
        // Blue: marker 0 over background 255 at MARKER_ALPHA → a value
        // strictly between 0 and 255, matching the exact blend.
        let alpha = f32::from(crate::MARKER_ALPHA) / 255.0;
        let expected_b = (255.0 * (1.0 - alpha)).round().clamp(0.0, 255.0) as u8;
        assert_eq!(
            centre.0[2], expected_b,
            "blue channel is the source-over blend (between bg and marker)"
        );
        assert!(
            centre.0[2] > 0 && centre.0[2] < 255,
            "blended pixel must be strictly between marker and background"
        );

        // A pixel off the stroke is untouched background.
        assert_eq!(
            out.get_pixel(50, 20).0,
            [255, 255, 255, 255],
            "off-stroke pixel is background"
        );
        // Background image not mutated.
        assert_eq!(
            state.image.get_pixel(50, 50).0,
            [255, 255, 255, 255],
            "state.image unchanged"
        );
    }

    /// `compose_final` ignores `pending` — in-flight annotations are
    /// not part of the saved output.
    #[test]
    fn compose_final_excludes_pending() {
        let bg = image::RgbaImage::from_pixel(50, 50, image::Rgba([255, 255, 255, 255]));
        let mut state = EditorState::new(bg);
        state.pending = Some(Annotation::Rect {
            rect: Rect::from_xywh(0.0, 0.0, 50.0, 50.0),
            stroke: Stroke::new(Color::RED, 50.0),
            fill: None,
        });

        let out = compose_final(&state);

        // Pending annotation would have covered the whole image red;
        // its absence means the background shows through.
        assert_eq!(
            out.get_pixel(25, 25),
            &image::Rgba([255, 255, 255, 255]),
            "pending must not appear in compose_final output"
        );
    }

    /// `compose_final` on an empty state returns a verbatim clone of
    /// `state.image` (no annotations drawn, no resize).
    #[test]
    fn compose_final_empty_state_is_image_clone() {
        let bg = image::RgbaImage::from_pixel(8, 8, image::Rgba([10, 20, 30, 255]));
        let state = EditorState::new(bg.clone());
        let out = compose_final(&state);
        assert_eq!(out.dimensions(), bg.dimensions());
        assert_eq!(out.as_raw(), bg.as_raw());
    }

    /// `compose_final_region` returns the sub-image at `rect` with
    /// annotations already baked in. Pixel-level check: a red rect
    /// drawn over the boundary between cropped-in and cropped-out
    /// regions appears only inside the crop.
    #[test]
    fn compose_final_region_crops_with_annotations() {
        // Background: white 100x100.
        let bg = image::RgbaImage::from_pixel(100, 100, image::Rgba([255, 255, 255, 255]));
        let mut state = EditorState::new(bg);
        // Solid red 100x100 rect covering the whole canvas — every
        // output pixel is red after compose_final.
        state.anns.push(Annotation::Rect {
            rect: Rect::from_xywh(0.0, 0.0, 100.0, 100.0),
            stroke: Stroke::new(Color::RED, 1.0),
            fill: Some(Color::RED),
        });

        let out = compose_final_region(&state, Rect::from_xywh(20.0, 30.0, 40.0, 50.0));

        assert_eq!(out.dimensions(), (40, 50), "cropped to rect size");
        // Every pixel inside the crop is red (fill applied before crop).
        for y in 0..50 {
            for x in 0..40 {
                assert_eq!(
                    out.get_pixel(x, y),
                    &image::Rgba([255, 0, 0, 255]),
                    "pixel ({x},{y}) should be red after crop"
                );
            }
        }
    }

    /// `compose_final_region` matches `crop_region(&compose_final(state), rect)`
    /// byte-for-byte — the documented equivalence. Catches any future
    /// refactor that tries to "optimise" by translating annotations
    /// directly into a smaller canvas (which would diverge on
    /// stroke-asymmetry / clipping edge cases).
    #[test]
    fn compose_final_region_matches_compose_then_crop() {
        let bg = image::RgbaImage::from_pixel(64, 64, image::Rgba([255, 255, 255, 255]));
        let mut state = EditorState::new(bg);
        state.anns.push(Annotation::Rect {
            rect: Rect::from_xywh(10.0, 10.0, 30.0, 30.0),
            stroke: Stroke::new(Color::rgb(0, 200, 0), 2.0),
            fill: None,
        });
        let rect = Rect::from_xywh(15.0, 15.0, 20.0, 20.0);

        let direct = compose_final_region(&state, rect);
        let two_step = crate::image_ops::crop_region(&compose_final(&state), rect);

        assert_eq!(direct.dimensions(), two_step.dimensions());
        assert_eq!(
            direct.as_raw(),
            two_step.as_raw(),
            "pixel buffers must match"
        );
    }

    /// `compose_final_region` does NOT mutate `state` — the editor's
    /// image, annotations, and selection are all left intact. This is
    /// the contract that distinguishes it from `EditorState::crop`
    /// (which the Copy path must not invoke, since the user is not
    /// asking to narrow the editor — only to copy the selection).
    #[test]
    fn compose_final_region_does_not_mutate_state() {
        let bg = image::RgbaImage::from_pixel(50, 50, image::Rgba([255, 255, 255, 255]));
        let mut state = EditorState::new(bg.clone());
        state.anns.push(Annotation::Rect {
            rect: Rect::from_xywh(5.0, 5.0, 10.0, 10.0),
            stroke: Stroke::new(Color::RED, 1.0),
            fill: None,
        });
        state.crop_region = Some(Rect::from_xywh(10.0, 10.0, 20.0, 20.0));

        let img_before = state.image.clone();
        let anns_before = state.anns.clone();
        let sel_before = state.crop_region;

        let _ = compose_final_region(&state, Rect::from_xywh(10.0, 10.0, 20.0, 20.0));

        assert_eq!(state.image.as_raw(), img_before.as_raw(), "image unchanged");
        assert_eq!(state.anns.len(), anns_before.len(), "anns unchanged");
        assert_eq!(state.crop_region, sel_before, "crop region unchanged");
    }

    /// A zero-area `rect` (matching `crop_region`'s degenerate-input
    /// contract) yields a 0×0 image. The Copy path guards against
    /// degenerate selections before calling, but the function itself
    /// must still be safe to call with a degenerate rect.
    #[test]
    fn compose_final_region_degenerate_is_empty() {
        let state = EditorState::new(image::RgbaImage::new(50, 50));
        // Zero-width rect: min.x == max.x, so x1.saturating_sub(x0) == 0.
        let out = compose_final_region(&state, Rect::from_xywh(10.0, 10.0, 0.0, 20.0));
        assert_eq!(out.dimensions(), (0, 0));
    }

    /// A non-empty `state.crop_region` renders as a single
    /// `DrawRectOutline` on top of annotations + pending, using the
    /// distinct selection stroke.
    #[test]
    fn render_draws_selection_rect_on_top() {
        let mut state = EditorState::new(image::RgbaImage::new(100, 100));
        state.anns.push(Annotation::Rect {
            rect: Rect::from_xywh(10.0, 10.0, 30.0, 30.0),
            stroke: Stroke::new(Color::RED, 3.0),
            fill: None,
        });
        state.crop_region = Some(Rect::from_xywh(5.0, 5.0, 80.0, 80.0));

        let mut canvas = MockCanvas::default();
        let mut bg = None;
        render(&state, &mut canvas, &mut bg, &mut BlurCache::default()).expect("render");

        // Upload + blit + committed rect outline + selection outline.
        assert_eq!(canvas.calls.len(), 4);
        assert!(matches!(canvas.calls[0], DrawCall::UploadImage));
        assert!(matches!(canvas.calls[1], DrawCall::DrawImage { .. }));
        assert!(matches!(canvas.calls[2], DrawCall::DrawRectOutline { .. }));
        match &canvas.calls[3] {
            DrawCall::DrawRectOutline { rect, stroke } => {
                assert_eq!(*rect, Rect::from_xywh(5.0, 5.0, 80.0, 80.0));
                // Distinct from any annotation color (blue).
                assert_eq!(stroke.color, Color::rgb(0, 120, 215));
            }
            other => panic!("expected selection outline, got {other:?}"),
        }
    }

    /// A degenerate selection (< 1 px) is not rendered.
    #[test]
    fn render_skips_degenerate_selection() {
        let mut state = EditorState::new(image::RgbaImage::new(100, 100));
        state.crop_region = Some(Rect::new(Point::new(50.0, 50.0), Point::new(50.5, 50.0)));

        let mut canvas = MockCanvas::default();
        let mut bg = None;
        render(&state, &mut canvas, &mut bg, &mut BlurCache::default()).expect("render");

        // Only upload + blit — no selection outline.
        assert_eq!(canvas.calls.len(), 2);
    }

    /// TODO Task 12 acceptance: `compose_final` bakes a text annotation
    /// into the saved image via `ab_glyph` + system fontconfig. Soft
    /// assertion — per the plan ("tests assert on geometry/flow, not
    /// pixel-exact glyph shapes") we don't pin glyph pixels, but we DO
    /// verify the call never panics and that *if* a system font is
    /// discoverable, at least one pixel near the anchor changes from
    /// the background. Hosts without a sans-serif installed (minimal
    /// CI containers) skip the pixel leg gracefully.
    #[test]
    fn compose_final_renders_text_or_skips_gracefully() {
        let bg = image::RgbaImage::from_pixel(100, 40, image::Rgba([255, 255, 255, 255]));
        let mut state = EditorState::new(bg);
        state.anns.push(Annotation::text(
            Point::new(5.0, 5.0),
            "flintshot",
            TextStyle::simple(16.0, Color::rgb(0, 0, 0)),
        ));

        let out = compose_final(&state);
        // Must succeed and preserve dimensions regardless of font.
        assert_eq!(out.dimensions(), (100, 40));

        // Soft check: scan the expected text band for any non-white
        // pixel. If a font was available, glyphs landed here; if not,
        // `draw_text` warned and skipped (no pixels changed), which is
        // acceptable in a font-less environment.
        let drew_anything = (0..100)
            .zip(0..40)
            .flat_map(|(x, _)| (0..40).map(move |y| (x, y)))
            .any(|(x, y)| *out.get_pixel(x, y) != image::Rgba([255, 255, 255, 255]));
        if drew_anything {
            // Font was available — sanity-check the background image
            // wasn't mutated (compose_final clones).
            assert_eq!(
                state.image.get_pixel(0, 0),
                &image::Rgba([255, 255, 255, 255]),
                "state.image unchanged"
            );
        }
    }

    /// TODO Task 11 acceptance: render an `EditorState` with one
    /// committed `Annotation::Arrow` and assert the exact `DrawArrow`
    /// call arguments. Locks the dispatch contract: a single
    /// `draw_arrow` call carrying the annotation's tail, head, stroke,
    /// and `head_size` verbatim — no normalization, no recompute.
    #[test]
    fn render_arrow_emits_single_draw_arrow_call() {
        let mut state = EditorState::new(image::RgbaImage::new(100, 100));
        state.anns.push(Annotation::Arrow {
            a: Point::new(10.0, 50.0),
            b: Point::new(90.0, 50.0),
            stroke: Stroke::new(Color::RED, 3.0),
            head_size: 10.0,
        });

        let mut canvas = MockCanvas::default();
        let mut bg = None;
        render(&state, &mut canvas, &mut bg, &mut BlurCache::default()).expect("render");

        // Upload + blit + one DrawArrow.
        assert_eq!(canvas.calls.len(), 3);
        assert!(matches!(canvas.calls[0], DrawCall::UploadImage));
        assert!(matches!(canvas.calls[1], DrawCall::DrawImage { .. }));
        match &canvas.calls[2] {
            DrawCall::DrawArrow {
                a,
                b,
                stroke,
                head_size,
            } => {
                assert_eq!(*a, Point::new(10.0, 50.0), "tail = down point");
                assert_eq!(*b, Point::new(90.0, 50.0), "head = up point");
                assert_eq!(*stroke, Stroke::new(Color::RED, 3.0));
                assert_eq!(*head_size, 10.0);
            }
            other => panic!("expected DrawArrow, got {other:?}"),
        }
    }

    /// TODO Task 11 acceptance: `compose_final` bakes an arrow (shaft +
    /// ±30° barbs) into the saved / copied image. Pixel-level
    /// assertions on a horizontal arrow — shaft midpoint, tail, head,
    /// and both barb endpoints are stroke-coloured; pixels past the
    /// head and off the shaft stay background.
    ///
    /// Currently RED: `ImageCanvas::draw_arrow` is a warn-only no-op
    /// stub until Task 11 fills it in.
    #[test]
    fn compose_final_renders_arrow_shaft_and_barbs() {
        let bg = image::RgbaImage::from_pixel(100, 100, image::Rgba([255, 255, 255, 255]));
        let mut state = EditorState::new(bg);
        state.anns.push(Annotation::Arrow {
            a: Point::new(10.0, 50.0),
            b: Point::new(90.0, 50.0),
            stroke: Stroke::new(Color::RED, 3.0),
            head_size: 10.0,
        });

        let out = compose_final(&state);
        let red = image::Rgba([255, 0, 0, 255]);
        let white = image::Rgba([255, 255, 255, 255]);

        // Shaft midpoint.
        assert_eq!(out.get_pixel(50, 50), &red, "shaft midpoint");
        // Tail near `a`.
        assert_eq!(out.get_pixel(11, 50), &red, "tail near down point");
        // Head near `b`.
        assert_eq!(out.get_pixel(89, 50), &red, "head near up point");
        // Left barb endpoint ≈ (90 − 10·cos30°, 50 − 10·sin30°)
        //                   ≈ (81.3, 45) → pixel (81, 45).
        assert_eq!(out.get_pixel(81, 45), &red, "left barb endpoint");
        // Right barb endpoint ≈ (81.3, 55) → pixel (81, 55).
        assert_eq!(out.get_pixel(81, 55), &red, "right barb endpoint");
        // Past the head — no shaft, no barb.
        assert_eq!(out.get_pixel(95, 50), &white, "past head is background");
        // Off the shaft, above the barbs.
        assert_eq!(out.get_pixel(50, 30), &white, "off-shaft is background");
        // Background image not mutated.
        assert_eq!(
            state.image.get_pixel(50, 50),
            &white,
            "state.image unchanged (compose_final clones)"
        );
    }

    /// TODO Task 8 acceptance: render an `EditorState` with two
    /// committed `Annotation::Rect`s; assert recorded `draw_rect_outline`
    /// count and arguments. Exercises the path the live Rect tool
    /// produces: drag-rect → commit → render in subsequent frames.
    #[test]
    fn render_two_committed_rects_emits_two_outlines() {
        let mut state = EditorState::new(image::RgbaImage::new(100, 100));
        // First rect: outline only (no fill).
        state.anns.push(Annotation::Rect {
            rect: Rect::from_xywh(10.0, 10.0, 20.0, 20.0),
            stroke: Stroke::new(Color::RED, 3.0),
            fill: None,
        });
        // Second rect: filled + outlined.
        state.anns.push(Annotation::Rect {
            rect: Rect::from_xywh(50.0, 50.0, 30.0, 30.0),
            stroke: Stroke::new(Color::rgb(0, 255, 0), 5.0),
            fill: Some(Color::rgb(0, 0, 255)),
        });

        let mut canvas = MockCanvas::default();
        let mut bg = None;
        render(&state, &mut canvas, &mut bg, &mut BlurCache::default()).expect("render");

        // Expected sequence: UploadImage, DrawImage (bg),
        // DrawRectOutline (rect 1, outline only),
        // DrawRectFilled (rect 2 fill) + DrawRectOutline (rect 2 outline).
        assert_eq!(canvas.calls.len(), 5);
        assert!(matches!(canvas.calls[0], DrawCall::UploadImage));
        assert!(matches!(canvas.calls[1], DrawCall::DrawImage { .. }));

        // Both outlines recorded with the exact geometry + stroke we
        // committed — verifies the render path doesn't mutate ann state.
        let outlines: Vec<_> = canvas
            .calls
            .iter()
            .filter_map(|c| match c {
                DrawCall::DrawRectOutline { rect, stroke } => Some((*rect, *stroke)),
                _ => None,
            })
            .collect();
        assert_eq!(outlines.len(), 2, "exactly two outlines");
        assert_eq!(outlines[0].0, Rect::from_xywh(10.0, 10.0, 20.0, 20.0));
        assert_eq!(outlines[0].1, Stroke::new(Color::RED, 3.0));
        assert_eq!(outlines[1].0, Rect::from_xywh(50.0, 50.0, 30.0, 30.0));
        assert_eq!(outlines[1].1, Stroke::new(Color::rgb(0, 255, 0), 5.0));

        // Fill is emitted before the outline (matches `render_ann`'s
        // order: fill first, then outline on top).
        match &canvas.calls[2] {
            DrawCall::DrawRectOutline { rect, .. } => {
                assert_eq!(*rect, Rect::from_xywh(10.0, 10.0, 20.0, 20.0));
            }
            other => panic!("expected outline for rect 1, got {other:?}"),
        }
        match &canvas.calls[3] {
            DrawCall::DrawRectFilled { rect, color } => {
                assert_eq!(*rect, Rect::from_xywh(50.0, 50.0, 30.0, 30.0));
                assert_eq!(*color, Color::rgb(0, 0, 255));
            }
            other => panic!("expected fill for rect 2, got {other:?}"),
        }
        match &canvas.calls[4] {
            DrawCall::DrawRectOutline { rect, stroke } => {
                assert_eq!(*rect, Rect::from_xywh(50.0, 50.0, 30.0, 30.0));
                assert_eq!(stroke.color, Color::rgb(0, 255, 0));
            }
            other => panic!("expected outline for rect 2, got {other:?}"),
        }
    }

    /// The live `render()` path draws a thin outline around each blur
    /// region so the user can locate it even on uniform backgrounds
    /// (Gaussian blur of a flat colour is invisible). Without this the
    /// user sees no box indicator at all — Bug 1 in the original report.
    #[test]
    fn render_draws_outline_for_committed_blur() {
        let mut state = EditorState::new(image::RgbaImage::new(100, 100));
        state.anns.push(Annotation::Blur {
            rect: Rect::from_xywh(20.0, 20.0, 40.0, 30.0),
            kind: crate::editor::BlurKind::Gaussian,
            strength: 3,
        });

        let mut canvas = MockCanvas::default();
        let mut bg = None;
        let mut cache = BlurCache::default();
        render(&state, &mut canvas, &mut bg, &mut cache).expect("render");

        // Expect: UploadImage, DrawImage (bg),
        //         DrawImageRegion (blurred pixels),
        //         DrawRectOutline (blur boundary indicator).
        let has_blur_outline = canvas.calls.iter().any(|c| match c {
            DrawCall::DrawRectOutline { rect, stroke } => {
                *rect == Rect::from_xywh(20.0, 20.0, 40.0, 30.0)
                    && stroke.color == Color::rgba(255, 255, 255, 180)
            }
            _ => false,
        });
        assert!(
            has_blur_outline,
            "committed blur should have a visible outline, got {:?}",
            canvas.calls
        );
    }

    /// The live `render()` path also draws an outline for a *pending*
    /// blur annotation (during drag), so the user sees a box indicator
    /// while creating the blur region.
    #[test]
    fn render_draws_outline_for_pending_blur() {
        let mut state = EditorState::new(image::RgbaImage::new(100, 100));
        state.pending = Some(Annotation::Blur {
            rect: Rect::from_xywh(10.0, 10.0, 50.0, 50.0),
            kind: crate::editor::BlurKind::Gaussian,
            strength: 3,
        });

        let mut canvas = MockCanvas::default();
        let mut bg = None;
        let mut cache = BlurCache::default();
        render(&state, &mut canvas, &mut bg, &mut cache).expect("render");

        let has_blur_outline = canvas.calls.iter().any(|c| match c {
            DrawCall::DrawRectOutline { rect, .. } => {
                *rect == Rect::from_xywh(10.0, 10.0, 50.0, 50.0)
            }
            _ => false,
        });
        assert!(
            has_blur_outline,
            "pending blur should have a visible outline during drag"
        );
    }

    /// Blur outlines are a live-view-only affordance. `compose_final`
    /// (which drives `render_ann` directly, not `render`) must NOT
    /// include them in saved / copied output.
    #[test]
    fn compose_final_excludes_blur_outline() {
        let mut state = EditorState::new(image::RgbaImage::new(100, 100));
        state.anns.push(Annotation::Blur {
            rect: Rect::from_xywh(20.0, 20.0, 40.0, 30.0),
            kind: crate::editor::BlurKind::Gaussian,
            strength: 3,
        });

        let out = compose_final(&state);
        // The blur region's top-left corner should be blurred source
        // pixels, not the white outline colour. If the outline leaked
        // into compose_final, (20, 20) would be white-ish.
        let corner = out.get_pixel(20, 20).0;
        assert!(
            !(corner[0] > 240 && corner[1] > 240 && corner[2] > 240),
            "blur outline leaked into compose_final: corner = {corner:?}"
        );
    }

    /// The blur cache computes each committed blur once and reuses it
    /// across frames. Render the same state twice with the same cache
    /// and verify both frames produce the same draw calls — the cache
    /// is transparent (the blit still happens), just the expensive
    /// computation is skipped.
    #[test]
    fn blur_cache_reuses_across_frames() {
        let mut state = EditorState::new(image::RgbaImage::new(50, 50));
        state.anns.push(Annotation::Blur {
            rect: Rect::from_xywh(5.0, 5.0, 30.0, 30.0),
            kind: crate::editor::BlurKind::Gaussian,
            strength: 4,
        });

        let mut cache = BlurCache::default();

        // Frame 1 — cache miss, computes + stores.
        let mut canvas1 = MockCanvas::default();
        let mut bg1 = None;
        render(&state, &mut canvas1, &mut bg1, &mut cache).expect("frame 1");
        let frame1_region: Vec<_> = canvas1
            .calls
            .iter()
            .filter_map(|c| match c {
                DrawCall::DrawImageRegion {
                    width,
                    height,
                    dest,
                    ..
                } => Some((*width, *height, *dest)),
                _ => None,
            })
            .collect();

        // Frame 2 — cache hit, blits same image.
        let mut canvas2 = MockCanvas::default();
        let mut bg2 = None;
        render(&state, &mut canvas2, &mut bg2, &mut cache).expect("frame 2");
        let frame2_region: Vec<_> = canvas2
            .calls
            .iter()
            .filter_map(|c| match c {
                DrawCall::DrawImageRegion {
                    width,
                    height,
                    dest,
                    ..
                } => Some((*width, *height, *dest)),
                _ => None,
            })
            .collect();

        assert_eq!(
            frame1_region, frame2_region,
            "cached blur should produce identical blit geometry across frames"
        );
    }

    /// `BlurCache::clear` drops all entries. After clearing, the next
    /// render recomputes from scratch.
    #[test]
    fn blur_cache_clear_works() {
        let mut state = EditorState::new(image::RgbaImage::new(50, 50));
        state.anns.push(Annotation::Blur {
            rect: Rect::from_xywh(5.0, 5.0, 30.0, 30.0),
            kind: crate::editor::BlurKind::Gaussian,
            strength: 4,
        });

        let mut cache = BlurCache::default();
        let mut canvas = MockCanvas::default();
        let mut bg = None;

        // Populate cache.
        render(&state, &mut canvas, &mut bg, &mut cache).expect("first render");
        assert_eq!(canvas.calls.len(), 4, "upload + blit + region + outline");

        // Clear + re-render — should still produce the same calls.
        cache.clear();
        canvas.calls.clear();
        render(&state, &mut canvas, &mut bg, &mut cache).expect("re-render after clear");
        assert!(
            canvas
                .calls
                .iter()
                .any(|c| matches!(c, DrawCall::DrawImageRegion { .. })),
            "blur should re-render after cache clear"
        );
    }

    // --- Task 004: Number-tool render path tests ---

    /// `contrast_text_color` picks white on dark fills and black on
    /// light ones, using Rec. 601 luminance (`Y = 0.299R + 0.587G +
    /// 0.114B`) with a 128-threshold. Pure green is the canonical
    /// boundary case for the formula: at full intensity it lands at
    /// `255 × 0.587 ≈ 150`, above the threshold, so it gets black —
    /// which matches the perceptual intuition that pure green is
    /// "bright". The test pins every common annotation colour so a
    /// future refactor of the formula doesn't silently flip a case.
    #[test]
    fn contrast_text_color_picks_white_on_dark_black_on_light() {
        // Dark fills → white text.
        assert_eq!(
            contrast_text_color(Color::rgb(0, 0, 0)),
            Color::rgb(255, 255, 255),
            "black bg → white"
        );
        assert_eq!(
            contrast_text_color(Color::rgb(255, 0, 0)),
            Color::rgb(255, 255, 255),
            "red bg → white"
        );
        assert_eq!(
            contrast_text_color(Color::rgb(0, 0, 255)),
            Color::rgb(255, 255, 255),
            "blue bg → white"
        );
        assert_eq!(
            contrast_text_color(Color::rgb(128, 0, 128)),
            Color::rgb(255, 255, 255),
            "purple bg → white"
        );
        // Light fills → black text.
        assert_eq!(
            contrast_text_color(Color::rgb(255, 255, 255)),
            Color::rgb(0, 0, 0),
            "white bg → black"
        );
        assert_eq!(
            contrast_text_color(Color::rgb(255, 255, 0)),
            Color::rgb(0, 0, 0),
            "yellow bg → black"
        );
        // Boundary: pure green at full intensity lands just above 128
        // (Y = 0.587 × 255 = 149.7), so it picks black. Halving the
        // green channel drops Y to 74.8 — well under the threshold,
        // back to white. This is the "boundary unit test" the task
        // acceptance criteria call for: a small change in luminance
        // around the threshold flips the result predictably.
        assert_eq!(
            contrast_text_color(Color::rgb(0, 255, 0)),
            Color::rgb(0, 0, 0),
            "bright green (Y≈150) → black"
        );
        assert_eq!(
            contrast_text_color(Color::rgb(0, 128, 0)),
            Color::rgb(255, 255, 255),
            "dim green (Y≈75) → white"
        );
    }

    /// A committed `Annotation::Number` renders as exactly one filled
    /// `DrawEllipse` (a circle — equal width / height) followed by one
    /// `DrawText` for the digit. The text colour is the
    /// luminance-contrast pick (white on the dark-blue fill here).
    /// Replaces the old square-bubble placeholder (`draw_rect_filled`).
    #[test]
    fn render_ann_number_emits_filled_ellipse_and_text() {
        let mut canvas = MockCanvas::default();
        let source = image::RgbaImage::new(1, 1);
        render_ann(
            &mut canvas,
            &Annotation::Number {
                pos: Point::new(50.0, 50.0),
                n: 7,
                color: Color::rgb(0, 0, 255), // dark blue → white text
                diameter: 28.0,
            },
            &source,
        );

        // Exactly two calls — no `draw_rect_filled` square placeholder.
        assert_eq!(canvas.calls.len(), 2, "filled ellipse + text only");
        match &canvas.calls[0] {
            DrawCall::DrawEllipse { rect, fill, stroke } => {
                // Equal axes → a true circle (ellipse inscribed in a
                // square bbox). Bounding rect is centred on `pos`.
                assert_eq!(rect.width(), rect.height(), "equal axes (circle, not oval)");
                let half = 14.0;
                assert_eq!(*rect, Rect::from_xywh(50.0 - half, 50.0 - half, 28.0, 28.0));
                assert_eq!(
                    *fill,
                    Some(Color::rgb(0, 0, 255)),
                    "filled with bubble colour"
                );
                assert_eq!(stroke.width, 0.0, "no outline on the bubble");
            }
            other => panic!("expected DrawEllipse, got {other:?}"),
        }
        match &canvas.calls[1] {
            DrawCall::DrawText { text, style, pos } => {
                assert_eq!(text, "7", "digit label");
                // Luminance pick: dark-blue fill → white text.
                assert_eq!(style.color, Color::rgb(255, 255, 255));
                // Font size scales with diameter (existing behaviour).
                assert!((style.size - 28.0 * 0.6).abs() < f32::EPSILON);
                // The anchor is the bubble centre minus half the
                // measured label size — the digit's top-left, so the
                // glyph block sits centred in the disc. (Font 16.8 →
                // mock width 10.08, height 20.16 → anchor
                // (44.96, 39.92).)
                let font = 28.0 * 0.6;
                assert_eq!(*pos, centered_anchor(Point::new(50.0, 50.0), 1.0, font));
            }
            other => panic!("expected DrawText, got {other:?}"),
        }
    }

    /// A two-digit label centres the same way: the anchor shifts left
    /// by the extra digit's width, half of which is 10.08 − 5.04 =
    /// 5.04 px here. Pins that centering is measured, not a constant
    /// offset — the anchor tracks the label's own width.
    #[test]
    fn render_ann_number_centers_two_digit_label() {
        let mut canvas = MockCanvas::default();
        let source = image::RgbaImage::new(1, 1);
        let pos = Point::new(50.0, 50.0);
        render_ann(
            &mut canvas,
            &Annotation::Number {
                pos,
                n: 12,
                color: Color::rgb(0, 0, 255),
                diameter: 28.0,
            },
            &source,
        );
        match &canvas.calls[1] {
            DrawCall::DrawText {
                text, pos: anchor, ..
            } => {
                assert_eq!(text, "12", "two-digit label");
                let font = 28.0 * 0.6;
                // Mock width for 2 chars: 2 · 16.8 · 0.6 = 20.16 →
                // anchor x = 50 − 10.08 = 39.92.
                assert_eq!(*anchor, centered_anchor(pos, 2.0, font));
            }
            other => panic!("expected DrawText, got {other:?}"),
        }
    }

    /// Centering scales with the diameter: at 64 px the font is 38.4,
    /// so the mock-measured block is 23.04 × 46.08 and the anchor
    /// lands at (38.48, 26.96) — not the 28 px-diameter offset. Pins
    /// that the fix holds at non-default sizes (the size control this
    /// effort adds to the panel).
    #[test]
    fn render_ann_number_centers_at_scaled_diameter() {
        let mut canvas = MockCanvas::default();
        let source = image::RgbaImage::new(1, 1);
        let pos = Point::new(50.0, 50.0);
        render_ann(
            &mut canvas,
            &Annotation::Number {
                pos,
                n: 9,
                color: Color::rgb(255, 0, 0),
                diameter: 64.0,
            },
            &source,
        );
        match &canvas.calls[1] {
            DrawCall::DrawText {
                text, pos: anchor, ..
            } => {
                assert_eq!(text, "9", "digit label");
                let font = 64.0 * 0.6;
                assert_eq!(*anchor, centered_anchor(pos, 1.0, font));
            }
            other => panic!("expected DrawText, got {other:?}"),
        }
    }

    /// On a light bubble fill (yellow), the digit text is black rather
    /// than white. Pins the contrast pick on the other side of the
    /// luminance threshold so a regression that hardcodes white text
    /// fails loudly.
    #[test]
    fn render_ann_number_picks_black_text_on_light_fill() {
        let mut canvas = MockCanvas::default();
        let source = image::RgbaImage::new(1, 1);
        render_ann(
            &mut canvas,
            &Annotation::Number {
                pos: Point::new(10.0, 10.0),
                n: 1,
                color: Color::rgb(255, 255, 0), // yellow → black text
                diameter: 20.0,
            },
            &source,
        );
        match canvas.calls.iter().find_map(|c| match c {
            DrawCall::DrawText { style, .. } => Some(style.color),
            _ => None,
        }) {
            Some(c) => assert_eq!(c, Color::rgb(0, 0, 0), "black text on yellow bubble"),
            None => panic!("expected a DrawText call"),
        }
    }
}
