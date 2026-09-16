//! The header bar — identity, tabs seam, AI seam, overflow menu.
//!
//! A quiet cutout bar across the top of every routed surface
//! (home, editor, loading): cream fill, hairline bottom edge, no
//! grain. Left: the flintshot wordmark. Beside it the **tab-strip
//! region rendering exactly one tab** (the open document, or the
//! "New capture" placeholder on Home) — the inert seam for future
//! tabs (spec §forward-compat 1). The **AI entry slot** is a
//! constant-gated no-op that compiles to nothing today (spec
//! §forward-compat 3). Right: an overflow menu (about, status
//! line, and a diagnostics hint pointing at `flintshot backends` —
//! no new plumbing).

use crate::theme;

/// Fixed header height, logical px — modest by design; the canvas
/// and side panel absorb it without scrolling.
pub(crate) const HEADER_HEIGHT: f32 = 36.0;

/// What the single tab reads on each surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Doc {
    /// The home route: no document yet — the tab is the capture
    /// placeholder.
    Home,
    /// An editor surface: the open capture document.
    Editor,
}

/// The AI entry slot (spec §forward-compat 3): exactly one hidden
/// header slot, nothing attached to tool pods. While `false` the
/// slot compiles to nothing; flipping it is the seam's whole
/// integration point.
const AI_SLOT_LIVE: bool = false;

/// Draw the header across the top of `ui`.
pub(crate) fn show(ui: &mut egui::Ui, doc: Doc, status: Option<&str>) {
    egui::Panel::top("flintshot_header")
        .resizable(false)
        .exact_size(HEADER_HEIGHT)
        .frame(
            egui::Frame::new()
                .fill(theme::palette::PAPER_CREAM)
                .inner_margin(egui::Margin::symmetric(8, 2)),
        )
        .show(ui, |ui| {
            ui.set_min_height(HEADER_HEIGHT - 4.0);
            ui.horizontal_centered(|ui| {
                wordmark(ui);
                ui.add_space(theme::POD_GAP);
                single_tab(ui, doc);
                if AI_SLOT_LIVE {
                    // Unreachable while the seam is inert; the block
                    // exists so the slot has exactly one home.
                    unreachable!("AI_SLOT_LIVE is false until its spec lands");
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    overflow_menu(ui, status);
                });
            });
            // The quiet bottom hairline.
            let rect = ui.max_rect();
            ui.painter().line_segment(
                [rect.left_bottom(), rect.right_bottom()],
                theme::HAIRLINE_STROKE,
            );
        });
}

/// The flintshot wordmark: `ExtraBold` display family, title size —
/// the one place the display weight speaks.
fn wordmark(ui: &mut egui::Ui) {
    let galley = ui.painter().layout_no_wrap(
        "flintshot".to_owned(),
        egui::FontId::new(
            theme::TYPE_TITLE,
            egui::FontFamily::Name(theme::FAMILY_DISPLAY.into()),
        ),
        theme::palette::INK,
    );
    let size = egui::vec2(galley.size().x + 2.0, HEADER_HEIGHT - 4.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let _resp = ui.interact(rect, egui::Id::new("header-identity"), egui::Sense::hover());
    let pos = egui::pos2(rect.min.x, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(pos, galley, theme::palette::INK);
}

/// The tab strip (spec §forward-compat 1): exactly one tab today —
/// a quiet cutout chip, hover-sensed only. Future tabs grow this
/// row; nothing else changes.
fn single_tab(ui: &mut egui::Ui, doc: Doc) {
    let label = match doc {
        Doc::Home => "New capture",
        Doc::Editor => "Capture",
    };
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::proportional(theme::TYPE_CAPTION),
        theme::palette::INK_SOFT,
    );
    let w = 10.0 + galley.size().x + 10.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 22.0), egui::Sense::hover());
    let _resp = ui.interact(rect, egui::Id::new("header-tab"), egui::Sense::hover());
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(theme::CHIP_RADIUS),
        theme::palette::CUTOUT_WHITE,
        theme::HAIRLINE_STROKE,
        egui::StrokeKind::Middle,
    );
    let pos = egui::pos2(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(pos, galley, theme::palette::INK_SOFT);
}

/// The overflow menu (right): about, the status line when one
/// exists, and the backends-diagnostics hint. A `MenuButton` —
/// egui owns its open state.
fn overflow_menu(ui: &mut egui::Ui, status: Option<&str>) {
    let galley = ui.painter().layout_no_wrap(
        "Menu".to_owned(),
        egui::FontId::proportional(theme::TYPE_CAPTION),
        theme::palette::INK_SOFT,
    );
    let w = 8.0 + galley.size().x + 8.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 22.0), egui::Sense::hover());
    let resp = ui.interact(rect, egui::Id::new("header-menu"), egui::Sense::click());
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(theme::CHIP_RADIUS),
        if resp.hovered() {
            theme::palette::HOLIDAY_FAINT
        } else {
            theme::palette::CUTOUT_WHITE
        },
        theme::HAIRLINE_STROKE,
        egui::StrokeKind::Middle,
    );
    let pos = egui::pos2(rect.left() + 8.0, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(pos, galley, theme::palette::INK_SOFT);
    egui::Popup::from_toggle_button_response(&resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(220.0);
            ui.label("flintshot — screenshots, pop-up book era");
            if let Some(status) = status {
                ui.separator();
                ui.label(
                    egui::RichText::new(status)
                        .color(theme::palette::INK_SOFT)
                        .size(theme::TYPE_CAPTION),
                );
            }
            ui.separator();
            ui.label(
                egui::RichText::new("Capture diagnostics: run `flintshot backends`")
                    .color(theme::palette::INK_SOFT)
                    .size(theme::TYPE_CAPTION),
            );
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(doc: Doc, events: Vec<egui::Event>) -> egui::Context {
        let ctx = egui::Context::default();
        // The wordmark speaks the display family — installed by
        // `theme::apply` in every real window.
        crate::theme::apply(&ctx);
        let raw = egui::RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(1024.0, 640.0),
            )),
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            show(ui, doc, Some("connected"));
            let _ = egui::CentralPanel::default().show(ui, |ui| {
                ui.allocate_space(egui::vec2(1.0, 1.0));
            });
        });
        ctx
    }

    /// The header registers its vocabulary on every surface: the
    /// tab, the overflow menu — and never the AI slot.
    #[test]
    fn header_registers_tab_and_menu_but_no_ai_slot() {
        for doc in [Doc::Home, Doc::Editor] {
            let ctx = frame(doc, vec![]);
            assert!(
                ctx.read_response(egui::Id::new("header-tab")).is_some(),
                "{doc:?}: exactly one tab renders"
            );
            assert!(
                ctx.read_response(egui::Id::new("header-menu")).is_some(),
                "{doc:?}: the overflow menu renders"
            );
            assert!(
                ctx.read_response(egui::Id::new("header-ai")).is_none(),
                "{doc:?}: the AI slot stays hidden"
            );
        }
    }

    /// The header stays modest: its panel leaves the central body
    /// the vast majority of a 640-tall screen.
    #[test]
    fn header_height_stays_modest() {
        let ctx = frame(Doc::Home, vec![]);
        // The header's own top panel is the first thing under the
        // root: measure via the tab's rect — it must sit within the
        // first 48 px.
        let tab = ctx.read_response(egui::Id::new("header-tab")).expect("tab");
        assert!(
            tab.rect.max.y <= 48.0,
            "header taller than intended: {:?}",
            tab.rect
        );
    }
}
