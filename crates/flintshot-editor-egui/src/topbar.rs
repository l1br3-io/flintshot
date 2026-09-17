//! The topbar — zone 1 of the five-zone window: a 44 px forest-deep
//! cutout bar with the flame brand + route line, the `Home ▸ Editor`
//! breadcrumb (current stage gold + bold), exactly three hero ghost
//! buttons — **Capture** and **Copy** with the gold hero-key border,
//! **Open** plain — and the `⋯` overflow.
//!
//! One component serves every routed surface ([`Surface`]): the
//! heroes stay, the overflow contents vary by route (editor:
//! Save/Upload/Paste; home/loading: about + note; all: the Settings
//! entry, inert until the settings route lands). Worker editor
//! windows draw the same bar themselves.

use egui::Color32;

use crate::theme;
use crate::widgets;

/// Topbar height, logical px — the mockup's zone contract.
pub(crate) const TOPBAR_HEIGHT: f32 = 44.0;

/// Which surface the topbar serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Surface {
    /// The welcome page.
    Home,
    /// An editor surface (worker window or routed editor).
    Editor,
    /// The capture-progress page.
    Loading,
}

/// What a topbar interaction fired this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopbarAction {
    /// The hero Capture button.
    Capture,
    /// The hero Copy button.
    Copy,
    /// The hero Open button.
    Open,
    /// The overflow's Save item.
    Save,
    /// The overflow's Upload item.
    Upload,
    /// The overflow's Paste item.
    Paste,
    /// The overflow's Settings entry.
    Settings,
}

/// Which topbar buttons are live this frame, as a compact bitmask —
/// disabled buttons render in the cream-dim style and swallow input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Enable(u8);

impl Enable {
    /// The hero Capture button.
    pub(crate) const CAPTURE: u8 = 1;
    /// The hero Copy button.
    pub(crate) const COPY: u8 = 1 << 1;
    /// The hero Open button.
    pub(crate) const OPEN: u8 = 1 << 2;
    /// The overflow's Save / Upload / Paste items (editor routes).
    pub(crate) const EDITOR_ITEMS: u8 = 1 << 3;

    /// All actions live (the routed editor surface).
    pub(crate) const ALL: Enable =
        Enable(Self::CAPTURE | Self::COPY | Self::OPEN | Self::EDITOR_ITEMS);

    /// The worker editor: Open / Paste have no bridge, so they stay
    /// disabled; Capture / Copy / Save / Upload are local.
    pub(crate) const WORKER: Enable = Enable(Self::CAPTURE | Self::COPY);

    /// Nothing live (home before its task wires the heroes, and the
    /// loading route while a grab is in flight).
    pub(crate) const NONE: Enable = Enable(0);

    /// Whether `flag` is enabled.
    #[must_use]
    pub(crate) const fn has(self, flag: u8) -> bool {
        self.0 & flag != 0
    }
}

/// Draw the topbar across the top of `ui` and report what fired.
pub(crate) fn show(
    ui: &mut egui::Ui,
    surface: Surface,
    status: Option<&str>,
    enable: Enable,
) -> Option<TopbarAction> {
    use crate::theme::palette as p;
    let mut fired = None;
    egui::Panel::top("flintshot_topbar")
        .resizable(false)
        .exact_size(TOPBAR_HEIGHT)
        .frame(
            egui::Frame::new()
                .fill(p::FOREST_DEEP)
                .inner_margin(egui::Margin::symmetric(14, 0)),
        )
        .show(ui, |ui| {
            ui.set_min_height(TOPBAR_HEIGHT - 4.0);
            ui.horizontal_centered(|ui| {
                brand(ui, surface);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    overflow(ui, surface, status, enable, &mut fired);
                    ui.add_space(theme::SPACE_SCALE[1]);
                    hero(
                        ui,
                        "topbar-open",
                        "🗀",
                        "Open",
                        false,
                        enable.has(Enable::OPEN),
                        &mut fired,
                    );
                    hero(
                        ui,
                        "topbar-copy",
                        "⧉",
                        "Copy",
                        true,
                        enable.has(Enable::COPY),
                        &mut fired,
                    );
                    hero(
                        ui,
                        "topbar-capture",
                        "✚",
                        "Capture",
                        true,
                        enable.has(Enable::CAPTURE),
                        &mut fired,
                    );
                });
            });
            // The quiet cream bottom edge, per the mockup.
            let rect = ui.max_rect();
            ui.painter().line_segment(
                [rect.left_bottom(), rect.right_bottom()],
                egui::Stroke::new(2.0, p::with_alpha(p::CREAM_TEXT, 36)),
            );
        });
    fired
}

/// The flame badge + `Flintshot | Route` identity + breadcrumb.
fn brand(ui: &mut egui::Ui, surface: Surface) {
    use crate::theme::palette as p;
    // The gold flame badge, 24 px.
    let (rect, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::hover());
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(8),
        p::GOLD,
        egui::Stroke::NONE,
        egui::StrokeKind::Middle,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "🔥",
        egui::FontId::proportional(theme::TYPE_SM),
        p::FOREST_INK,
    );
    ui.add_space(4.0);
    // `Flintshot` in the display weight, then the route in dim.
    let name = ui.painter().layout_no_wrap(
        "Flintshot".to_owned(),
        egui::FontId::new(
            theme::TYPE_MD,
            egui::FontFamily::Name(theme::FAMILY_DISPLAY.into()),
        ),
        p::CREAM_TEXT,
    );
    let (rect, _) = ui.allocate_exact_size(name.size(), egui::Sense::hover());
    ui.painter().galley(rect.min, name, p::CREAM_TEXT);
    let route = match surface {
        Surface::Editor => " | Editor",
        Surface::Home => "",
        Surface::Loading => " | Loading",
    };
    if !route.is_empty() {
        let galley = ui.painter().layout_no_wrap(
            route.to_owned(),
            egui::FontId::proportional(theme::TYPE_MD),
            p::CREAM_DIM,
        );
        let (rect, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
        ui.painter().galley(rect.min, galley, p::CREAM_DIM);
    }
    ui.add_space(theme::POD_GAP);
    breadcrumb(ui, surface);
}

/// The `Home ▸ Editor` breadcrumb: past stages dim, current gold.
fn breadcrumb(ui: &mut egui::Ui, surface: Surface) {
    use crate::theme::palette as p;
    let (stages, current) = match surface {
        Surface::Home => (&["Home"][..], 0usize),
        Surface::Editor => (&["Home", "Editor"][..], 1),
        Surface::Loading => (&["Home", "Loading"][..], 1),
    };
    for (i, stage) in stages.iter().enumerate() {
        let cur = i == current;
        let font = if cur {
            egui::FontId::new(
                theme::TYPE_SM,
                egui::FontFamily::Name(theme::FAMILY_DISPLAY.into()),
            )
        } else {
            egui::FontId::proportional(theme::TYPE_SM)
        };
        let galley = ui.painter().layout_no_wrap(
            (*stage).to_owned(),
            font,
            if cur { p::GOLD } else { p::CREAM_DIM },
        );
        let (rect, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
        ui.painter()
            .galley(rect.min, galley, if cur { p::GOLD } else { p::CREAM_DIM });
        if i + 1 < stages.len() {
            ui.painter().text(
                ui.cursor().min,
                egui::Align2::LEFT_CENTER,
                "▸",
                egui::FontId::proportional(theme::TYPE_XS),
                p::with_alpha(p::CREAM_DIM, 128),
            );
            ui.add_space(14.0);
        }
    }
}

/// One hero ghost button; registers under `id` and reports clicks
/// through `fired`.
fn hero(
    ui: &mut egui::Ui,
    id: &'static str,
    icon: &str,
    label: &str,
    hero_key: bool,
    enabled: bool,
    fired: &mut Option<TopbarAction>,
) {
    let action = match id {
        "topbar-capture" => TopbarAction::Capture,
        "topbar-copy" => TopbarAction::Copy,
        _ => TopbarAction::Open,
    };
    let resp = ui
        .scope(|ui| {
            if !enabled {
                ui.disable();
            }
            widgets::ghost_button(ui, egui::Id::new(id), icon, label, hero_key)
        })
        .inner;
    if enabled && resp.clicked() {
        *fired = Some(action);
    }
}

/// The `⋯` overflow: an about note + route-varying items. Editor
/// surfaces get Save / Upload / Paste; every surface gets the
/// Settings entry (inert until its route lands — the action is
/// reported and callers ignore it for now).
fn overflow(
    ui: &mut egui::Ui,
    surface: Surface,
    status: Option<&str>,
    enable: Enable,
    fired: &mut Option<TopbarAction>,
) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(30.0, 30.0), egui::Sense::hover());
    let resp = ui
        .interact(rect, egui::Id::new("topbar-overflow"), egui::Sense::click())
        .on_hover_text("More");
    let fill = if resp.hovered() {
        theme::palette::with_alpha(theme::palette::CREAM_TEXT, 36)
    } else {
        theme::palette::with_alpha(theme::palette::CREAM_TEXT, 15)
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(theme::POD_RADIUS),
        fill,
        egui::Stroke::new(
            1.5,
            theme::palette::with_alpha(theme::palette::CREAM_TEXT, 71),
        ),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "⋯",
        egui::FontId::proportional(theme::TYPE_MD),
        theme::palette::CREAM_TEXT,
    );
    egui::Popup::from_toggle_button_response(&resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(212.0);
            if enable.has(Enable::EDITOR_ITEMS) {
                if om_row(ui, "topbar-save", "⤓", "Save", Some("Ctrl+S")) {
                    *fired = Some(TopbarAction::Save);
                }
                if om_row(ui, "topbar-upload", "⇧", "Upload", None) {
                    *fired = Some(TopbarAction::Upload);
                }
                if om_row(ui, "topbar-paste", "📋", "Paste", Some("Ctrl+V")) {
                    *fired = Some(TopbarAction::Paste);
                }
                om_separator(ui);
            }
            // The about row: informational, hover-only.
            om_row_about(ui, "ℹ", "About Flintshot");
            om_note(ui, status);
            om_separator(ui);
            if om_row(ui, "topbar-settings", "⚙", "Settings", None) {
                *fired = Some(TopbarAction::Settings);
            }
            let _ = surface;
        });
}

/// One clickable overflow row: icon + label + optional kbd chip,
/// paper-cream with a dim hover.
fn om_row(ui: &mut egui::Ui, id: &'static str, icon: &str, label: &str, kbd: Option<&str>) -> bool {
    use crate::theme::palette as p;
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), egui::Sense::hover());
    let resp = ui.interact(rect, egui::Id::new(id), egui::Sense::click());
    let fill = if resp.hovered() {
        p::PAPER_DIM
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(8),
        fill,
        egui::Stroke::NONE,
        egui::StrokeKind::Middle,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 10.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        icon,
        egui::FontId::proportional(theme::TYPE_SM),
        p::INK,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 32.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(theme::TYPE_SM),
        p::INK,
    );
    if let Some(kbd) = kbd {
        ui.painter().text(
            egui::pos2(rect.right() - 10.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            kbd,
            egui::FontId::proportional(theme::TYPE_XS),
            p::INK_SOFT,
        );
    }
    resp.clicked()
}

/// The about row — informational (the About "action" is the note
/// itself), so hover-only with the same row styling.
fn om_row_about(ui: &mut egui::Ui, icon: &str, label: &str) {
    use crate::theme::palette as p;
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), egui::Sense::hover());
    ui.painter().text(
        egui::pos2(rect.left() + 10.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        icon,
        egui::FontId::proportional(theme::TYPE_SM),
        p::INK,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 32.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(theme::TYPE_SM),
        p::INK,
    );
}

/// The diagnostics note under the about row: the status line when
/// one exists + the backends hint.
fn om_note(ui: &mut egui::Ui, status: Option<&str>) {
    use crate::theme::palette as p;
    let mut note = "Diagnostics: run `flintshot backends`".to_owned();
    if let Some(status) = status {
        note.push_str("\nStatus: ");
        note.push_str(status);
    }
    let galley = ui.painter().layout_no_wrap(
        note,
        egui::FontId::proportional(theme::TYPE_XS),
        p::INK_SOFT,
    );
    let (rect, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
    ui.painter().galley(rect.min, galley, p::INK_SOFT);
}

/// The hairline separator between overflow groups.
fn om_separator(ui: &mut egui::Ui) {
    use crate::theme::palette as p;
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        egui::Stroke::new(1.0, p::with_alpha(p::FOREST_INK, 38)),
    );
    ui.add_space(4.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(time: f64, events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            time: Some(time),
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(640.0, 480.0),
            )),
            ..egui::RawInput::default()
        }
    }

    fn click_events(at: egui::Pos2) -> Vec<egui::Event> {
        vec![
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
        ]
    }

    /// The topbar is a top strip: the central panel starts at its
    /// 44 px edge.
    #[test]
    fn topbar_is_a_44px_top_strip() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let central_top = std::cell::Cell::new(0.0_f32);
        let _ = ctx.run_ui(screen(0.0, vec![]), |ui| {
            let _ = show(ui, Surface::Editor, None, Enable::ALL);
            let central = egui::CentralPanel::default().show(ui, |ui| {
                ui.allocate_space(egui::vec2(1.0, 1.0));
            });
            central_top.set(central.response.rect.top());
        });
        assert!(
            (central_top.get() - TOPBAR_HEIGHT).abs() < 1.0,
            "the topbar strips {} px off the top, central top {}",
            TOPBAR_HEIGHT,
            central_top.get()
        );
    }

    /// The heroes + the overflow register on every surface; the AI
    /// slot never returns.
    #[test]
    fn heroes_and_overflow_register_ai_stays_gone() {
        for surface in [Surface::Home, Surface::Editor, Surface::Loading] {
            let ctx = egui::Context::default();
            theme::apply(&ctx);
            let _ = ctx.run_ui(screen(0.0, vec![]), |ui| {
                show(ui, surface, None, Enable::ALL);
            });
            for id in [
                "topbar-capture",
                "topbar-copy",
                "topbar-open",
                "topbar-overflow",
            ] {
                assert!(
                    ctx.read_response(egui::Id::new(id)).is_some(),
                    "{surface:?}: {id} registers"
                );
            }
            assert!(
                ctx.read_response(egui::Id::new("header-ai")).is_none()
                    && ctx.read_response(egui::Id::new("topbar-ai")).is_none(),
                "{surface:?}: the AI slot stays hidden"
            );
        }
    }

    /// Clicking a hero reports its action; a disabled hero swallows
    /// the click (the cream-dim state).
    #[test]
    fn heroes_fire_when_enabled_and_swallow_when_disabled() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        // Locate the Capture hero.
        let _ = ctx.run_ui(screen(0.0, vec![]), |ui| {
            show(ui, Surface::Editor, None, Enable::ALL);
        });
        let capture = ctx
            .read_response(egui::Id::new("topbar-capture"))
            .expect("capture hero")
            .rect;
        let fired = std::cell::Cell::new(None);
        let _ = ctx.run_ui(screen(0.016, click_events(capture.center())), |ui| {
            let f = show(ui, Surface::Editor, None, Enable::ALL);
            fired.set(f);
        });
        assert_eq!(fired.get(), Some(TopbarAction::Capture));

        // Worker: Open is disabled — the same click reports nothing.
        let open = ctx
            .read_response(egui::Id::new("topbar-open"))
            .expect("open hero")
            .rect;
        let _ = ctx.run_ui(screen(0.032, click_events(open.center())), |ui| {
            let f = show(ui, Surface::Editor, None, Enable::WORKER);
            fired.set(f);
        });
        assert_eq!(fired.get(), None, "a disabled hero swallows clicks");
    }

    /// The overflow opens, Save fires, and the Settings entry is
    /// present but callers receive it inertly until Task 10.
    #[test]
    fn overflow_save_fires_and_settings_is_present() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let _ = ctx.run_ui(screen(0.0, vec![]), |ui| {
            show(ui, Surface::Editor, Some("connected"), Enable::ALL);
        });
        let ovf = ctx
            .read_response(egui::Id::new("topbar-overflow"))
            .expect("overflow")
            .rect;
        // Open the menu.
        let _ = ctx.run_ui(screen(0.016, click_events(ovf.center())), |ui| {
            show(ui, Surface::Editor, Some("connected"), Enable::ALL);
        });
        // Settle passes: the popup anchors at the screen centre on
        // its open pass and re-anchors below the chip one pass later
        // (the same quirk the `+` popup tests document).
        let _ = ctx.run_ui(screen(0.024, vec![]), |ui| {
            show(ui, Surface::Editor, Some("connected"), Enable::ALL);
        });
        let _ = ctx.run_ui(screen(0.028, vec![]), |ui| {
            show(ui, Surface::Editor, Some("connected"), Enable::ALL);
        });
        // The items render.
        assert!(
            ctx.read_response(egui::Id::new("topbar-save")).is_some(),
            "Save item renders"
        );
        assert!(
            ctx.read_response(egui::Id::new("topbar-upload")).is_some(),
            "Upload item renders"
        );
        assert!(
            ctx.read_response(egui::Id::new("topbar-paste")).is_some(),
            "Paste item renders"
        );
        assert!(
            ctx.read_response(egui::Id::new("topbar-settings"))
                .is_some(),
            "Settings entry renders"
        );
        // Click Save.
        let save = ctx
            .read_response(egui::Id::new("topbar-save"))
            .expect("save item")
            .rect;
        let fired = std::cell::Cell::new(None);
        let _ = ctx.run_ui(screen(0.032, click_events(save.center())), |ui| {
            let f = show(ui, Surface::Editor, Some("connected"), Enable::ALL);
            fired.set(f);
        });
        assert_eq!(fired.get(), Some(TopbarAction::Save));
    }

    /// The home overflow carries only the about/note + Settings —
    /// no editor items.
    #[test]
    fn home_overflow_omits_editor_items() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let _ = ctx.run_ui(screen(0.0, vec![]), |ui| {
            show(ui, Surface::Home, Some("connected"), Enable::NONE);
        });
        let ovf = ctx
            .read_response(egui::Id::new("topbar-overflow"))
            .expect("overflow")
            .rect;
        let _ = ctx.run_ui(screen(0.016, click_events(ovf.center())), |ui| {
            show(ui, Surface::Home, Some("connected"), Enable::NONE);
        });
        assert!(ctx.read_response(egui::Id::new("topbar-save")).is_none());
        assert!(ctx.read_response(egui::Id::new("topbar-paste")).is_none());
        assert!(
            ctx.read_response(egui::Id::new("topbar-settings"))
                .is_some(),
            "Settings is on every surface"
        );
    }

    /// The breadcrumb's current stage paints gold; on the editor it
    /// reads `Editor`.
    #[test]
    fn breadcrumb_current_stage_paints_gold() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let out = ctx.run_ui(screen(0.0, vec![]), |ui| {
            show(ui, Surface::Editor, None, Enable::ALL);
        });
        let gold_editor = out.shapes.iter().any(|cs| {
            matches!(
                &cs.shape,
                egui::Shape::Text(t)
                    if t.galley.job.text == "Editor"
                        && t.galley
                            .job
                            .sections
                            .first()
                            .is_some_and(|s| s.format.color == theme::palette::GOLD)
            )
        });
        assert!(gold_editor, "the current crumb stage paints gold");
    }
}
