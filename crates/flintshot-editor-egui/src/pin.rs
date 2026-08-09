//! The eframe app that drives a pin-to-desktop window.
//!
//! [`PinApp`] owns the pinned [`image::RgbaImage`], its cached egui
//! texture, and the resolved [`PinConfig`] (opacity + copy-on-close).
//! The shell ([`crate::shell::EguiShell`]) constructs one per
//! `run_pin` call; eframe owns it for the window's lifetime.
//!
//! ## The five UX features
//!
//! 1. **Always-on-top + drag** — the viewport is created
//!    `.with_always_on_top()`; a primary press anywhere off the resize
//!    handle hands control to the WM via [`egui::ViewportCommand::StartDrag`].
//! 2. **Close on Esc / double-click** — both publish a close, which
//!    sends [`egui::ViewportCommand::Close`].
//! 3. **Hover-opacity** — pointer over the window → fully opaque;
//!    pointer away → `PinConfig::default_opacity`. Instant (no tween),
//!    Flameshot-style.
//! 4. **Aspect-locked resize** — a bottom-right handle scales the
//!    window via [`egui::ViewportCommand::InnerSize`], deriving height
//!    from width ÷ image aspect so proportions stay intact.
//! 5. **Copy-on-close** — when `PinConfig::copy_on_close` is set, the
//!    image is placed on the clipboard via `arboard` as the window
//!    closes. The `arboard::Clipboard` handle is kept alive until the
//!    app drops (see `copy_image_to_clipboard` for the X11 caveat).
//!
//! Pin renders exactly one image (no annotations), so ADR-0012's
//! "route content through `render()`" is satisfied by the single texture
//! blit; hover-opacity is a paint alpha on that blit, not a re-render.

use eframe::App;
use egui::Context;
use flintshot_core::{PinConfig, Rect};

use crate::EguiTexture;

/// Edge length (logical px) of the bottom-right resize handle hit area.
const HANDLE_SIZE: f32 = 18.0;

/// Floor for either dimension when aspect-resizing, so a violent fling
/// toward the corner can't collapse the window to nothing.
const MIN_PIN_SIZE: f32 = 32.0;

/// What a close trigger decided to do. Pure data so the decision is
/// unit-testable without an egui context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CloseAction {
    /// Whether the image should be copied to the clipboard as part of
    /// closing (`PinConfig::copy_on_close`).
    copy: bool,
}

/// The eframe app for a single pin-to-desktop window.
///
/// Constructed by [`crate::shell::EguiShell::run_pin`] with the image,
/// the resolved window `rect`, and the merged [`PinConfig`]. The image
/// texture uploads lazily on the first `ui()` so construction is pure
/// and unit-testable without a display.
pub(crate) struct PinApp {
    /// The pinned image. Kept for clipboard copy-on-close; the texture
    /// is the rendered copy.
    image: image::RgbaImage,
    /// Cached texture handle, uploaded once on the first frame. `None`
    /// until then (mirrors the editor's bg cache).
    texture: Option<EguiTexture>,
    /// Resolved pin config (opacity + copy-on-close).
    config: PinConfig,
    /// Image width ÷ height. Used to keep resize aspect-locked.
    aspect: f32,
    /// Most recent content-panel rect (screen-logical px), cached by
    /// `ui()` so the next frame's `logic()` can hit-test the resize
    /// handle. `None` until the first `ui()` runs.
    panel_rect: Option<egui::Rect>,
    /// Whether an aspect-locked resize drag is in progress.
    resizing: bool,
    /// Pointer position (screen-logical) recorded when the resize drag
    /// began. Deltas are measured from here so feedback lag can't drift
    /// the size.
    resize_start: egui::Pos2,
    /// Inner size (screen-logical) at the start of the resize drag.
    resize_start_size: egui::Vec2,
    /// Clipboard handle kept alive after a copy-on-close so the X11
    /// selection owner (this process) doesn't release the data before
    /// the window finishes tearing down. `None` until the first copy.
    clipboard: Option<arboard::Clipboard>,
}

impl PinApp {
    /// Construct the app. No egui context is needed — the texture
    /// uploads lazily in `ui()` — so this is pure and unit-testable.
    #[allow(
        clippy::cast_precision_loss,
        reason = "image dims ≤ 2^24 are exact in f32"
    )]
    #[must_use]
    pub(crate) fn new(image: image::RgbaImage, rect: Rect, config: PinConfig) -> Self {
        let (w, h) = image.dimensions();
        let aspect = if h == 0 { 1.0 } else { w as f32 / h as f32 };
        let inner_size = egui::vec2(rect.width(), rect.height());
        Self {
            image,
            texture: None,
            config,
            aspect,
            panel_rect: None,
            resizing: false,
            resize_start: egui::Pos2::ZERO,
            resize_start_size: inner_size,
            clipboard: None,
        }
    }

    /// Decide whether the pin window should close this frame, given the
    /// translated input flags. Returns `Some(CloseAction)` when Esc was
    /// pressed or the image was double-clicked; `None` otherwise. The
    /// `copy` flag is `PinConfig::copy_on_close`, so callers know
    /// whether to seed the clipboard before sending
    /// [`egui::ViewportCommand::Close`].
    #[must_use]
    fn close_from_input(&self, esc: bool, double_clicked: bool) -> Option<CloseAction> {
        if esc || double_clicked {
            Some(CloseAction {
                copy: self.config.copy_on_close,
            })
        } else {
            None
        }
    }

    /// Bottom-right resize handle hit area, in screen-logical px.
    fn handle_rect(panel: egui::Rect) -> egui::Rect {
        egui::Rect::from_min_size(
            egui::pos2(panel.right() - HANDLE_SIZE, panel.bottom() - HANDLE_SIZE),
            egui::vec2(HANDLE_SIZE, HANDLE_SIZE),
        )
    }

    /// Whether `pos` (screen-logical) lands on the resize handle.
    /// `false` until the first `ui()` reports a panel rect.
    fn handle_hit(&self, pos: egui::Pos2) -> bool {
        self.panel_rect
            .is_some_and(|p| Self::handle_rect(p).contains(pos))
    }

    /// Upload the pinned image as an egui texture (Nearest filtering so
    /// screenshot pixels stay sharp). Mirrors `EguiCanvas::upload_image`.
    fn upload_texture(&self, ctx: &Context) -> EguiTexture {
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [self.image.width() as usize, self.image.height() as usize],
            self.image.as_raw(),
        );
        let handle = ctx.load_texture("flintshot-pin", color_image, egui::TextureOptions::NEAREST);
        EguiTexture(handle)
    }

    /// Place the pinned image on the clipboard, keeping the handle alive
    /// in `self.clipboard`.
    ///
    /// On X11 the clipboard is **owner-based**: the process that sets
    /// the selection must stay alive or the data is lost. The handle
    /// lives until [`PinApp`] drops (when `eframe::run_native` returns);
    /// for a standalone `flintshot pin` process that's only a few more
    /// milliseconds before exit, so X11 copy-on-close is best-effort.
    /// On Wayland / macOS / Windows the compositor / system owns the
    /// data and persistence does not depend on this handle.
    fn copy_image_to_clipboard(&mut self) {
        let mut clipboard = match self.clipboard.take() {
            Some(c) => c,
            None => match arboard::Clipboard::new() {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "could not open clipboard for pin copy-on-close"
                    );
                    return;
                }
            },
        };
        let (w, h) = self.image.dimensions();
        let img_data = arboard::ImageData {
            width: w as usize,
            height: h as usize,
            bytes: std::borrow::Cow::from(self.image.as_raw().as_slice()),
        };
        if let Err(e) = clipboard.set_image(img_data) {
            tracing::warn!(error = %e, "could not place pin image on clipboard");
        }
        self.clipboard = Some(clipboard);
    }
}

impl App for PinApp {
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let double = ctx.input(|i| {
            i.pointer
                .button_double_clicked(egui::PointerButton::Primary)
        });

        if let Some(action) = self.close_from_input(esc, double) {
            if action.copy {
                self.copy_image_to_clipboard();
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        let (pressed, down, pos) = ctx.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.latest_pos(),
            )
        });

        // On a fresh press: the resize handle starts an aspect-locked
        // drag; anywhere else hands the move off to the WM. StartDrag
        // only has effect when sent right after a press, so it must
        // live here rather than in a held-drag branch.
        if pressed {
            if let Some(p) = pos {
                if self.handle_hit(p) {
                    self.resizing = true;
                    self.resize_start = p;
                    if let Some(pr) = self.panel_rect {
                        self.resize_start_size = pr.size();
                    }
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
            }
        }

        if self.resizing {
            if !down {
                self.resizing = false;
            } else if let Some(p) = pos {
                let delta = p - self.resize_start;
                let new_w = (self.resize_start_size.x + delta.x).max(MIN_PIN_SIZE);
                let new_h = (new_w / self.aspect).max(MIN_PIN_SIZE / self.aspect);
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(new_w, new_h)));
                ctx.request_repaint();
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Transparent panel so the window's `with_transparent(true)` +
        // the image's paint alpha compose against the desktop, not
        // egui's opaque panel background.
        let frame = egui::Frame {
            fill: egui::Color32::TRANSPARENT,
            ..egui::Frame::NONE
        };
        let panel = egui::CentralPanel::default().frame(frame).show(ui, |ui| {
            let rect = ui.max_rect();
            if self.texture.is_none() {
                self.texture = Some(self.upload_texture(&ctx));
            }
            // Hover-opacity: opaque while the pointer is over the
            // window, resting opacity otherwise. `latest_pos` is
            // `Some` exactly while the pointer is inside the
            // viewport, so this is an instant on/off, no tween.
            let hovered = ctx.input(|i| i.pointer.latest_pos()).is_some();
            let opacity = if hovered {
                1.0
            } else {
                self.config.default_opacity
            };
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "opacity is clamped to 0.0..=1.0 before scaling"
            )]
            let alpha = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
            if let Some(tex) = &self.texture {
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                ui.painter()
                    .image(tex.0.id(), rect, uv, egui::Color32::from_white_alpha(alpha));
            }
            // Resize handle affordance at the bottom-right corner.
            let hr = Self::handle_rect(rect);
            ui.painter().rect_filled(
                hr,
                egui::CornerRadius::ZERO,
                egui::Color32::from_black_alpha(90),
            );
        });
        self.panel_rect = Some(panel.response.rect);
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, reason = "exact aspect ratio, no arithmetic error")]
mod tests {
    use super::*;
    use flintshot_core::Rect;

    /// An all-white RGBA image of the given dimensions.
    fn sample_image(w: u32, h: u32) -> image::RgbaImage {
        image::RgbaImage::from_raw(w, h, vec![255; (w as usize) * (h as usize) * 4])
            .expect("raw buffer matches w*h*4")
    }

    #[test]
    fn pin_app_constructs_without_panic_and_records_aspect() {
        let img = sample_image(80, 40);
        let rect = Rect::from_xywh(10.0, 20.0, 80.0, 40.0);
        let app = PinApp::new(img, rect, PinConfig::default());
        assert_eq!(app.aspect, 2.0, "80÷40 = 2.0");
        assert!(!app.config.copy_on_close, "default is no copy-on-close");
    }

    #[test]
    fn pin_app_close_on_esc() {
        let app = PinApp::new(
            sample_image(10, 10),
            Rect::from_xywh(0.0, 0.0, 10.0, 10.0),
            PinConfig::default(),
        );
        assert_eq!(
            app.close_from_input(true, false),
            Some(CloseAction { copy: false }),
            "Esc must close"
        );
    }

    #[test]
    fn pin_app_close_on_double_click() {
        let app = PinApp::new(
            sample_image(10, 10),
            Rect::from_xywh(0.0, 0.0, 10.0, 10.0),
            PinConfig::default(),
        );
        assert_eq!(
            app.close_from_input(false, true),
            Some(CloseAction { copy: false }),
            "double-click must close"
        );
    }

    #[test]
    fn pin_app_no_close_when_idle() {
        let app = PinApp::new(
            sample_image(10, 10),
            Rect::from_xywh(0.0, 0.0, 10.0, 10.0),
            PinConfig::default(),
        );
        assert_eq!(
            app.close_from_input(false, false),
            None,
            "no input → no close"
        );
    }

    #[test]
    fn pin_app_close_action_respects_copy_on_close() {
        let mut cfg = PinConfig::default();
        cfg.copy_on_close = true;
        let app = PinApp::new(
            sample_image(10, 10),
            Rect::from_xywh(0.0, 0.0, 10.0, 10.0),
            cfg,
        );
        assert_eq!(
            app.close_from_input(true, false),
            Some(CloseAction { copy: true }),
            "Esc with copy_on_close must flag the copy"
        );
    }
}
