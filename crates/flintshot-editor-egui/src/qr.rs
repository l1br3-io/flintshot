//! The QR tool's editor-side flow: region drag → bridge request →
//! clipboard + result card.
//!
//! Geometry reuses [`crate::pick`]'s drag-rect machine (the OCR
//! tool's exact selection grammar: a crop-style region drag that
//! never creates an annotation, Enter = whole image), and the decode
//! call rides the page bridge's request/poll seam so no detection
//! ever runs on the UI thread. Frame tests in `app.rs` drive the
//! whole flow through a mock bridge.
//!
//! The card's Open affordance is click-only and shows only for
//! payloads [`flintshot_core::is_url`] vetted (absolute http(s)) —
//! the click hands the URL to the host's [`crate::shell::EditorPageBridge::open_url`];
//! nothing ever opens by itself.

use flintshot_core::geom::Rect;
use flintshot_core::{crop_region, is_url};
use image::RgbaImage;

use crate::pick::PickState;

/// The pixels the engine receives for `rect` (`None` = the whole
/// image — Enter's contract). Only this crop ever leaves the editor:
/// the region-crop-before-decode rule, pinned by core's fixture
/// suite and by the frame tests' request-dims assertions.
#[must_use]
pub(crate) fn region_pixels(image: &RgbaImage, rect: Option<Rect>) -> RgbaImage {
    match rect {
        Some(rect) => crop_region(image, rect),
        None => image.clone(),
    }
}

/// The QR flow's phases. `Idle` doubles as "armed": the tool takes a
/// drag or Enter whenever no pass is in flight. `Done`/`Failed`
/// persist as the result card until the next pass (or Esc) so
/// switching tools never loses the payloads.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) enum QrPhase {
    /// No pass in flight and no result to show (the resting state).
    #[default]
    Idle,
    /// A pass is in flight (the region already left on the bridge).
    Pending,
    /// The last pass succeeded: every payload, largest-first.
    Done {
        /// Decoded payloads, largest-area first.
        payloads: Vec<String>,
    },
    /// The last pass failed; `detail` already names the re-capture
    /// cure.
    Failed {
        /// Actionable failure copy.
        detail: String,
    },
}

/// Intent fired by the result card's buttons (the app consumes it
/// after the panel pass).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QrCardAction {
    /// Copy payload at the row index again.
    Copy(usize),
    /// Open payload at the row index (a URL row's Open click — the
    /// only thing that ever opens a decoded URL).
    Open(usize),
    /// Re-run the last region.
    Retry,
}

/// The QR tool's state machine.
#[derive(Debug, Default)]
pub(crate) struct QrFlow {
    /// Current phase.
    pub phase: QrPhase,
    pick: PickState,
    /// The pixels of the last request — retry re-sends exactly these.
    last_pixels: Option<RgbaImage>,
    /// Card action fired this frame; the app takes it after the
    /// panel pass.
    pub card_action: Option<QrCardAction>,
}

impl QrFlow {
    /// Whether a region press or Enter may start a pass (never
    /// mid-pass, never mid-drag).
    #[must_use]
    pub fn accepts_press(&self) -> bool {
        self.phase != QrPhase::Pending && !self.dragging()
    }

    /// The result card is showing (Done or Failed).
    #[must_use]
    pub fn has_result(&self) -> bool {
        matches!(self.phase, QrPhase::Done { .. } | QrPhase::Failed { .. })
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
        self.phase = QrPhase::Pending;
    }

    /// The pixels to re-send on retry, if a pass ever left.
    #[must_use]
    pub fn retry_pixels(&self) -> Option<&RgbaImage> {
        self.last_pixels.as_ref()
    }

    /// The pass succeeded (payloads largest-first; the clipboard
    /// takes the first — the code the user framed).
    pub fn note_done(&mut self, payloads: Vec<String>) {
        self.phase = QrPhase::Done { payloads };
    }

    /// The pass failed (the copy already names the cure).
    pub fn note_failure(&mut self, detail: String) {
        self.phase = QrPhase::Failed { detail };
    }

    /// Esc: back to Idle (card gone, drag cancelled).
    pub fn reset(&mut self) {
        self.pick.cancel();
        self.phase = QrPhase::Idle;
    }
}

/// The result card: one row per payload (monospace text, Copy — and
/// Open only when [`is_url`] vetted the payload), Retry below. The
/// failure shape shows the cure copy instead. No-op unless a result
/// is showing.
pub(crate) fn show_result_card(ui: &mut egui::Ui, flow: &mut QrFlow) {
    if !flow.has_result() {
        return;
    }
    crate::widgets::section(
        ui,
        egui::Id::new("panel-section-qr"),
        "QR result",
        true,
        |ui| match flow.phase.clone() {
            QrPhase::Done { payloads } => {
                for (idx, payload) in payloads.iter().enumerate() {
                    ui.horizontal(|ui| {
                        // Truncating monospace label: the chips must
                        // stay inside the row's visible width — a
                        // clipped chip can't be clicked (frame-pinned
                        // by the Open test).
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(payload.as_str()).monospace().small(),
                            )
                            .truncate(),
                        );
                        if chip(ui, &format!("qr_card_copy_{idx}"), "Copy").clicked() {
                            flow.card_action = Some(QrCardAction::Copy(idx));
                        }
                        // THE security pin: Open appears only for
                        // vetted absolute http(s) URLs, and the click
                        // is the only thing that fires it.
                        if is_url(payload)
                            && chip(ui, &format!("qr_card_open_{idx}"), "Open").clicked()
                        {
                            flow.card_action = Some(QrCardAction::Open(idx));
                        }
                    });
                }
                retry_button(ui, flow);
            }
            QrPhase::Failed { detail } => {
                ui.label(egui::RichText::new(detail).small());
                retry_button(ui, flow);
            }
            _ => {}
        },
    );
}

/// The card's Retry (explicit id: frame tests click this response).
fn retry_button(ui: &mut egui::Ui, flow: &mut QrFlow) {
    if chip(ui, "qr_card_retry", "Retry").clicked() {
        flow.card_action = Some(QrCardAction::Retry);
    }
}

/// A small outlined, click-sensible chip with an explicit id (the
/// OCR card's hand-painted response pattern — frame tests click
/// these by id).
fn chip(ui: &mut egui::Ui, id: &str, label: &str) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(70.0, crate::theme::CONTROL_H),
        egui::Sense::click(),
    );
    let response = ui.interact(rect, egui::Id::new(id), egui::Sense::click());
    let visuals = ui.style().interact(&response);
    ui.painter().rect_stroke(
        rect,
        visuals.corner_radius,
        egui::Stroke::new(1.0, visuals.text_color()),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(13.0),
        visuals.text_color(),
    );
    response
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
        let mut flow = QrFlow::default();
        flow.press(pt(10.0, 10.0));
        flow.drag_to(pt(50.0, 40.0));
        assert_eq!(
            flow.release(),
            Some(Rect::from_xywh(10.0, 10.0, 40.0, 30.0))
        );
        flow.press(pt(5.0, 5.0));
        flow.drag_to(pt(5.5, 5.5));
        assert_eq!(flow.release(), None);
        assert_eq!(flow.phase, QrPhase::Idle);
    }

    /// Done keeps every payload (the card lists them all); Failed
    /// keeps the cure copy; both persist until Esc.
    #[test]
    fn done_lists_every_payload_failure_keeps_the_cure() {
        let mut flow = QrFlow::default();
        flow.note_done(vec!["https://a.example".into(), "second".into()]);
        assert_eq!(
            flow.phase,
            QrPhase::Done {
                payloads: vec!["https://a.example".into(), "second".into()]
            }
        );
        assert!(flow.has_result());
        flow.note_failure("no QR code found — move closer".into());
        assert_eq!(
            flow.phase,
            QrPhase::Failed {
                detail: "no QR code found — move closer".into()
            }
        );
        flow.reset();
        assert_eq!(flow.phase, QrPhase::Idle);
    }
}
