//! Pin-to-desktop window: a borderless always-on-top eframe app that
//! blits a single image and lets the user drag it around the desktop.
//!
//! This is the leaf use of egui — no [`EditorState`], no annotations,
//! no toolbar. The pin owns its event loop (it's its own process,
//! spawned by `flintshot pin <path>`); closing the window exits the
//! process. See ADR-0023 (drafted in Task 005) for why a pin is a
//! separate process rather than a deferred viewport inside the editor:
//! egui viewports die with their root window, so an editor-spawned
//! viewport would close when the editor closes — the opposite of what
//! "pin to desktop" means.
//!
//! [`EditorState`]: flintshot_core::EditorState

use eframe::App;
use egui::Context;

use crate::canvas::{EguiCanvas, EguiTexture};
use flintshot_core::{render::Canvas, Rect};

/// Window title used as the WM class hint. Decorations are off so the
/// title is never rendered as a titlebar; it still shows up in taskbar
/// thumbnails, window switchers, and `wmctrl -l`. Matches the editor's
/// em-dash convention (`"flintshot — annotate"`).
const WINDOW_TITLE: &str = "flintshot — pin";

/// Build the viewport for a pin window showing `img_dims` at `rect`.
///
/// `rect`'s dimensions are advisory (the image's own dimensions are the
/// source of truth for `inner_size`); `rect.min` carries the desired
/// screen position when the caller knows it. Both Phase 3 callers
/// (`flintshot pin <path>` and the editor's Pin button) pass a default
/// `Rect` with `min == (0, 0)`, which is interpreted as "let the WM
/// place the window" — `with_position` is skipped so the compositor
/// applies its default placement policy (cascade, center, pointer-follow).
/// The Phase 5 live overlay will pass a real screen position so a pin
/// opens exactly where the capture was taken.
///
/// Factored out of [`run_pin_blocking`] so the viewport flags can be
/// asserted headlessly without spinning up a real window.
#[must_use]
pub fn pin_viewport_builder(img_dims: (u32, u32), rect: Rect) -> egui::ViewportBuilder {
    #[allow(
        clippy::cast_precision_loss,
        reason = "image dims ≤ 2^24 are exact in f32"
    )]
    let inner_size = egui::vec2(img_dims.0 as f32, img_dims.1 as f32);
    let mut builder = egui::ViewportBuilder::default()
        .with_title(WINDOW_TITLE)
        .with_inner_size(inner_size)
        .with_resizable(true)
        .with_decorations(false)
        .with_always_on_top();
    // min == (0, 0) is the "no position preference" sentinel. A real
    // screen position (e.g. from the Phase 5 live overlay's in-place
    // capture) sets `with_position` so the pin opens at the capture
    // origin. Comparing f32 equality here is fine — the sentinel is a
    // literal `0.0, 0.0` from `Rect::from_xywh(0.0, 0.0, ...)`.
    if rect.min.x != 0.0 || rect.min.y != 0.0 {
        builder = builder.with_position(egui::pos2(rect.min.x, rect.min.y));
    }
    builder
}

/// The pin window's eframe app: owns the image, uploads it once, blits
/// it every frame covering the full inner area, supports drag-to-move
/// via `ViewportCommand::StartDrag`, and closes on `Esc`.
///
/// No outcome slot — unlike [`EditorApp`], the pin has no result to
/// report. `run_pin` returns `Ok(())` once the window closes.
///
/// [`EditorApp`]: crate::app::EditorApp
pub(crate) struct PinApp {
    /// The pinned image. Held for the app's lifetime; the texture
    /// handle is the cached upload.
    image: image::RgbaImage,
    /// Cached texture handle, uploaded lazily on the first frame and
    /// reused on every subsequent frame (same pattern as `EditorApp`'s
    /// `bg` cache). `None` until the first successful upload.
    texture: Option<EguiTexture>,
    /// The canvas used for the blit. Reusing `EguiCanvas` (rather than
    /// a raw `Painter::image` call) keeps the upload + blit path
    /// identical to the editor's, so `HiDPI` / `pixels_per_point`
    /// handling comes for free.
    canvas: EguiCanvas,
}

impl PinApp {
    /// Construct a pin app showing `image`.
    pub(crate) fn new(image: image::RgbaImage, ctx: &Context) -> Self {
        Self {
            image,
            texture: None,
            canvas: EguiCanvas::new(ctx.clone()),
        }
    }

    /// Upload the background texture on first call; reuse on every
    /// subsequent call. Kept infallible (returns `()`) to match the
    /// pin's "no outcome to report" contract — a texture upload
    /// failure would be a GPU/eframe bug, not a user-actionable error.
    fn ensure_texture(&mut self) {
        if self.texture.is_none() {
            // `Canvas::upload_image` is infallible by trait contract
            // (returns `Result`, but the only failure mode is a
            // pathological image; the editor's upload path treats it
            // the same way). An upload failure leaves `texture` as
            // `None` and the frame is skipped — the next frame retries.
            if let Ok(handle) = self.canvas.upload_image(&self.image) {
                self.texture = Some(handle);
            }
        }
    }
}

impl App for PinApp {
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Esc closes the pin. No other key handling — no annotations,
        // no undo, no save. The window-close button (Alt+F4, WM close
        // menu item) ends the app via eframe's default viewport-close
        // handling, not through this path.
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // Drag-to-move: any primary press on the pin area starts an OS
        // window drag. `StartDrag` hands control to the WM, which moves
        // the window with the pointer until the button releases. This
        // is the borderless-window drag primitive — there's no
        // titlebar to grab, so the whole image is the drag handle.
        // RATIONALE: ADR-0023 — StartDrag is preferred over
        // `OuterPosition(pos + drag_delta)` because the WM-authored
        // drag respects snap/tiling policies the manual delta would
        // fight.
        if ctx.input(|i| i.pointer.primary_pressed()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Upload on first frame; cache for the rest.
        self.ensure_texture();

        let (img_w, img_h) = self.image.dimensions();
        #[allow(
            clippy::cast_precision_loss,
            reason = "image dims ≤ 2^24 are exact in f32"
        )]
        let img_rect = Rect::from_xywh(0.0, 0.0, img_w as f32, img_h as f32);

        // CentralPanel gives us a painter whose clip_rect matches the
        // window's inner area. Allocate the full image size so egui's
        // layering knows the pin's interactive region (which the
        // `StartDrag` in `logic` relies on — the pointer must hit a
        // widget for `primary_pressed` to register). Bind the panel's
        // painter to the canvas so `draw_image` lands at (0,0)
        // covering the full image.
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                #[allow(
                    clippy::cast_precision_loss,
                    reason = "image dims ≤ 2^24 are exact in f32"
                )]
                let size = egui::vec2(img_w as f32, img_h as f32);
                let (rect, _response) = ui.allocate_exact_size(size, egui::Sense::drag());
                self.canvas.bind_painter(&ui.painter().clone());
                if let Some(tex) = self.texture.clone() {
                    self.canvas.draw_image(&tex, img_rect);
                }
                let _ = rect;
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flintshot_core::{Point, Rect};

    /// The viewport must be borderless + always-on-top + resizable, with
    /// the title used as the WM class hint. These flags are the defining
    /// behaviour of a pin window — a regression in any of them changes
    /// what the user sees (a titled window, a window that goes behind,
    /// a window that can't be resized).
    #[test]
    fn pin_viewport_builder_carries_required_flags() {
        let dims = (800, 600);
        let rect = Rect::from_xywh(0.0, 0.0, 800.0, 600.0);
        let vp = pin_viewport_builder(dims, rect);

        assert_eq!(vp.title.as_deref(), Some(WINDOW_TITLE));
        assert_eq!(vp.decorations, Some(false));
        assert_eq!(vp.window_level, Some(egui::WindowLevel::AlwaysOnTop));
        assert_eq!(vp.resizable, Some(true));
        assert_eq!(
            vp.inner_size,
            Some(egui::vec2(800.0, 600.0)),
            "inner_size must match the image dimensions"
        );
    }

    /// A default rect (`min == (0, 0)`) means "let the WM place the
    /// window." The builder must NOT carry a position in that case —
    /// `pos2(0.0, 0.0)` is the screen's top-left corner, which is
    /// almost never what a user wants for a fresh pin.
    #[test]
    fn default_rect_omits_position() {
        let rect = Rect::from_xywh(0.0, 0.0, 100.0, 100.0);
        let vp = pin_viewport_builder((100, 100), rect);
        assert!(
            vp.position.is_none(),
            "default rect must not set a position; WM places the window"
        );
    }

    /// A rect with a real screen position sets `with_position` so the
    /// pin opens at the caller-chosen origin (Phase 5 live overlay will
    /// use this to open a pin where the capture was taken).
    #[test]
    fn positioned_rect_sets_position() {
        let rect = Rect::new(Point::new(120.0, 240.0), Point::new(920.0, 840.0));
        let vp = pin_viewport_builder((800, 600), rect);
        assert_eq!(vp.position, Some(egui::pos2(120.0, 240.0)));
    }

    /// Inner size tracks the image dimensions, not the rect — the rect
    /// is position + advisory size, but the image is the source of
    /// truth for what the window must fit.
    #[test]
    fn inner_size_tracks_image_dims_ignoring_rect() {
        let rect = Rect::from_xywh(0.0, 0.0, 999.0, 999.0);
        let vp = pin_viewport_builder((320, 240), rect);
        assert_eq!(vp.inner_size, Some(egui::vec2(320.0, 240.0)));
    }

    /// Different image sizes produce different inner sizes — sanity
    /// check that the dims flow through without swapping.
    #[test]
    fn inner_size_preserves_width_before_height() {
        let rect = Rect::from_xywh(0.0, 0.0, 0.0, 0.0);
        let vp = pin_viewport_builder((1920, 1080), rect);
        assert_eq!(vp.inner_size, Some(egui::vec2(1920.0, 1080.0)));
    }
}
