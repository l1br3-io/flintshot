//! The OCR tool's editor-side flow: region drag → bridge request →
//! clipboard + result card, plus the models consent dialog.
//!
//! Geometry reuses [`crate::pick`]'s drag-rect machine (crop-style
//! region selection that never creates an annotation), and every
//! engine call rides the page bridge's request/poll seam (the
//! Ask-the-shot pattern) so no recognition ever runs on the UI
//! thread. Frame tests in `app.rs` drive the whole flow through a
//! mock bridge.

use flintshot_core::geom::Rect;
use flintshot_core::{crop_region, OcrEngineKind};
use image::RgbaImage;

use crate::pick::PickState;

/// The pixels an engine receives for `rect` (`None` = the whole
/// image — Enter's contract). Only this crop ever leaves the editor:
/// the "region-crop-before-recognize" rule, pinned by unit and by
/// the frame tests' request-dims assertions.
#[must_use]
pub(crate) fn region_pixels(image: &RgbaImage, rect: Option<Rect>) -> RgbaImage {
    match rect {
        Some(rect) => crop_region(image, rect),
        None => image.clone(),
    }
}

/// The OCR flow's phases. `Idle` doubles as "armed": the tool takes
/// a drag or Enter whenever no pass is in flight. `Done`/`Failed`
/// persist as the result card until the next pass (or Esc) so
/// switching tools never loses the recognized text.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) enum OcrPhase {
    /// No pass in flight and no result to show (the resting state).
    #[default]
    Idle,
    /// A pass is in flight (the region already left on the bridge).
    Pending,
    /// The last pass succeeded.
    Done {
        /// Recognized text (per-line newlines).
        text: String,
        /// Which engine produced it (the badge).
        engine: OcrEngineKind,
    },
    /// The last pass failed; `missing_models` drives the consent
    /// dialog's exactly-once rule.
    Failed {
        /// Actionable failure copy (guidance names the cure).
        detail: String,
        /// Whether the cure is a consented model download.
        missing_models: bool,
    },
}

/// Intent fired by the result card's buttons (the app consumes it
/// after the panel pass).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OcrCardAction {
    /// Copy the recognized text again.
    CopyText,
    /// Re-run the last region.
    Retry,
}

/// Intent fired by the consent dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OcrConsentAction {
    /// Consented: fetch the model pair.
    Download,
    /// Declined for now.
    Dismiss,
}

/// The card's engine badge: short label + the caveat tooltip
/// (SPEC-ocr-region's "documented in UI" honesty split).
#[must_use]
pub(crate) fn badge_for(engine: OcrEngineKind) -> (&'static str, &'static str) {
    match engine {
        OcrEngineKind::Ocrs => (
            "built-in · Latin",
            "Pure-Rust ocrs models (downloaded with your consent). Upstream self-describes as an early preview — expect more errors than commercial OCR. Latin script only.",
        ),
        OcrEngineKind::Tesseract => (
            "tesseract",
            "The system tesseract binary. Every installed language; quality depends on the installed version and language packs.",
        ),
    }
}

/// The OCR tool's state machine + consent bookkeeping.
#[derive(Debug, Default)]
pub(crate) struct OcrFlow {
    /// Current phase.
    pub phase: OcrPhase,
    pick: PickState,
    /// The pixels of the last request — retry re-sends exactly these.
    last_pixels: Option<RgbaImage>,
    /// The consent dialog already ran for the current missing-model
    /// state (the "exactly once" rule).
    consent_asked: bool,
    /// The consent dialog is open (modal while open).
    pub consent_open: bool,
    /// Card action fired this frame; the app takes it after the
    /// panel pass.
    pub card_action: Option<OcrCardAction>,
}

impl OcrFlow {
    /// Whether a region press or Enter may start a pass (never
    /// mid-pass, never mid-drag).
    #[must_use]
    pub fn accepts_press(&self) -> bool {
        self.phase != OcrPhase::Pending && !self.dragging()
    }

    /// The result card is showing (Done or Failed).
    #[must_use]
    pub fn has_result(&self) -> bool {
        matches!(self.phase, OcrPhase::Done { .. } | OcrPhase::Failed { .. })
    }

    /// The in-progress drag rect, if any (paint feedback).
    #[must_use]
    pub fn drag_rect(&self) -> Option<Rect> {
        self.pick.current_rect()
    }

    /// Press began (image coords).
    pub fn press(&mut self, at: flintshot_core::Point) {
        self.pick.press(at);
    }

    /// Drag moved (image coords).
    pub fn drag_to(&mut self, to: flintshot_core::Point) {
        self.pick.drag(to);
    }

    /// Whether a drag is live.
    #[must_use]
    pub fn dragging(&self) -> bool {
        self.drag_rect().is_some()
    }

    /// Button released: the picked rect, or `None` for a degenerate
    /// click (the flow stays armed — Enter remains whole-image).
    pub fn release(&mut self) -> Option<Rect> {
        self.pick.release()
    }

    /// A pass left on the bridge; remember `pixels` for retry.
    pub fn note_request(&mut self, pixels: RgbaImage) {
        self.last_pixels = Some(pixels);
        self.phase = OcrPhase::Pending;
    }

    /// The pixels to re-send on retry, if a pass ever left.
    #[must_use]
    pub fn retry_pixels(&self) -> Option<&RgbaImage> {
        self.last_pixels.as_ref()
    }

    /// The pass succeeded. Also re-arms the consent rule: models now
    /// work, so the *next* missing-model failure is a new state.
    pub fn note_done(&mut self, text: String, engine: OcrEngineKind) {
        self.consent_asked = false;
        self.phase = OcrPhase::Done { text, engine };
    }

    /// The pass failed. Returns whether the consent dialog should
    /// open — `true` only for the missing-models shape, and only
    /// once per that state.
    pub fn note_failure(&mut self, detail: String, missing_models: bool) -> bool {
        if missing_models && !self.consent_asked {
            self.consent_asked = true;
            self.consent_open = true;
        }
        self.phase = OcrPhase::Failed {
            detail,
            missing_models,
        };
        self.consent_open
    }

    /// Dismiss the consent dialog (the flow keeps its Failed card —
    /// the guidance copy still names both cures).
    pub fn dismiss_consent(&mut self) {
        self.consent_open = false;
    }

    /// Esc: back to Idle (card gone, drag cancelled).
    pub fn reset(&mut self) {
        self.pick.cancel();
        self.phase = OcrPhase::Idle;
    }
}

/// Paint the consent dialog (the `AiDialog` shape: centered Middle
/// area, Esc-dismissable). Returns the fired action, if any.
pub(crate) fn show_consent_dialog(
    ctx: &egui::Context,
    flow: &mut OcrFlow,
) -> Option<OcrConsentAction> {
    if !flow.consent_open {
        return None;
    }
    let mut action = None;
    egui::Area::new(egui::Id::new("ocr-consent-dialog"))
        .order(egui::Order::Middle)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .interactable(true)
        .show(ctx, |ui| {
            egui::Frame::window(ui.style()).show(ui, |ui| {
                ui.set_width(340.0);
                ui.heading("Download OCR models?");
                ui.label(
                    "The built-in OCR engine needs its model files. \
                     They download once, from:",
                );
                for spec in flintshot_core::MODEL_SPECS {
                    ui.monospace(spec.url).on_hover_text(
                        "The exact source this download reads; no other URL is used.",
                    );
                }
                ui.label("flintshot never downloads anything without your say-so.");
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    // Explicit id: frame tests click this response
                    // (the eyedropper chip pattern).
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(150.0, crate::theme::CONTROL_H),
                        egui::Sense::click(),
                    );
                    let download = ui.interact(
                        rect,
                        egui::Id::new("ocr_consent_download"),
                        egui::Sense::click(),
                    );
                    let visuals = ui.style().interact(&download);
                    ui.painter().rect_filled(
                        rect,
                        visuals.corner_radius,
                        crate::theme::palette::FOREST,
                    );
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Download models",
                        egui::FontId::proportional(13.0),
                        crate::theme::palette::CREAM_TEXT,
                    );
                    if download.clicked() {
                        action = Some(OcrConsentAction::Download);
                    }
                    if ui.button("Not now").clicked() {
                        action = Some(OcrConsentAction::Dismiss);
                    }
                });
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    action = Some(OcrConsentAction::Dismiss);
                }
            });
        });
    action
}

/// The result card: engine badge (caveat on hover), the text, and
/// Copy / Retry. No-op unless a result is showing.
pub(crate) fn show_result_card(ui: &mut egui::Ui, flow: &mut OcrFlow) {
    if !flow.has_result() {
        return;
    }
    crate::widgets::section(
        ui,
        egui::Id::new("panel-section-ocr"),
        "OCR result",
        true,
        |ui| match flow.phase.clone() {
            OcrPhase::Done { text, engine } => {
                let (label, caveat) = badge_for(engine);
                ui.add(egui::Label::new(
                    egui::RichText::new(label).small().strong(),
                ))
                .on_hover_text(caveat);
                let mut shown = text;
                ui.add(
                    egui::TextEdit::multiline(&mut shown)
                        .font(egui::TextStyle::Monospace)
                        .desired_rows(4)
                        .interactive(false),
                );
                ui.horizontal(|ui| {
                    if ui.button("Copy").clicked() {
                        flow.card_action = Some(OcrCardAction::CopyText);
                    }
                    // Explicit id: frame tests click this response.
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(70.0, crate::theme::CONTROL_H),
                        egui::Sense::click(),
                    );
                    let retry =
                        ui.interact(rect, egui::Id::new("ocr_card_retry"), egui::Sense::click());
                    let visuals = ui.style().interact(&retry);
                    ui.painter().rect_stroke(
                        rect,
                        visuals.corner_radius,
                        egui::Stroke::new(1.0, visuals.text_color()),
                        egui::StrokeKind::Inside,
                    );
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Retry",
                        egui::FontId::proportional(13.0),
                        visuals.text_color(),
                    );
                    if retry.clicked() {
                        flow.card_action = Some(OcrCardAction::Retry);
                    }
                });
            }
            OcrPhase::Failed { detail, .. } => {
                ui.label(egui::RichText::new(detail).small());
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(70.0, crate::theme::CONTROL_H),
                    egui::Sense::click(),
                );
                let retry =
                    ui.interact(rect, egui::Id::new("ocr_card_retry"), egui::Sense::click());
                let visuals = ui.style().interact(&retry);
                ui.painter().rect_stroke(
                    rect,
                    visuals.corner_radius,
                    egui::Stroke::new(1.0, visuals.text_color()),
                    egui::StrokeKind::Inside,
                );
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Retry",
                    egui::FontId::proportional(13.0),
                    visuals.text_color(),
                );
                if retry.clicked() {
                    flow.card_action = Some(OcrCardAction::Retry);
                }
            }
            _ => {}
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f32, y: f32) -> flintshot_core::Point {
        flintshot_core::Point::new(x, y)
    }

    /// A 10×10 gradient image: cropping keeps exactly the sub-block.
    #[test]
    fn region_pixels_crops_the_rect_and_only_the_rect() {
        let mut img = RgbaImage::new(10, 10);
        for y in 0..10 {
            for x in 0..10 {
                #[allow(clippy::cast_possible_truncation, reason = "x, y < 10 fit u8")]
                let (x8, y8) = (x as u8, y as u8);
                img.put_pixel(x, y, image::Rgba([x8, y8, 0, 255]));
            }
        }
        let cropped = region_pixels(&img, Some(Rect::from_xywh(2.0, 3.0, 4.0, 5.0)));
        assert_eq!(cropped.dimensions(), (4, 5));
        assert_eq!(cropped.get_pixel(0, 0).0, [2, 3, 0, 255]);
        assert_eq!(cropped.get_pixel(3, 4).0, [5, 7, 0, 255]);
        // Whole-image contract (Enter).
        assert_eq!(region_pixels(&img, None).dimensions(), (10, 10));
    }

    /// Drag → release produces the rect; a degenerate click leaves
    /// the flow idle (Enter's whole-image path still available).
    #[test]
    fn drag_release_yields_rect_degenerate_click_stays_idle() {
        let mut flow = OcrFlow::default();
        flow.press(pt(10.0, 10.0));
        flow.drag_to(pt(50.0, 40.0));
        assert_eq!(
            flow.release(),
            Some(Rect::from_xywh(10.0, 10.0, 40.0, 30.0))
        );
        flow.press(pt(5.0, 5.0));
        flow.drag_to(pt(5.5, 5.5));
        assert_eq!(flow.release(), None);
        assert_eq!(flow.phase, OcrPhase::Idle);
    }

    /// THE consent pin: the dialog opens exactly once per
    /// missing-model state, and a later success re-arms it.
    #[test]
    fn consent_dialog_opens_exactly_once_per_missing_model_state() {
        let mut flow = OcrFlow::default();
        assert!(
            flow.note_failure("models missing".into(), true),
            "first missing-model failure opens the dialog"
        );
        flow.dismiss_consent();
        assert!(
            !flow.note_failure("models missing again".into(), true),
            "second failure in the same state must not re-open"
        );
        // Non-model failures never open it.
        assert!(!flow.note_failure("engine broke".into(), false));
        // Success changes the state: the next miss may ask again.
        flow.note_done("ok".into(), OcrEngineKind::Ocrs);
        assert!(flow.note_failure("models gone".into(), true));
    }
}
