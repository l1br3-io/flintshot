//! The color / stroke picker popover — Gradia pattern #4.
//!
//! One pod button ("Color · Stroke") collapses the whole picker bay
//! behind a single popover: the seven swatches, the custom-color
//! chip, and (for shape colors) the S / M / L stroke presets +
//! width slider. Every commit applies live — the next draft
//! annotation is created from the committed values; there is no
//! OK step. Text's pod edits the decoupled text color and omits
//! the stroke row (text size lives in the contextual section).

use flintshot_core::{Color, EditorState};

use crate::panel;
use crate::theme;

/// Which color the popover edits this time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColorTarget {
    /// `EditorState::color` — shapes, markers, numbers.
    Shape,
    /// `EditorState::text_color` — the Text tool's own color.
    Text,
}

/// Stable pod-button id (the popup hangs off its response).
const POD_ID: &str = "color-stroke-pod";

/// Draw the picker pod button: a cutout chip whose leading square
/// shows the live color, labeled `Color · Stroke`. Opens the
/// popover on click.
pub(crate) fn pod_button(ui: &mut egui::Ui, state: &mut EditorState, target: ColorTarget) {
    let color = match target {
        ColorTarget::Shape => state.color,
        ColorTarget::Text => state.text_color,
    };
    let resp = labeled_color_chip(ui, color, target);
    egui::Popup::from_toggle_button_response(&resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(200.0);
            egui::Frame::new()
                .fill(theme::palette::CUTOUT_WHITE)
                .corner_radius(egui::CornerRadius::same(theme::POD_RADIUS))
                .inner_margin(egui::Margin::same(theme::POD_PAD))
                .stroke(theme::HAIRLINE_STROKE)
                .show(ui, |ui| {
                    ui.vertical(|ui| match target {
                        ColorTarget::Shape => {
                            ui.horizontal(|ui| panel::swatch_row(ui, &mut state.color));
                            ui.add_space(4.0);
                            ui.horizontal(|ui| panel::stroke_row(ui, state));
                        }
                        ColorTarget::Text => {
                            ui.horizontal(|ui| panel::swatch_row(ui, &mut state.text_color));
                        }
                    });
                });
        });
}

/// The pod chip itself: current-color square + label, galley-sized,
/// cutout-styled like every other chip.
fn labeled_color_chip(ui: &mut egui::Ui, color: Color, target: ColorTarget) -> egui::Response {
    let label = match target {
        ColorTarget::Shape => "Color · Stroke",
        ColorTarget::Text => "Text color",
    };
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::proportional(theme::TYPE_BODY),
        theme::palette::INK,
    );
    let galley_size = galley.size();
    let swatch = 14.0;
    let pad = 10.0;
    let width = pad + swatch + 8.0 + galley_size.x + pad;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 28.0), egui::Sense::hover());
    let resp = ui
        .interact(rect, egui::Id::new(POD_ID), egui::Sense::click())
        .on_hover_text("Color and stroke width");
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(theme::CHIP_RADIUS),
        theme::palette::CUTOUT_WHITE,
        theme::HAIRLINE_STROKE,
        egui::StrokeKind::Middle,
    );
    let swatch_rect = egui::Rect::from_min_size(
        egui::pos2(
            rect.left() + pad,
            f32::midpoint(rect.top(), rect.bottom()) - swatch / 2.0,
        ),
        egui::vec2(swatch, swatch),
    );
    let (r, g, b, a) = (color.r, color.g, color.b, color.a);
    ui.painter().rect_filled(
        swatch_rect,
        egui::CornerRadius::same(3),
        egui::Color32::from_rgba_unmultiplied(r, g, b, a),
    );
    let pos = egui::pos2(
        swatch_rect.right() + 8.0,
        rect.center().y - galley_size.y / 2.0,
    );
    ui.painter().galley(pos, galley, theme::palette::INK);
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> EditorState {
        let img = image::RgbaImage::from_raw(64, 48, vec![255; 64 * 48 * 4])
            .expect("raw buffer matches 64*48*4");
        EditorState::new(img)
    }

    /// Lay the pod out + settle, then click it — returns the ctx.
    fn opened_popup(target: ColorTarget) -> egui::Context {
        let mut s = state();
        let ctx = egui::Context::default();
        let screen = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(1024.0, 768.0),
            )),
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(screen(), |ui| {
            pod_button(ui, &mut s, target);
        });
        let pod = ctx
            .read_response(egui::Id::new(POD_ID))
            .expect("pod registered");
        let at = pod.rect.center();
        let _ = ctx.run_ui(
            egui::RawInput {
                events: vec![
                    egui::Event::PointerMoved(at),
                    egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                    egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..screen()
            },
            |ui| {
                pod_button(ui, &mut s, target);
            },
        );
        ctx
    }

    #[test]
    fn shape_pod_opens_the_full_picker_bay() {
        let ctx = opened_popup(ColorTarget::Shape);
        // Swatches register (berry = the leading default).
        let berry = theme::palette::SWATCHES[0];
        assert!(
            ctx.read_response(egui::Id::new(("swatch", berry.r(), berry.g(), berry.b())))
                .is_some(),
            "the popover offers the swatch row"
        );
        // Custom color chip + stroke presets + slider register.
        assert!(ctx.read_response(egui::Id::new("custom_color")).is_some());
        for i in 0..theme::STROKE_PRESETS.len() {
            assert!(
                ctx.read_response(egui::Id::new(("stroke", i))).is_some(),
                "stroke preset {i} offered"
            );
        }
    }

    #[test]
    fn text_pod_omits_the_stroke_row() {
        let ctx = opened_popup(ColorTarget::Text);
        let berry = theme::palette::SWATCHES[0];
        assert!(
            ctx.read_response(egui::Id::new(("swatch", berry.r(), berry.g(), berry.b())))
                .is_some(),
            "text target still offers swatches"
        );
        for i in 0..theme::STROKE_PRESETS.len() {
            assert!(
                ctx.read_response(egui::Id::new(("stroke", i))).is_none(),
                "text target has no stroke row"
            );
        }
    }
}
