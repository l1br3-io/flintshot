//! egui implementation of the [`Canvas`] trait.
//!
//! [`Canvas`]: flintshot_core::render::Canvas

use flintshot_core::{arrow_barbs, render::Canvas, Color, Point, Rect, Stroke};

/// egui-side texture handle, returned by [`EguiCanvas::upload_image`].
#[derive(Clone)]
pub struct EguiTexture(pub egui::TextureHandle);

/// egui implementation of the [`Canvas`] trait.
///
/// Holds the [`egui::Context`] (captured at construction time, stable
/// across the editor's lifetime) and a per-frame [`egui::Painter`]
/// that the shell binds before each `render()` call. Background
/// blits and annotation primitives both go through the painter.
///
/// Phase 1 Task 8 implemented `draw_rect_outline` / `draw_rect_filled`;
/// Task 11 adds `draw_line` / `draw_arrow`. The remaining primitives
/// (`draw_ellipse`, `draw_text`) stay as stubs until Tasks 12–13.
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
        }
    }

    /// Bind the per-frame [`egui::Painter`]. Called by the shell from
    /// inside its `update()` body; subsequent `draw_*` calls dispatch
    /// through this painter until the next bind.
    pub fn bind_painter(&mut self, painter: &egui::Painter) {
        self.origin = painter.clip_rect().min;
        self.painter = Some(painter.clone());
    }

    /// Physical-to-logical pixel conversion factor for the bound context.
    fn ppp(&self) -> f32 {
        self.ctx.pixels_per_point()
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

/// Convert a physical-px [`Rect`] to a screen-logical [`egui::Rect`]:
/// divide by `pixels_per_point` and translate by the canvas `origin`
/// (the panel's top-left in screen-logical px). Without the origin
/// translation every draw call lands at the window's top-left,
/// leaving the image and any annotation in the topmost rows hidden
/// behind the editor toolbar.
///
/// Callers must pass an already-normalized rect (`min` top-left). The
/// render path normalizes before invoking the canvas methods.
fn to_egui_rect(rect: Rect, ppp: f32, origin: egui::Pos2) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(rect.min.x / ppp + origin.x, rect.min.y / ppp + origin.y),
        egui::vec2(rect.width() / ppp, rect.height() / ppp),
    )
}

/// Convert a physical-px [`Point`] to a screen-logical [`egui::Pos2`].
fn to_egui_pos(p: Point, ppp: f32, origin: egui::Pos2) -> egui::Pos2 {
    egui::pos2(p.x / ppp + origin.x, p.y / ppp + origin.y)
}

/// Convert a [`Stroke`] to an [`egui::Stroke`], scaling the width from
/// physical px to logical px.
fn to_egui_stroke(stroke: Stroke, ppp: f32) -> egui::Stroke {
    egui::Stroke::new(stroke.width / ppp, to_egui_color(stroke.color))
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
        let ppp = self.ctx.pixels_per_point();
        let dest = to_egui_rect(rect, ppp, self.origin);
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        painter.image(img.0.id(), dest, uv, egui::Color32::WHITE);
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "dest.min components are non-negative physical px within image bounds"
    )]
    fn draw_image_region(&mut self, img: &image::RgbaImage, dest: Rect) {
        let Some(painter) = &self.painter else {
            return;
        };
        // A 0×0 source image creates a zero-dimension wgpu texture,
        // which panics inside `Device::create_texture` with
        // "Dimension X is zero". The core blur render path guards
        // against this before calling, but defend here too so any
        // future caller (or partial edge-case the core guard misses)
        // can't take down the editor session.
        if img.width() == 0 || img.height() == 0 {
            return;
        }
        let ppp = self.ppp();
        // Upload the raw pixels to a uniquely-named texture so multiple
        // blur annotations in the same frame don't clobber each other
        // in egui's texture manager. The name incorporates the dest
        // origin; Phase 1 doesn't cache across frames (re-uploads each
        // frame), which is acceptable for the small number of blur
        // annotations a typical screenshot accumulates.
        let name = format!("flintshot-blur-{}-{}", dest.min.x as u32, dest.min.y as u32);
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [img.width() as usize, img.height() as usize],
            img.as_raw(),
        );
        let texture = self
            .ctx
            .load_texture(name, color_image, egui::TextureOptions::NEAREST);
        let dest_rect = to_egui_rect(dest, ppp, self.origin);
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        painter.image(texture.id(), dest_rect, uv, egui::Color32::WHITE);
    }

    fn draw_rect_outline(&mut self, rect: Rect, stroke: &Stroke) {
        let Some(painter) = &self.painter else {
            return;
        };
        let ppp = self.ppp();
        // StrokeKind::Inside keeps the stroke inside the dragged rect
        // bounds so "what you dragged is what you get" — the visible
        // outline's outer edge coincides with the rect.
        painter.rect_stroke(
            to_egui_rect(rect, ppp, self.origin),
            egui::CornerRadius::ZERO,
            to_egui_stroke(*stroke, ppp),
            egui::StrokeKind::Inside,
        );
    }

    fn draw_rect_filled(&mut self, rect: Rect, color: Color) {
        let Some(painter) = &self.painter else {
            return;
        };
        painter.rect_filled(
            to_egui_rect(rect, self.ppp(), self.origin),
            egui::CornerRadius::ZERO,
            to_egui_color(color),
        );
    }

    fn draw_ellipse(&mut self, _rect: Rect, _stroke: &Stroke, _fill: Option<Color>) {}

    fn draw_line(&mut self, a: Point, b: Point, stroke: &Stroke) {
        let Some(painter) = &self.painter else {
            return;
        };
        let ppp = self.ppp();
        painter.line_segment(
            [
                to_egui_pos(a, ppp, self.origin),
                to_egui_pos(b, ppp, self.origin),
            ],
            to_egui_stroke(*stroke, ppp),
        );
    }

    fn draw_arrow(&mut self, a: Point, b: Point, stroke: &Stroke, head_size: f32) {
        let Some(painter) = &self.painter else {
            return;
        };
        let ppp = self.ppp();
        let a_l = to_egui_pos(a, ppp, self.origin);
        let b_l = to_egui_pos(b, ppp, self.origin);
        let egui_stroke = to_egui_stroke(*stroke, ppp);
        painter.line_segment([a_l, b_l], egui_stroke);
        // `arrow_barbs` returns physical-px endpoints (head_size is
        // defined in physical px); converting them to logical via
        // `to_egui_pos` yields the right on-screen length at any
        // `pixels_per_point`, matching what `ImageCanvas` bakes into
        // the saved PNG (ADR-0015 WYSIWYG).
        let [left, right] = arrow_barbs(a, b, head_size);
        painter.line_segment([b_l, to_egui_pos(left, ppp, self.origin)], egui_stroke);
        painter.line_segment([b_l, to_egui_pos(right, ppp, self.origin)], egui_stroke);
    }

    fn draw_text(&mut self, pos: Point, text: &str, size: f32, color: Color) {
        let Some(painter) = &self.painter else {
            return;
        };
        let ppp = self.ppp();
        // `size` is physical px; egui wants logical px for FontId.
        let font_id = egui::FontId::proportional(size / ppp);
        painter.text(
            to_egui_pos(pos, ppp, self.origin),
            egui::Align2::LEFT_TOP,
            text,
            font_id,
            to_egui_color(color),
        );
    }

    fn measure_text(&self, text: &str, size: f32) -> f32 {
        let ppp = self.ppp();
        // `size` is physical px; egui wants logical px for FontId.
        let font_id = egui::FontId::proportional(size / ppp);
        // Laying out the text is the authoritative measurement path —
        // it consults the same font + shaping that `draw_text` uses,
        // so the caret lands exactly at the visible end of the run.
        // `fonts_mut` caches the resulting galley, so repeated calls
        // for the same draft text are cheap. `INFINITY` wrap width
        // matches the single-line Phase 1 contract (no Shift+Enter
        // until Phase 2 — ADR-0017).
        let galley = self.ctx.fonts_mut(|f| {
            f.layout(
                text.to_owned(),
                font_id,
                egui::Color32::PLACEHOLDER,
                f32::INFINITY,
            )
        });
        // Galley size is logical px; convert back to physical to match
        // the canvas-physical coordinate space the render path uses.
        galley.size().x * ppp
    }
}
