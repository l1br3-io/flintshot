//! egui implementation of `flintshot-core`'s [`Canvas`] and
//! [`EditorShell`] traits.
//!
//! Phase 0 ships empty stubs so the trait abstractions compile end-to-end.
//! Phase 1 fills in real painting, window setup, and event translation.
//!
//! [`Canvas`]: flintshot_core::render::Canvas
//! [`EditorShell`]: flintshot_core::shell::EditorShell

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]

use flintshot_core::{
    render::Canvas,
    shell::{EditorOutcome, EditorShell},
    Color, EditorState, FlintshotError, Point, Rect, Stroke,
};

/// egui-side texture handle, returned by [`EguiCanvas::upload_image`].
pub struct EguiTexture(pub egui::TextureHandle);

/// egui implementation of the [`Canvas`] trait.
///
/// Phase 0: every method is a stub. Phase 1 fills them in via
/// `egui::Painter` calls.
#[derive(Default)]
pub struct EguiCanvas {
    /// Captured during `paint` for use by subsequent draw calls within the
    /// same frame. Phase 1 wires this from the embedding `egui::Context`.
    // Field is unused in Phase 0 stubs; Phase 1 populates it from the
    // egui paint callback. Silence the dead-code lint until then.
    #[allow(dead_code)]
    painter: Option<egui::Painter>,
}

impl EguiCanvas {
    /// Construct an empty canvas. Phase 1 will replace this with one that
    /// takes the active `egui::Painter`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Canvas for EguiCanvas {
    type Image = EguiTexture;

    fn upload_image(&mut self, _img: &image::RgbaImage) -> flintshot_core::Result<Self::Image> {
        // Phase 1: ctx.load_texture("screenshot", image, NEAREST).
        Err(FlintshotError::NotImplemented("EguiCanvas::upload_image"))
    }

    fn draw_image(&mut self, _img: &Self::Image, _rect: Rect) {}
    fn draw_rect_outline(&mut self, _rect: Rect, _stroke: &Stroke) {}
    fn draw_rect_filled(&mut self, _rect: Rect, _color: Color) {}
    fn draw_ellipse(&mut self, _rect: Rect, _stroke: &Stroke, _fill: Option<Color>) {}
    fn draw_line(&mut self, _a: Point, _b: Point, _stroke: &Stroke) {}
    fn draw_arrow(&mut self, _a: Point, _b: Point, _stroke: &Stroke, _head_size: f32) {}
    fn draw_text(&mut self, _pos: Point, _text: &str, _size: f32, _color: Color) {}
}

/// egui implementation of the [`EditorShell`] trait.
///
/// Phase 0: stubs. Phase 1 wires `eframe::run_simple_native` with a
/// full editor app.
#[derive(Default)]
pub struct EguiShell;

impl EguiShell {
    /// Construct the shell.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl EditorShell for EguiShell {
    fn run_editor(&self, _state: EditorState) -> flintshot_core::Result<EditorOutcome> {
        // Phase 1: spin up eframe, return the user's outcome.
        Err(FlintshotError::NotImplemented("EguiShell::run_editor"))
    }

    fn run_pin(&self, _image: image::RgbaImage, _rect: Rect) -> flintshot_core::Result<()> {
        // Phase 3: borderless always-on-top viewport.
        Err(FlintshotError::NotImplemented("EguiShell::run_pin"))
    }
}
