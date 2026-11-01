//! The standalone rect-pick mode for `flintshot record` on X11.
//!
//! One fullscreen frozen frame, one drag, one JSON rect on stdout,
//! exit — no annotation UI. The drag logic is a pure state machine
//! ([`PickState`]) so the geometry (normalization, degenerate
//! drags, cancel) is unit-tested headless; the eframe window around
//! it is the manual-pass surface, like every other editor window.

use flintshot_core::geom::{Point, Rect};

/// Minimum drag extent (pixels) below which a release is treated as
/// a click, not a rect.
pub const PICK_MIN_EXTENT: f32 = 2.0;

/// Pure drag-rect state machine for the pick window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PickPhase {
    /// Nothing pressed yet.
    Idle,
    /// Button down at `anchor`, pointer now at `current`.
    Dragging { anchor: Point, current: Point },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PickState {
    phase: PickPhase,
}

impl Default for PickState {
    fn default() -> Self {
        Self {
            phase: PickPhase::Idle,
        }
    }
}

impl PickState {
    /// New idle picker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The current drag rectangle, normalized (min < max), if a
    /// drag is in progress.
    #[must_use]
    pub fn current_rect(&self) -> Option<Rect> {
        match self.phase {
            PickPhase::Idle => None,
            PickPhase::Dragging { anchor, current } => Some(normalize(Rect::new(anchor, current))),
        }
    }

    /// Button pressed at `at`.
    pub fn press(&mut self, at: Point) {
        if self.phase == PickPhase::Idle {
            self.phase = PickPhase::Dragging {
                anchor: at,
                current: at,
            };
        }
    }

    /// Pointer moved while pressed.
    pub fn drag(&mut self, to: Point) {
        if let PickPhase::Dragging { anchor, .. } = self.phase {
            self.phase = PickPhase::Dragging {
                anchor,
                current: to,
            };
        }
    }

    /// Button released: the picked rect, or `None` for a degenerate
    /// (click-like) drag. The picker is idle again afterwards.
    pub fn release(&mut self) -> Option<Rect> {
        let picked = self.current_rect().filter(|rect| {
            let (w, h) = (rect.max.x - rect.min.x, rect.max.y - rect.min.y);
            w >= PICK_MIN_EXTENT && h >= PICK_MIN_EXTENT
        });
        self.phase = PickPhase::Idle;
        picked
    }

    /// Escape pressed mid-drag: abort to idle.
    pub fn cancel(&mut self) {
        self.phase = PickPhase::Idle;
    }
}

/// Normalize a rect so `min` is the top-left and `max` the
/// bottom-right regardless of drag direction.
#[must_use]
pub fn normalize(rect: Rect) -> Rect {
    let min_x = rect.min.x.min(rect.max.x);
    let max_x = rect.min.x.max(rect.max.x);
    let min_y = rect.min.y.min(rect.max.y);
    let max_y = rect.min.y.max(rect.max.y);
    Rect::new(Point::new(min_x, min_y), Point::new(max_x, max_y))
}

// ── the window (manual-pass surface; the logic above is unit-tested) ──

use std::cell::RefCell;
use std::rc::Rc;

use eframe::egui::{self, Key, ViewportCommand};
use eframe::{AppCreator, NativeOptions};

/// Shared slot: `None` until a pick lands; a closed window with no
/// pick means cancelled (the overlay's default-cancelled stance).
type PickedSlot = Rc<RefCell<Option<Rect>>>;

/// Frame dimensions as an egui size.
#[expect(
    clippy::cast_precision_loss,
    reason = "frame dimensions fit f32's mantissa exactly"
)]
fn frame_size(image: &image::RgbaImage) -> egui::Vec2 {
    egui::Vec2::new(image.width() as f32, image.height() as f32)
}

/// Run the fullscreen pick window over `image` — the frozen frame of
/// the monitor being picked from — and block until the user picks a
/// rectangle (returned in **image pixels**, normalized) or cancels
/// (`None`). Screen-point → image-pixel mapping divides by the
/// window/image size ratio, so any display scale is honored.
///
/// # Errors
/// Surfaces [`flintshot_core::FlintshotError`] for window-system or
/// render failures — the same seam as every other native window in
/// this crate.
pub fn run_rect_pick_blocking(image: image::RgbaImage) -> flintshot_core::Result<Option<Rect>> {
    let options = NativeOptions {
        viewport: crate::overlay::overlay_viewport(),
        ..Default::default()
    };
    let picked: PickedSlot = Rc::new(RefCell::new(None));
    let slot_for_app = Rc::clone(&picked);

    let app_creator: AppCreator<'_> = Box::new(move |cc| {
        let size = frame_size(&image);
        let pixels = egui::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize],
            image.as_raw(),
        );
        let texture =
            cc.egui_ctx
                .load_texture("pick-frozen-frame", pixels, egui::TextureOptions::LINEAR);
        Ok(Box::new(PickApp {
            state: PickState::new(),
            texture,
            image_size: size,
            picked: slot_for_app,
        }))
    });

    eframe::run_native("flintshot — pick a region", options, app_creator)
        .map_err(|e| crate::app::eframe_err_to_flintshot(&e))?;
    let rect = picked.borrow_mut().take();
    Ok(rect)
}

/// The pick window: frozen frame, dimmed, with the dragged rect
/// undimmed and outlined.
struct PickApp {
    state: PickState,
    texture: egui::TextureHandle,
    image_size: egui::Vec2,
    picked: PickedSlot,
}

/// Paint one pick frame: the frozen capture, dimmed except the
/// dragged rect, which is re-drawn undimmed, outlined, and sized.
fn paint_pick(
    painter: &egui::Painter,
    texture: &egui::TextureHandle,
    state: &PickState,
    screen: egui::Rect,
    scale: egui::Vec2,
    image_size: egui::Vec2,
) {
    let full_uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    painter.image(texture.id(), screen, full_uv, egui::Color32::WHITE);
    let Some(picking) = state.current_rect() else {
        return;
    };
    let picked = egui::Rect::from_min_max(
        egui::pos2(picking.min.x / scale.x, picking.min.y / scale.y),
        egui::pos2(picking.max.x / scale.x, picking.max.y / scale.y),
    );
    let dim = crate::theme::palette::readability_black(140);
    // Dim above and below, left and right of the pick…
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(screen.left(), screen.top()),
            egui::pos2(screen.right(), picked.top()),
        ),
        egui::CornerRadius::same(0),
        dim,
    );
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(screen.left(), picked.bottom()),
            egui::pos2(screen.right(), screen.bottom()),
        ),
        egui::CornerRadius::same(0),
        dim,
    );
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(screen.left(), picked.top()),
            egui::pos2(picked.left(), picked.bottom()),
        ),
        egui::CornerRadius::same(0),
        dim,
    );
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(picked.right(), picked.top()),
            egui::pos2(screen.right(), screen.bottom()),
        ),
        egui::CornerRadius::same(0),
        dim,
    );
    // …re-draw the picked strip undimmed, outlined, with a size
    // readout.
    let uv = egui::Rect::from_min_max(
        egui::pos2(picking.min.x / image_size.x, picking.min.y / image_size.y),
        egui::pos2(picking.max.x / image_size.x, picking.max.y / image_size.y),
    );
    painter.image(texture.id(), picked, uv, egui::Color32::WHITE);
    painter.rect_stroke(
        picked,
        egui::CornerRadius::same(0),
        egui::Stroke::new(2.0, egui::Color32::WHITE),
        egui::StrokeKind::Inside,
    );
    let (w, h) = (picking.max.x - picking.min.x, picking.max.y - picking.min.y);
    painter.debug_text(
        picked.left_top() + egui::vec2(4.0, 4.0),
        egui::Align2::LEFT_TOP,
        egui::Color32::WHITE,
        format!("{w:.0}×{h:.0}"),
    );
}

impl eframe::App for PickApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // The scale (screen points -> image pixels) falls out of the
        // panel's rect once it has filled the window; pointer
        // mapping happens after the panel, off this value.
        let mut scale = egui::Vec2::splat(1.0);

        // Frame::NONE + position (0,0): screen px == image px is the
        // pick window's whole coordinate contract (the overlay's
        // invariant, reused).
        let panel = egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let screen = ui.max_rect();
                let size = ui.available_size();
                let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
                scale = egui::Vec2::new(
                    self.image_size.x / screen.width().max(1.0),
                    self.image_size.y / screen.height().max(1.0),
                );
                paint_pick(
                    ui.painter(),
                    &self.texture,
                    &self.state,
                    rect,
                    scale,
                    self.image_size,
                );
                response
            });

        let response = panel.inner;
        let to_image = |pos: egui::Pos2| Point::new(pos.x * scale.x, pos.y * scale.y);
        if response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos() {
                self.state.press(to_image(pos));
            }
        }
        if response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                self.state.drag(to_image(pos));
            }
        }
        if response.drag_stopped() {
            if let Some(rect) = self.state.release() {
                *self.picked.borrow_mut() = Some(rect);
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
        if ctx.input(|input| input.key_pressed(Key::Escape)) {
            self.state.cancel();
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f32, y: f32) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn drag_down_right_up_round_trips_the_rect() {
        let mut pick = PickState::new();
        pick.press(pt(10.0, 20.0));
        pick.drag(pt(110.0, 220.0));
        assert_eq!(
            pick.current_rect(),
            Some(Rect::new(pt(10.0, 20.0), pt(110.0, 220.0)))
        );
        assert_eq!(
            pick.release(),
            Some(Rect::new(pt(10.0, 20.0), pt(110.0, 220.0)))
        );
        // Idle again: a stray release yields nothing.
        assert_eq!(pick.release(), None);
    }

    #[test]
    fn drag_up_left_normalizes_to_top_left_min() {
        let mut pick = PickState::new();
        pick.press(pt(110.0, 220.0));
        pick.drag(pt(10.0, 20.0));
        assert_eq!(
            pick.release(),
            Some(Rect::new(pt(10.0, 20.0), pt(110.0, 220.0)))
        );
    }

    #[test]
    fn degenerate_click_is_not_a_rect() {
        let mut pick = PickState::new();
        pick.press(pt(50.0, 50.0));
        pick.drag(pt(51.0, 51.0));
        assert_eq!(pick.release(), None);
    }

    #[test]
    fn cancel_aborts_the_drag() {
        let mut pick = PickState::new();
        pick.press(pt(0.0, 0.0));
        pick.drag(pt(400.0, 300.0));
        pick.cancel();
        assert_eq!(pick.current_rect(), None);
        assert_eq!(pick.release(), None);
    }

    #[test]
    fn press_while_dragging_reanchors_nothing() {
        // A second press mid-drag is ignored — the anchor holds.
        let mut pick = PickState::new();
        pick.press(pt(10.0, 10.0));
        pick.drag(pt(100.0, 100.0));
        pick.press(pt(999.0, 999.0));
        pick.drag(pt(200.0, 100.0));
        assert_eq!(
            pick.release(),
            Some(Rect::new(pt(10.0, 10.0), pt(200.0, 100.0)))
        );
    }
}
