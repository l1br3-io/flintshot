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
}

impl EguiCanvas {
    /// Construct a canvas bound to an [`egui::Context`]. The shell
    /// must call [`EguiCanvas::bind_painter`] each frame before
    /// invoking [`flintshot_core::render::render`].
    #[must_use]
    pub fn new(ctx: egui::Context) -> Self {
        Self { ctx, painter: None }
    }

    /// Bind the per-frame [`egui::Painter`]. Called by the shell from
    /// inside its `update()` body; subsequent `draw_*` calls dispatch
    /// through this painter until the next bind.
    pub fn bind_painter(&mut self, painter: &egui::Painter) {
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

/// Convert a physical-px [`Rect`] to a logical-px [`egui::Rect`] using
/// the toolkit's `pixels_per_point`.
///
/// Callers must pass an already-normalized rect (`min` top-left). The
/// render path normalizes before invoking the canvas methods.
fn to_egui_rect(rect: Rect, ppp: f32) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(rect.min.x / ppp, rect.min.y / ppp),
        egui::vec2(rect.width() / ppp, rect.height() / ppp),
    )
}

/// Convert a physical-px [`Point`] to a logical-px [`egui::Pos2`].
fn to_egui_pos(p: Point, ppp: f32) -> egui::Pos2 {
    egui::pos2(p.x / ppp, p.y / ppp)
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
        let dest = egui::Rect::from_min_size(
            egui::pos2(rect.min.x / ppp, rect.min.y / ppp),
            egui::vec2(rect.width() / ppp, rect.height() / ppp),
        );
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        painter.image(img.0.id(), dest, uv, egui::Color32::WHITE);
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
            to_egui_rect(rect, ppp),
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
            to_egui_rect(rect, self.ppp()),
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
            [to_egui_pos(a, ppp), to_egui_pos(b, ppp)],
            to_egui_stroke(*stroke, ppp),
        );
    }

    fn draw_arrow(&mut self, a: Point, b: Point, stroke: &Stroke, head_size: f32) {
        let Some(painter) = &self.painter else {
            return;
        };
        let ppp = self.ppp();
        let a_l = to_egui_pos(a, ppp);
        let b_l = to_egui_pos(b, ppp);
        let egui_stroke = to_egui_stroke(*stroke, ppp);
        painter.line_segment([a_l, b_l], egui_stroke);
        // `arrow_barbs` returns physical-px endpoints (head_size is
        // defined in physical px); converting them to logical via
        // `to_egui_pos` yields the right on-screen length at any
        // `pixels_per_point`, matching what `ImageCanvas` bakes into
        // the saved PNG (ADR-0015 WYSIWYG).
        let [left, right] = arrow_barbs(a, b, head_size);
        painter.line_segment([b_l, to_egui_pos(left, ppp)], egui_stroke);
        painter.line_segment([b_l, to_egui_pos(right, ppp)], egui_stroke);
    }

    fn draw_text(&mut self, pos: Point, text: &str, size: f32, color: Color) {
        let Some(painter) = &self.painter else {
            return;
        };
        let ppp = self.ppp();
        // `size` is physical px; egui wants logical px for FontId.
        let font_id = egui::FontId::proportional(size / ppp);
        painter.text(
            to_egui_pos(pos, ppp),
            egui::Align2::LEFT_TOP,
            text,
            font_id,
            to_egui_color(color),
        );
    }
}
