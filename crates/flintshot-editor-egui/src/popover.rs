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
    /// `EditorState::text_color` — the Text tool's own color.
    Text,
}

/// Stable pod-button id (the popup hangs off its response).
const POD_ID: &str = "color-stroke-pod";

/// Draw the picker pod button: a cutout chip whose leading square
/// shows the live color, labeled `Color · Stroke`. Opens the
/// popover on click.
pub(crate) fn pod_button(ui: &mut egui::Ui, state: &mut EditorState, target: ColorTarget) {
    // The pod's chip shows — and the popover edits — the same source
    // the panel's Text section reads (session draft > selection >
    // defaults); a colour change applies as one discrete gesture.
    let color = match target {
        ColorTarget::Text => panel::effective_text_style(state).color,
    };
    let resp = labeled_color_chip(ui, color, target);
    egui::Popup::from_toggle_button_response(&resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(200.0);
            egui::Frame::new()
                .fill(theme::palette::PAPER_CREAM)
                .corner_radius(egui::CornerRadius::same(theme::CUTOUT_RADIUS))
                .inner_margin(egui::Margin::same(theme::POD_PAD))
                .stroke(theme::HAIRLINE_STROKE)
                .show(ui, |ui| {
                    ui.vertical(|ui| match target {
                        ColorTarget::Text => text_color_bay(ui, state),
                    });
                });
        });
}

/// The Text colour bay the pod's popover shows: the shared swatch row
/// reading from — and writing through to — the same source the panel
/// edits (session draft > selection > defaults), one discrete gesture
/// per pick. Split out of `pod_button` so the interaction is drivable
/// at the root ui (the popup machinery is orthogonal to the retarget).
pub(crate) fn text_color_bay(ui: &mut egui::Ui, state: &mut EditorState) {
    let before = panel::effective_text_style(state).color;
    let mut picked = before;
    ui.horizontal(|ui| panel::swatch_row(ui, &mut picked));
    if picked != before {
        let mut st = panel::effective_text_style(state);
        st.color = picked;
        panel::commit_text_style_change(state, st);
    }
}

/// The pod chip itself: current-color square + label, galley-sized,
/// cutout-styled like every other chip.
fn labeled_color_chip(ui: &mut egui::Ui, color: Color, target: ColorTarget) -> egui::Response {
    let label = match target {
        ColorTarget::Text => "Text color",
    };
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::proportional(theme::TYPE_MD),
        theme::palette::INK,
    );
    let galley_size = galley.size();
    let swatch = 16.0;
    let pad = 10.0;
    let width = pad + swatch + 8.0 + galley_size.x + pad;
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(width, theme::CONTROL_H), egui::Sense::hover());
    let resp = ui
        .interact(rect, egui::Id::new(POD_ID), egui::Sense::click())
        .on_hover_text("Color and stroke width");
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(theme::POD_RADIUS),
        theme::palette::PAPER_CREAM,
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
        crate::theme::palette::premultiplied(r, g, b, a),
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

    fn screen() -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(1024.0, 768.0),
            )),
            ..egui::RawInput::default()
        }
    }

    /// Lay the pod out + settle, then click it — returns the ctx.
    fn opened_popup(target: ColorTarget) -> egui::Context {
        let mut s = state();
        opened_popup_over(&mut s, target)
    }

    /// [`opened_popup`] over caller-owned state (the retarget tests
    /// need a selected text annotation in place).
    fn opened_popup_over(s: &mut EditorState, target: ColorTarget) -> egui::Context {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(screen(), |ui| {
            pod_button(ui, s, target);
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
                pod_button(ui, s, target);
            },
        );
        ctx
    }

    #[test]
    fn text_pod_opens_the_swatch_bay() {
        let ctx = opened_popup(ColorTarget::Text);
        // Swatches register (berry = the leading default).
        let berry = theme::palette::SWATCHES[0];
        assert!(
            ctx.read_response(egui::Id::new(("swatch", berry.r(), berry.g(), berry.b())))
                .is_some(),
            "the popover offers the swatch row"
        );
        // Custom color chip registers.
        assert!(ctx.read_response(egui::Id::new("custom_color")).is_some());
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

    // --- Selection retarget (text-select-edit Task 006) ---

    /// With a selected text annotation, a swatch click restyles it
    /// (colour only — the read came from the selection, so every
    /// other field survives), syncs the defaults, and records one
    /// undo entry.
    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "the size is assigned verbatim from the fixture, never computed"
    )]
    fn text_pod_swatch_click_restyles_selection() {
        let mut s = state();
        s.tool = flintshot_core::Tool::Text;
        s.anns.push(flintshot_core::Annotation::text(
            flintshot_core::Point::new(4.0, 4.0),
            "hi",
            flintshot_core::text::TextStyle::simple(23.0, flintshot_core::Color::rgb(0, 0, 255)),
        ));
        s.selected = vec![0];
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(screen(), |ui| {
            text_color_bay(ui, &mut s);
        });
        // A swatch distinct from the annotation's blue.
        let pick = *theme::palette::SWATCHES
            .iter()
            .find(|sw| **sw != panel::to_color32(flintshot_core::Color::rgb(0, 0, 255)))
            .expect("a non-blue swatch exists");
        let id = egui::Id::new(("swatch", pick.r(), pick.g(), pick.b()));
        let at = ctx
            .read_response(id)
            .expect("swatch registered")
            .rect
            .center();
        let press = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let _ = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::PointerMoved(at), press(at, true)],
                ..screen()
            },
            |ui| {
                text_color_bay(ui, &mut s);
            },
        );
        let _ = ctx.run_ui(
            egui::RawInput {
                events: vec![press(at, false)],
                ..screen()
            },
            |ui| {
                text_color_bay(ui, &mut s);
            },
        );
        let want = flintshot_core::Color::rgb(pick.r(), pick.g(), pick.b());
        match &s.anns[0] {
            flintshot_core::Annotation::Text {
                color, font_size, ..
            } => {
                assert_eq!(*color, want, "the selection recolours");
                assert_eq!(*font_size, 23.0, "the read came from the selection");
            }
            other => panic!("expected Text, got {other:?}"),
        }
        assert_eq!(s.text_color, want, "defaults synced");
        assert_eq!(s.undo.len(), 1, "one entry for the discrete click");
    }
}
