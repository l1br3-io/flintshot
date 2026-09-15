//! egui implementation of the [`Canvas`] trait.
//!
//! [`Canvas`]: flintshot_core::render::Canvas

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use flintshot_core::{
    arrow_barbs,
    render::Canvas,
    text::{FontFamily, FontStyle, FontWeight, TextAlign, TextStyle},
    Color, Point, Rect, Stroke,
};

/// egui-side texture handle, returned by [`EguiCanvas::upload_image`].
#[derive(Clone)]
pub struct EguiTexture(pub egui::TextureHandle);

/// The full image-physical ↔ screen-logical view transform: the
/// canvas panel rect, the display's pixels-per-point, and the
/// fit-to-view scale factor.
///
/// One shared mapping every coordinate translation goes through
/// (editor-capture-ux Task 003; the GOTCHA it exists to prevent: a
/// site hand-rolling its own `ppp` division drifts from the others
/// and corrupts input hit-testing or chrome placement under scale).
/// The mapping sites today:
///
/// - the canvas `draw_*` path (image px → screen-logical paint),
/// - the editor shell's pointer translation
///   (`logical_to_image`: screen → image px for the dispatcher),
/// - the chrome paint sites — magnifier, move chrome, selected
///   chrome, selection-phase mask — and the magnifier's cursor
///   anchor (`image_to_logical`),
/// - text measurement back into image px (`image_len`).
///
/// `scale` is 1.0 in today's image-sized windows (the transform
/// lands byte-identical); Task 004 computes the fit and letterboxes
/// by adjusting the origin.
#[derive(Clone, Copy, Debug)]
pub struct CanvasView {
    /// Canvas panel rect in screen-logical px; `rect.min` is the
    /// image origin's on-screen anchor. The magnifier also clamps
    /// against this rect.
    rect: egui::Rect,
    /// Display pixels-per-point.
    ppp: f32,
    /// Fit-to-view scale (image px → on-screen physical px):
    /// `x_logical = x_img * scale / ppp + origin`. 1.0 = 1:1.
    scale: f32,
}

impl CanvasView {
    /// A scale-1.0 view over a full panel rect (the editor shell's
    /// pointer/chrome sites and the magnifier).
    #[must_use]
    pub(crate) fn over(rect: egui::Rect, ppp: f32) -> Self {
        Self {
            rect,
            ppp,
            scale: 1.0,
        }
    }

    /// A scale-1.0 view anchored at an origin with no meaningful
    /// bounds (the canvas's own draw path — it never clamps).
    #[must_use]
    pub(crate) fn at_origin(origin: egui::Pos2, ppp: f32) -> Self {
        Self::over(
            egui::Rect::from_min_size(origin, egui::vec2(f32::INFINITY, f32::INFINITY)),
            ppp,
        )
    }

    /// Set the fit scale (builder form — the shell computes the fit
    /// then threads the view everywhere).
    #[must_use]
    pub(crate) fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    /// The canvas panel rect in screen-logical px (magnifier
    /// clamping).
    pub(crate) fn rect(&self) -> egui::Rect {
        self.rect
    }

    /// The display pixels-per-point (screen-space widget sizing —
    /// the magnifier's inset).
    pub(crate) fn ppp(&self) -> f32 {
        self.ppp
    }

    /// Image-physical px → screen-logical px (chrome placement).
    pub(crate) fn image_to_logical(&self, p: Point) -> egui::Pos2 {
        let k = self.scale / self.ppp;
        egui::pos2(p.x * k + self.rect.min.x, p.y * k + self.rect.min.y)
    }

    /// Screen-logical px → image-physical px (pointer mapping).
    pub(crate) fn logical_to_image(&self, p: egui::Pos2) -> Point {
        let k = self.ppp / self.scale;
        let local = p - self.rect.min;
        Point::new(local.x * k, local.y * k)
    }

    /// Image-physical rect → screen-logical rect.
    pub(crate) fn image_rect_to_logical(&self, r: Rect) -> egui::Rect {
        let k = self.scale / self.ppp;
        egui::Rect::from_min_size(
            self.image_to_logical(r.min),
            egui::vec2(r.width() * k, r.height() * k),
        )
    }

    /// Image-px length → screen-logical length (stroke widths, font
    /// sizes — WYSIWYG relative to the fitted image).
    pub(crate) fn len(&self, image_px: f32) -> f32 {
        image_px * self.scale / self.ppp
    }

    /// Screen-logical length → image-px length (the measure path's
    /// inverse of [`len`](Self::len)).
    pub(crate) fn image_len(&self, logical: f32) -> f32 {
        logical * self.ppp / self.scale
    }
}

/// Pure fit-to-view computation: the scale that fits an image
/// `(w, h)` (physical px) inside a viewport `(w, h)` (logical px),
/// capped at 1.0 — never upscales. Degenerate inputs (any zero
/// dimension) yield no scale; callers keep the previous fit (or 1.0)
/// rather than collapsing the canvas.
// Wired into the viewport flip in editor-capture-ux Task 004; lands
// tested ahead of it here (Task 003's byte-identical bar).
#[cfg_attr(not(test), allow(dead_code))]
#[must_use]
pub(crate) fn fit_scale(img: (f32, f32), viewport: (f32, f32)) -> Option<f32> {
    let (w, h) = img;
    let (vw, vh) = viewport;
    if w <= 0.0 || h <= 0.0 || vw <= 0.0 || vh <= 0.0 {
        return None;
    }
    Some((vw / w).min(vh / h).min(1.0))
}

/// egui implementation of the [`Canvas`] trait.
///
/// Holds the [`egui::Context`] (captured at construction time, stable
/// across the editor's lifetime) and a per-frame [`egui::Painter`]
/// that the shell binds before each `render()` call. Background
/// blits and annotation primitives both go through the painter.
///
/// Phase 1 Task 8 implemented `draw_rect_outline` / `draw_rect_filled`;
/// Task 11 added `draw_line` / `draw_arrow`. Phase 2b added
/// `draw_ellipse` (outline + optional fill via `EllipseShape`) and
/// `draw_polyline` (one `PathShape::line` for Marker/Pencil freehand
/// strokes — see ADR-0022 on why per-segment `draw_line` can't carry a
/// translucent marker without stamp-compounding toward opaque).
pub struct EguiCanvas {
    /// Stable toolkit context, captured by reference at construction.
    ctx: egui::Context,
    /// Bound per-frame by the embedding shell. `None` outside a frame.
    painter: Option<egui::Painter>,
    /// Top-left of the canvas panel in screen-logical px (captured
    /// by [`bind_painter`](Self::bind_painter) from the painter's
    /// `clip_rect`). Added to every draw call so the (0, 0) of
    /// canvas-physical space lands at the panel's top-left rather
    /// than the window's — otherwise the toolbar covers the top of
    /// the image and any annotation drawn in the topmost rows.
    origin: egui::Pos2,
    /// Optional annotation clip in screen-logical px, set via
    /// [`set_clip`](Self::set_clip). Constrains every draw except
    /// the background blit (Phase 5 overlay's region confinement).
    clip: Option<egui::Rect>,
    /// Fit-to-view scale factor (image px → on-screen physical px).
    /// 1.0 today (image-sized windows); Task 004 sets the fitted
    /// value. Every image→logical mapping in this canvas and in the
    /// shell's pointer/chrome sites goes through it via
    /// [`CanvasView`].
    scale: f32,
    /// Cached textures for [`draw_image_region`](Self::draw_image_region)
    /// (the blur path), keyed by pixel content. Phase 5 Task 008:
    /// this path used to re-upload a fresh GPU texture every frame
    /// per blur — tolerable in the editor window, come due in the
    /// overlay, whose fullscreen hover magnifier repaints on every
    /// pointer motion.
    region_textures: HashMap<RegionTexKey, egui::TextureHandle>,
    /// Keys drawn this frame; [`bind_painter`](Self::bind_painter)
    /// evicts unmarked entries, so undrawn blurs (undo, crop) free
    /// their GPU textures and long sessions can't leak.
    region_used: HashSet<RegionTexKey>,
}

/// Identity of a region-blit's pixels: a content hash plus the
/// dimensions (which factor the byte stream into a width/height).
/// Deliberately **not** the dest rect or the buffer's address — a
/// dest-only key serves a stale blur after undo-and-redraw at the
/// same rect, and an address key breaks when the core `BlurCache`
/// re-allocates (crop clears it; allocator address reuse would alias
/// different pixels at the same address). The hash is a read-only
/// pass over the buffer and still costs far less than the
/// premultiply+copy+GPU upload it prevents.
#[derive(Clone, Hash, PartialEq, Eq)]
struct RegionTexKey {
    hash: u64,
    width: u32,
    height: u32,
}

impl EguiCanvas {
    /// Construct a canvas bound to an [`egui::Context`]. The shell
    /// must call [`EguiCanvas::bind_painter`] each frame before
    /// invoking [`flintshot_core::render::render`].
    #[must_use]
    pub fn new(ctx: egui::Context) -> Self {
        Self {
            ctx,
            painter: None,
            origin: egui::Pos2::ZERO,
            clip: None,
            scale: 1.0,
            region_textures: HashMap::new(),
            region_used: HashSet::new(),
        }
    }

    /// Set the fit-to-view scale factor (see [`Self::scale`]). The
    /// shell computes the fit ([`fit_scale`]) and calls this whenever
    /// the image or viewport changes; 1.0 restores the image-sized
    /// mapping.
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
    }

    /// The current fit-to-view scale factor.
    #[must_use]
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// The view anchored at the canvas's bound origin (the draw
    /// path's own mapping).
    fn view(&self) -> CanvasView {
        CanvasView::at_origin(self.origin, self.ppp()).with_scale(self.scale)
    }

    /// The view over the shell's cached canvas rect — the pointer /
    /// chrome sites' mapping (one source of truth for the scale).
    #[must_use]
    pub(crate) fn view_over(&self, rect: egui::Rect) -> CanvasView {
        CanvasView::over(rect, self.ppp()).with_scale(self.scale)
    }

    /// Bind the per-frame [`egui::Painter`]. Called by the shell from
    /// inside its `update()` body; subsequent `draw_*` calls dispatch
    /// through this painter until the next bind.
    ///
    /// Also the frame boundary for the region-texture cache: entries
    /// not drawn since the last bind are evicted here, dropping their
    /// [`egui::TextureHandle`]s and freeing the GPU textures.
    pub fn bind_painter(&mut self, painter: &egui::Painter) {
        self.region_textures
            .retain(|key, _| self.region_used.contains(key));
        self.region_used.clear();
        self.origin = painter.clip_rect().min;
        self.painter = Some(painter.clone());
    }

    /// Constrain subsequent **annotation** draws to `clip`
    /// (canvas-physical px, normalized like every other canvas-space
    /// rect); `None` clears the constraint (the default).
    ///
    /// The Phase 5 overlay clips to the active selection so strokes
    /// dragged past the region edge stop painting there (Flameshot
    /// confinement). The background blit
    /// ([`draw_image`](Self::draw_image)) deliberately ignores the
    /// clip — the frozen screen outside the region must stay visible
    /// under the overlay's dim mask. The editor window never sets a
    /// clip, so its rendering is byte-identical to pre-Phase 5.
    pub fn set_clip(&mut self, clip: Option<Rect>) {
        self.clip = clip.map(|r| self.view().image_rect_to_logical(r.normalized()));
    }

    /// Physical-to-logical pixel conversion factor for the bound context.
    fn ppp(&self) -> f32 {
        self.ctx.pixels_per_point()
    }

    /// The painter annotation draws go through: the bound painter,
    /// clip-constrained when [`set_clip`](Self::set_clip) is active.
    /// `with_clip_rect` intersects with the panel clip, so an
    /// over-wide clip can't escape the canvas.
    fn clip_painter(&self) -> Option<egui::Painter> {
        let painter = self.painter.as_ref()?;
        Some(match self.clip {
            Some(clip) => painter.with_clip_rect(clip),
            None => painter.clone(),
        })
    }
}

/// Convert a [`Color`] to [`egui::Color32`].
///
/// Both are 8-bit sRGB; we use `from_rgba_unmultiplied` so straight-alpha
/// source data stays straight (egui's `Color32` stores premultiplied
/// internally but the helper handles the conversion).
fn to_egui_color(c: Color) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

/// Convert a [`Stroke`] to an [`egui::Stroke`] through the view:
/// the width is an image-px length and scales with the fit (WYSIWYG
/// relative to the fitted image).
fn to_egui_stroke(stroke: Stroke, view: CanvasView) -> egui::Stroke {
    egui::Stroke::new(view.len(stroke.width), to_egui_color(stroke.color))
}

impl Canvas for EguiCanvas {
    type Image = EguiTexture;

    /// Upload `img` into egui's texture manager with `Nearest`
    /// filtering so screenshot pixels stay sharp under any scale.
    /// The returned handle is owned by the caller (the shell caches
    /// it across frames per [`flintshot_core::render::render`]'s
    /// `&mut Option<C::Image>` contract).
    fn upload_image(&mut self, img: &image::RgbaImage) -> flintshot_core::Result<Self::Image> {
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [img.width() as usize, img.height() as usize],
            img.as_raw(),
        );
        let handle = self.ctx.load_texture(
            "flintshot-background",
            color_image,
            egui::TextureOptions::NEAREST,
        );
        Ok(EguiTexture(handle))
    }

    /// Blit a previously-uploaded texture into `rect` (physical px).
    /// Converts physical → logical by dividing by `pixels_per_point`
    /// so `HiDPI` displays get the right on-screen size while the source
    /// texture stays 1:1 with the captured framebuffer.
    ///
    /// No-op when no painter is bound (i.e. outside a frame). This
    /// shouldn't happen in practice — the shell always binds before
    /// calling `render()` — but defending against it keeps `draw_*`
    /// infallible, matching the [`Canvas`] trait signature.
    fn draw_image(&mut self, img: &Self::Image, rect: Rect) {
        let Some(painter) = &self.painter else {
            return;
        };
        let dest = self.view().image_rect_to_logical(rect);
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        painter.image(img.0.id(), dest, uv, egui::Color32::WHITE);
    }

    #[allow(
        clippy::cast_possible_truncation,
        reason = "image dims are u32; the usize casts are widening on every supported platform"
    )]
    fn draw_image_region(&mut self, img: &image::RgbaImage, dest: Rect) {
        // A 0×0 source image creates a zero-dimension wgpu texture,
        // which panics inside `Device::create_texture` with
        // "Dimension X is zero". The core blur render path guards
        // against this before calling, but defend here too so any
        // future caller (or partial edge-case the core guard misses)
        // can't take down the editor session.
        if img.width() == 0 || img.height() == 0 {
            return;
        }
        let Some(painter) = self.clip_painter() else {
            return;
        };
        // Content-keyed cache (Phase 5 Task 008): identical pixels
        // re-blit the existing GPU texture instead of paying the
        // premultiply+copy of `ColorImage::from_rgba_unmultiplied`
        // plus a `load_texture` allocation and upload every frame.
        // The core `BlurCache` hands committed blurs the same buffer
        // each frame, so this hits for exactly the static case; a
        // pending (in-flight) blur drag changes content per frame and
        // misses, uploading fresh — the same behaviour as before, now
        // without accumulating (the stale entry evicts a frame later).
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        img.as_raw().hash(&mut hasher);
        let key = RegionTexKey {
            hash: hasher.finish(),
            width: img.width(),
            height: img.height(),
        };
        self.region_used.insert(key.clone());
        let texture = if let Some(handle) = self.region_textures.get(&key) {
            handle.clone()
        } else {
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [img.width() as usize, img.height() as usize],
                img.as_raw(),
            );
            // Unique name per content: egui's texture manager keys by
            // name, and two different blurs drawn in the same frame
            // must not clobber each other's entry.
            let name = format!("flintshot-region-{}", key.hash);
            let handle = self
                .ctx
                .load_texture(name, color_image, egui::TextureOptions::NEAREST);
            self.region_textures.insert(key.clone(), handle.clone());
            handle
        };
        let dest_rect = self.view().image_rect_to_logical(dest);
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        painter.image(texture.id(), dest_rect, uv, egui::Color32::WHITE);
    }

    fn draw_rect_outline(&mut self, rect: Rect, stroke: &Stroke) {
        let Some(painter) = self.clip_painter() else {
            return;
        };
        let view = self.view();
        // StrokeKind::Inside keeps the stroke inside the dragged rect
        // bounds so "what you dragged is what you get" — the visible
        // outline's outer edge coincides with the rect.
        painter.rect_stroke(
            view.image_rect_to_logical(rect),
            egui::CornerRadius::ZERO,
            to_egui_stroke(*stroke, view),
            egui::StrokeKind::Inside,
        );
    }

    fn draw_rect_filled(&mut self, rect: Rect, color: Color) {
        let Some(painter) = self.clip_painter() else {
            return;
        };
        painter.rect_filled(
            self.view().image_rect_to_logical(rect),
            egui::CornerRadius::ZERO,
            to_egui_color(color),
        );
    }

    fn draw_ellipse(&mut self, rect: Rect, stroke: &Stroke, fill: Option<Color>) {
        let Some(painter) = self.clip_painter() else {
            return;
        };
        let view = self.view();
        let er = view.image_rect_to_logical(rect);
        painter.add(egui::epaint::EllipseShape {
            center: er.center(),
            radius: er.size() / 2.0,
            fill: fill.map_or(egui::Color32::TRANSPARENT, to_egui_color),
            stroke: to_egui_stroke(*stroke, view),
            angle: 0.0,
        });
    }

    fn draw_line(&mut self, a: Point, b: Point, stroke: &Stroke) {
        let Some(painter) = self.clip_painter() else {
            return;
        };
        let view = self.view();
        painter.line_segment(
            [view.image_to_logical(a), view.image_to_logical(b)],
            to_egui_stroke(*stroke, view),
        );
    }

    fn draw_polyline(&mut self, points: &[Point], stroke: &Stroke) {
        let Some(painter) = self.clip_painter() else {
            return;
        };
        // One `PathShape::line` (a single tessellated strip) renders the
        // whole freehand stroke with per-pixel coverage computed once,
        // so a translucent marker doesn't compound at the segment joints
        // the way per-segment `line_segment` calls would (ADR-0022).
        if points.len() < 2 {
            return;
        }
        let view = self.view();
        let path: Vec<egui::Pos2> = points.iter().map(|&p| view.image_to_logical(p)).collect();
        painter.add(egui::epaint::PathShape::line(
            path,
            to_egui_stroke(*stroke, view),
        ));
    }

    fn draw_arrow(&mut self, a: Point, b: Point, stroke: &Stroke, head_size: f32) {
        let Some(painter) = self.clip_painter() else {
            return;
        };
        let view = self.view();
        let a_l = view.image_to_logical(a);
        let b_l = view.image_to_logical(b);
        let egui_stroke = to_egui_stroke(*stroke, view);
        painter.line_segment([a_l, b_l], egui_stroke);
        // `arrow_barbs` returns image-px endpoints (head_size is
        // defined in image px); mapping them through the view yields
        // the right on-screen length at any `pixels_per_point` and fit
        // scale, matching what `ImageCanvas` bakes into the saved PNG
        // (ADR-0015 WYSIWYG).
        let [left, right] = arrow_barbs(a, b, head_size);
        painter.line_segment([b_l, view.image_to_logical(left)], egui_stroke);
        painter.line_segment([b_l, view.image_to_logical(right)], egui_stroke);
    }

    fn draw_text(&mut self, pos: Point, text: &str, style: &TextStyle) {
        let Some(painter) = self.clip_painter() else {
            return;
        };
        let view = self.view();
        // `style.size` is image px; egui wants logical px for FontId,
        // and the fit scale keeps text WYSIWYG with the fitted image.
        let size = view.len(style.size);
        // egui ships two built-in families; Serif falls back to
        // Proportional (Sans) until a serif face is bundled — see ADR.
        let family = match style.family {
            FontFamily::Monospace => egui::FontFamily::Monospace,
            _ => egui::FontFamily::Proportional,
        };
        let format = egui::text::TextFormat {
            font_id: egui::FontId { size, family },
            color: to_egui_color(style.color),
            // Background is drawn explicitly below (solid rect) so the
            // save path — which can only fill a rect — matches WYSIWYG.
            background: egui::Color32::TRANSPARENT,
            italics: matches!(style.style, FontStyle::Italic),
            ..egui::text::TextFormat::default()
        };
        let mut job = egui::text::LayoutJob {
            // No word-wrap (only `\n` breaks lines), matching the save path.
            wrap: egui::text::TextWrapping {
                max_width: f32::INFINITY,
                ..egui::text::TextWrapping::default()
            },
            halign: match style.align {
                TextAlign::Center => egui::Align::Center,
                TextAlign::Right => egui::Align::RIGHT,
                _ => egui::Align::LEFT,
            },
            ..egui::text::LayoutJob::default()
        };
        job.append(text, 0.0, format);
        let galley = self.ctx.fonts_mut(|f| f.layout_job(job));
        let anchor = view.image_to_logical(pos);
        let galley_size = galley.size();
        // Solid background rect behind the laid-out block.
        if let Some(bg) = style.background {
            let r = egui::Rect::from_min_size(anchor, galley_size);
            painter.rect_filled(r, egui::CornerRadius::ZERO, to_egui_color(bg));
        }
        let color = to_egui_color(style.color);
        // Faux outline: redraw the galley at the four cardinal offsets in
        // the outline colour, then the fill pass centres the glyphs.
        // `galley_with_override_text_color` repaints the cached galley
        // in any colour without re-laying-out.
        if let Some(outline) = style.outline {
            let oc = to_egui_color(outline.color);
            let w = view.len(outline.width);
            for (dx, dy) in [(w, 0.0), (-w, 0.0), (0.0, w), (0.0, -w)] {
                painter.galley_with_override_text_color(
                    anchor + egui::vec2(dx, dy),
                    galley.clone(),
                    oc,
                );
            }
        }
        painter.galley_with_override_text_color(anchor, galley.clone(), color);
        // Faux bold: egui has no weight API (its `strong()` only changes
        // colour), so thicken by overdrawing at a sub-pixel x offset.
        // The save path uses a real bold face via fontdb — minor
        // WYSIWYG difference, documented in the ADR.
        if matches!(style.weight, FontWeight::Bold) {
            painter.galley_with_override_text_color(
                anchor + egui::vec2(view.len(0.5), 0.0),
                galley,
                color,
            );
        }
    }

    fn measure_text(&self, text: &str, style: &TextStyle) -> flintshot_core::text::TextMetrics {
        let view = self.view();
        // `style.size` is image px; egui wants logical px for FontId.
        let font_id = egui::FontId::proportional(view.len(style.size));
        // Laying out the text is the authoritative measurement path —
        // it consults the same font + shaping that `draw_text` uses, so
        // the caret lands exactly at the visible end of the run. egui's
        // galley already splits on `\n`, so multi-line runs get a row
        // per line for free. `fonts_mut` caches the galley.
        let galley = self.ctx.fonts_mut(|f| {
            f.layout(
                text.to_owned(),
                font_id,
                egui::Color32::PLACEHOLDER,
                f32::INFINITY,
            )
        });
        // Galley size is logical px; convert back to image px — the
        // canvas-physical coordinate space the render path uses —
        // inverting the fit scale so metrics track the drawn glyphs.
        let size = galley.size();
        let width = view.image_len(size.x);
        // The caret sits at the end of the last line; `galley.rows`'s
        // final entry holds that line's width (empty after a trailing
        // `\n`, which is correct — the caret drops to a fresh line).
        let last_line_width = galley
            .rows
            .last()
            .map_or(0.0, |row| view.image_len(row.rect().width()));
        flintshot_core::text::TextMetrics {
            width,
            height: view.image_len(size.y),
            last_line_width,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flintshot_core::{Color, Rect, Stroke};

    // --- Fit-to-view (editor-capture-ux Task 003) ---

    #[test]
    fn fit_scale_scales_large_images_down_on_the_binding_axis() {
        // Wide image, viewport exactly half: x binds.
        assert_eq!(fit_scale((2000.0, 1000.0), (1000.0, 500.0)), Some(0.5));
        // Tall image, viewport exactly half: y binds.
        assert_eq!(fit_scale((1000.0, 2000.0), (500.0, 1000.0)), Some(0.5));
        // Both axes shrink: the smaller ratio wins.
        assert_eq!(fit_scale((2000.0, 2000.0), (500.0, 1000.0)), Some(0.25));
    }

    #[test]
    fn fit_scale_never_upscales() {
        // A tiny image in a roomy viewport renders 1:1, not stretched.
        assert_eq!(fit_scale((100.0, 50.0), (1000.0, 1000.0)), Some(1.0));
        // Exactly viewport-sized: 1.0, not 1.0-epsilon games.
        assert_eq!(fit_scale((500.0, 300.0), (500.0, 300.0)), Some(1.0));
    }

    #[test]
    fn fit_scale_degenerate_inputs_yield_no_scale() {
        assert_eq!(fit_scale((0.0, 100.0), (100.0, 100.0)), None);
        assert_eq!(fit_scale((100.0, 0.0), (100.0, 100.0)), None);
        assert_eq!(fit_scale((100.0, 100.0), (0.0, 100.0)), None);
        assert_eq!(fit_scale((100.0, 100.0), (100.0, 0.0)), None);
    }

    #[test]
    fn fit_scale_aspect_extreme_fits_the_binding_axis() {
        // 10000×10 into 100×100: x binds hard, y is nearly empty.
        assert_eq!(fit_scale((10_000.0, 10.0), (100.0, 100.0)), Some(0.01));
        // 10×10000 into 100×100: y binds.
        assert_eq!(fit_scale((10.0, 10_000.0), (100.0, 100.0)), Some(0.01));
    }

    /// A view over a panel at logical (10, 20) sized 400×300, ppp 2,
    /// fit-scale 0.5 — the arbitrary-transform fixture.
    fn scaled_view() -> CanvasView {
        CanvasView::over(
            egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(400.0, 300.0)),
            2.0,
        )
        .with_scale(0.5)
    }

    #[test]
    fn view_maps_by_the_fit_formula() {
        let view = scaled_view();
        // x_logical = x_img * scale / ppp + origin.x
        assert_eq!(
            view.image_to_logical(Point::new(100.0, 200.0)),
            egui::pos2(35.0, 70.0)
        );
        assert_eq!(
            view.image_rect_to_logical(Rect::from_xywh(100.0, 200.0, 40.0, 20.0)),
            egui::Rect::from_min_size(egui::pos2(35.0, 70.0), egui::vec2(10.0, 5.0))
        );
    }

    #[test]
    fn pointer_mapping_round_trips_under_scale() {
        // logical → image px → logical: the pointer translation and
        // its inverse must agree at an arbitrary scale.
        let view = scaled_view();
        for l in [
            egui::pos2(10.0, 20.0),
            egui::pos2(210.0, 170.0),
            egui::pos2(409.5, 319.5),
        ] {
            let round = view.image_to_logical(view.logical_to_image(l));
            assert!((round - l).length() < 1e-3, "{l} round-trips, got {round}");
        }
    }

    #[test]
    fn chrome_mapping_round_trips_under_scale() {
        // image px → logical → image: the chrome placement path.
        let view = scaled_view();
        for p in [
            Point::new(0.0, 0.0),
            Point::new(512.0, 384.0),
            Point::new(1599.0, 1199.0),
        ] {
            let round = view.logical_to_image(view.image_to_logical(p));
            assert!(
                (round.x - p.x).abs() < 1e-3 && (round.y - p.y).abs() < 1e-3,
                "{p:?} round-trips, got {round:?}"
            );
        }
    }

    #[test]
    fn view_lengths_scale_and_invert() {
        let view = scaled_view();
        // 10 image px → 10 * 0.5 / 2.0 logical px on screen.
        assert!((view.len(10.0) - 2.5).abs() < 1e-6);
        // And back: a measured 2.5 logical px is 10 image px.
        assert!((view.image_len(2.5) - 10.0).abs() < 1e-6);
    }

    #[test]
    fn identity_view_matches_the_legacy_formula() {
        // At scale 1.0 the mapping is exactly the old
        // `p / ppp + origin` math (this task's byte-identical bar).
        let view = CanvasView::over(
            egui::Rect::from_min_size(egui::pos2(30.0, 40.0), egui::vec2(800.0, 600.0)),
            1.25,
        );
        let p = Point::new(250.0, 125.0);
        assert_eq!(
            view.image_to_logical(p),
            egui::pos2(30.0 + 250.0 / 1.25, 40.0 + 125.0 / 1.25)
        );
        assert!((view.len(9.0) - 9.0 / 1.25).abs() < 1e-6);
    }

    /// `draw_ellipse` emits at least one `egui::Painter` shape — guards
    /// against the Phase 1 regression where the method body was empty.
    #[test]
    fn draw_ellipse_emits_shape() {
        let ctx = egui::Context::default();
        let clip_rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(400.0, 300.0));
        let painter = egui::Painter::new(ctx.clone(), egui::LayerId::debug(), clip_rect);

        let mut canvas = EguiCanvas::new(ctx);
        canvas.bind_painter(&painter);
        canvas.draw_ellipse(
            Rect::from_xywh(10.0, 10.0, 100.0, 60.0),
            &Stroke::new(Color::rgb(0, 0, 255), 2.0),
            Some(Color::rgb(0, 255, 0)),
        );

        let mut count = 0;
        let mut has_ellipse = false;
        painter.for_each_shape(|cs| {
            count += 1;
            if matches!(cs.shape, egui::Shape::Ellipse(_)) {
                has_ellipse = true;
            }
        });

        assert!(count > 0, "draw_ellipse must emit at least one shape");
        assert!(
            has_ellipse,
            "draw_ellipse must emit an Ellipse variant, not a no-op"
        );
    }

    /// `draw_ellipse` with no painter bound is a safe no-op (matching
    /// the other draw methods' infallible contract).
    #[test]
    fn draw_ellipse_without_painter_is_noop() {
        let ctx = egui::Context::default();
        let mut canvas = EguiCanvas::new(ctx);
        canvas.draw_ellipse(
            Rect::from_xywh(0.0, 0.0, 50.0, 50.0),
            &Stroke::new(Color::RED, 2.0),
            None,
        );
    }

    // --- Annotation clipping (Phase 5 Task 005) ---
    //
    // The overlay confines annotation draws to the selection region;
    // egui records the clip with every shape, so the tests assert the
    // `ClippedShape::clip_rect` each draw lands with. A default
    // context runs at ppp 1.0 and the debug painter's clip starts at
    // (0, 0), so canvas-physical == screen-logical in these fixtures.

    /// A canvas bound to a 400×300 painter at (0, 0), ppp 1.0 — the
    /// shared fixture for the clip tests.
    fn bound_canvas() -> (egui::Context, egui::Painter, EguiCanvas) {
        let ctx = egui::Context::default();
        let clip_rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(400.0, 300.0));
        let painter = egui::Painter::new(ctx.clone(), egui::LayerId::debug(), clip_rect);
        let mut canvas = EguiCanvas::new(ctx.clone());
        canvas.bind_painter(&painter);
        (ctx, painter, canvas)
    }

    /// The clip rects recorded alongside every shape drawn so far.
    fn recorded_clips(painter: &egui::Painter) -> Vec<egui::Rect> {
        let mut clips = Vec::new();
        painter.for_each_shape(|cs| clips.push(cs.clip_rect));
        clips
    }

    #[test]
    fn set_clip_constrains_annotation_draws() {
        let (_ctx, painter, mut canvas) = bound_canvas();
        canvas.set_clip(Some(Rect::from_xywh(50.0, 50.0, 100.0, 100.0)));
        canvas.draw_line(
            Point::new(0.0, 0.0),
            Point::new(300.0, 300.0),
            &Stroke::new(Color::RED, 2.0),
        );
        canvas.draw_rect_filled(
            Rect::from_xywh(0.0, 0.0, 400.0, 300.0),
            Color::rgb(0, 0, 255),
        );
        assert_eq!(
            recorded_clips(&painter),
            vec![
                egui::Rect::from_min_size(egui::pos2(50.0, 50.0), egui::vec2(100.0, 100.0)),
                egui::Rect::from_min_size(egui::pos2(50.0, 50.0), egui::vec2(100.0, 100.0)),
            ],
            "every annotation draw must record the set clip"
        );
    }

    #[test]
    fn background_blit_ignores_clip() {
        let (_ctx, painter, mut canvas) = bound_canvas();
        let img = image::RgbaImage::from_raw(8, 8, vec![255; 8 * 8 * 4]).expect("w*h*4 buffer");
        let tex = canvas.upload_image(&img).expect("upload succeeds");
        canvas.set_clip(Some(Rect::from_xywh(50.0, 50.0, 100.0, 100.0)));
        canvas.draw_image(&tex, Rect::from_xywh(0.0, 0.0, 400.0, 300.0));
        assert_eq!(
            recorded_clips(&painter),
            vec![egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(400.0, 300.0)
            )],
            "the background blit must stay unclipped so the frozen screen outside the region stays visible"
        );
    }

    #[test]
    fn blur_region_blit_honors_clip() {
        let (_ctx, painter, mut canvas) = bound_canvas();
        let img = image::RgbaImage::from_raw(8, 8, vec![255; 8 * 8 * 4]).expect("w*h*4 buffer");
        canvas.set_clip(Some(Rect::from_xywh(10.0, 10.0, 50.0, 50.0)));
        canvas.draw_image_region(&img, Rect::from_xywh(10.0, 10.0, 8.0, 8.0));
        assert_eq!(
            recorded_clips(&painter),
            vec![egui::Rect::from_min_size(
                egui::pos2(10.0, 10.0),
                egui::vec2(50.0, 50.0)
            )],
            "blur pixels must not paint outside the region"
        );
    }

    #[test]
    fn clearing_clip_restores_panel_clipping() {
        let (_ctx, painter, mut canvas) = bound_canvas();
        canvas.set_clip(Some(Rect::from_xywh(50.0, 50.0, 100.0, 100.0)));
        canvas.set_clip(None);
        canvas.draw_line(
            Point::new(0.0, 0.0),
            Point::new(10.0, 10.0),
            &Stroke::new(Color::RED, 2.0),
        );
        assert_eq!(
            recorded_clips(&painter),
            vec![egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(400.0, 300.0)
            )],
            "no clip set → shapes record the panel clip"
        );
    }

    #[test]
    fn set_clip_normalizes_flipped_rect() {
        let (_ctx, painter, mut canvas) = bound_canvas();
        canvas.set_clip(Some(Rect::new(
            Point::new(150.0, 150.0),
            Point::new(50.0, 50.0),
        )));
        canvas.draw_line(
            Point::new(0.0, 0.0),
            Point::new(10.0, 10.0),
            &Stroke::new(Color::RED, 2.0),
        );
        assert_eq!(
            recorded_clips(&painter),
            vec![egui::Rect::from_min_size(
                egui::pos2(50.0, 50.0),
                egui::vec2(100.0, 100.0)
            )],
            "a flipped clip rect must normalize before recording"
        );
    }

    // --- Region texture cache (Phase 5 Task 008) ---
    //
    // `draw_image_region` (the blur path) must not re-upload identical
    // pixels every frame. The tests drive the real begin-pass → paint
    // → end-pass flow a backend runs, so each frame's
    // `textures_delta` reflects exactly what a GPU integration would
    // upload / free.

    /// A solid 8×8 RGBA image (all `v`), the stand-in for a cached
    /// blur buffer.
    fn region_image(v: u8) -> image::RgbaImage {
        image::RgbaImage::from_raw(8, 8, vec![v; 8 * 8 * 4]).expect("raw buffer matches w*h*4")
    }

    /// Run one headless frame that region-blits `draws`, returning the
    /// frame's texture delta. `bind_painter` runs every frame — the
    /// same contract both shells uphold — so frame-start eviction is
    /// exercised exactly as in the apps.
    fn region_frame(
        ctx: &egui::Context,
        canvas: &mut EguiCanvas,
        draws: &[(image::RgbaImage, Rect)],
    ) -> egui::TexturesDelta {
        ctx.begin_pass(egui::RawInput::default());
        let clip_rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(400.0, 300.0));
        let painter = egui::Painter::new(ctx.clone(), egui::LayerId::debug(), clip_rect);
        canvas.bind_painter(&painter);
        for (img, dest) in draws {
            canvas.draw_image_region(img, *dest);
        }
        ctx.end_pass().textures_delta
    }

    /// Uploads in a delta, excluding the font atlas (`TextureId::default`).
    fn region_uploads(delta: &egui::TexturesDelta) -> usize {
        delta
            .set
            .iter()
            .filter(|(id, _)| *id != egui::TextureId::default())
            .count()
    }

    #[test]
    fn identical_blur_regions_are_not_reuploaded() {
        let ctx = egui::Context::default();
        let mut canvas = EguiCanvas::new(ctx.clone());
        let draws = [
            (region_image(200), Rect::from_xywh(10.0, 10.0, 8.0, 8.0)),
            (region_image(120), Rect::from_xywh(60.0, 60.0, 8.0, 8.0)),
        ];
        let first = region_frame(&ctx, &mut canvas, &draws);
        assert_eq!(
            region_uploads(&first),
            2,
            "frame 1 uploads one texture per distinct blur"
        );
        let second = region_frame(&ctx, &mut canvas, &draws);
        assert_eq!(
            region_uploads(&second),
            0,
            "frame 2 with identical blurs must upload nothing"
        );
    }

    #[test]
    fn changed_blur_content_is_reuploaded() {
        let ctx = egui::Context::default();
        let mut canvas = EguiCanvas::new(ctx.clone());
        let dest = Rect::from_xywh(10.0, 10.0, 8.0, 8.0);
        let first = region_frame(&ctx, &mut canvas, &[(region_image(200), dest)]);
        assert_eq!(region_uploads(&first), 1);
        // Same dest, same dims, different pixels: a dest-only or
        // pointer-keyed cache would wrongly reuse the old texture.
        let second = region_frame(&ctx, &mut canvas, &[(region_image(60), dest)]);
        assert_eq!(
            region_uploads(&second),
            1,
            "different content at the same dest must re-upload"
        );
    }

    #[test]
    fn undrawn_blur_texture_is_freed() {
        let ctx = egui::Context::default();
        let mut canvas = EguiCanvas::new(ctx.clone());
        let draws = [(region_image(200), Rect::from_xywh(10.0, 10.0, 8.0, 8.0))];
        let first = region_frame(&ctx, &mut canvas, &draws);
        assert_eq!(region_uploads(&first), 1);
        // Eviction runs at bind with the *previous* frame's marks, so a
        // texture survives one grace frame after its last draw and
        // frees on the second — a long session can't leak textures.
        let _ = region_frame(&ctx, &mut canvas, &[]);
        let second = region_frame(&ctx, &mut canvas, &[]);
        assert!(
            !second.free.is_empty(),
            "an undrawn region texture must free once its grace frame passes"
        );
    }

    #[test]
    fn drawn_blur_texture_is_retained() {
        let ctx = egui::Context::default();
        let mut canvas = EguiCanvas::new(ctx.clone());
        let draws = [(region_image(200), Rect::from_xywh(10.0, 10.0, 8.0, 8.0))];
        let _ = region_frame(&ctx, &mut canvas, &draws);
        let second = region_frame(&ctx, &mut canvas, &draws);
        assert!(
            second.free.is_empty(),
            "a still-drawn region texture must not free"
        );
    }
}
