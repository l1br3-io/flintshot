//! egui implementation of the [`Canvas`] trait.
//!
//! [`Canvas`]: flintshot_core::render::Canvas

use flintshot_core::{render::Canvas, Color, Point, Rect, Stroke};

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
/// Annotation primitives (`draw_rect_*`, `draw_line`, …) remain
/// stubs until the corresponding tools land in Phase 1.C+.
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

    fn draw_rect_outline(&mut self, _rect: Rect, _stroke: &Stroke) {}
    fn draw_rect_filled(&mut self, _rect: Rect, _color: Color) {}
    fn draw_ellipse(&mut self, _rect: Rect, _stroke: &Stroke, _fill: Option<Color>) {}
    fn draw_line(&mut self, _a: Point, _b: Point, _stroke: &Stroke) {}
    fn draw_arrow(&mut self, _a: Point, _b: Point, _stroke: &Stroke, _head_size: f32) {}
    fn draw_text(&mut self, _pos: Point, _text: &str, _size: f32, _color: Color) {}
}
