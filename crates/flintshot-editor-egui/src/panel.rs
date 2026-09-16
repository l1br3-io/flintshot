//! The docked side panel — the editor's control home.
//!
//! Right-docked, grain-textured cream paper. Top → bottom: the
//! `+` capture chip (new capture / open / paste popup), the five
//! tool pods as white cutout cards with the spring hover-lift
//! (shadow grows, card translates up), the contextual bay for the
//! selected tool (temporary composition — the contextual section
//! redesign lands with its own task), and the history + output
//! actions pinned at the bottom. The canvas beside it is never
//! grained or shadowed.
//!
//! Input gating lives in `crate::app`: a press only starts a
//! canvas gesture when it lands inside the cached `CentralPanel`
//! rect, which egui shrinks past this panel automatically — the
//! panel's chips can never feed annotation drags.

use flintshot_core::{EditorState, NewCaptureMode, Tool};

use crate::theme;
use crate::toolbar::{self, ToolGate, ToolbarAction};

/// Docked panel width, logical px. Wide enough for the widest bay
/// row (the swatch row) plus card dressing; leaves ≥ ~700 px of
/// canvas at the 1024×640 editor page default.
pub(crate) const PANEL_WIDTH: f32 = 260.0;

/// How far a lifted card translates up, logical px.
const LIFT_PX: f32 = 4.0;

/// Hover-lift ramp duration, seconds.
const HOVER_LIFT_DURATION: f32 = 0.18;

/// Grain seed — any fixed value; determinism is the contract.
const GRAIN_SEED: u64 = 0x5EED_C0DE;

/// Grain tile edge, texels. 64² repeats unnoticeably at panel
/// scale.
const GRAIN_TILE: u32 = 64;

/// Panel state that persists across frames: per-pod hover-lift
/// progress and the (lazily created) grain texture handle.
#[derive(Default)]
pub(crate) struct PanelState {
    /// Hover-lift progress per tool pod (index into
    /// [`toolbar::TOOL_PODS`]), 0 = resting.
    pod_lift: Vec<f32>,
    /// The grain tile, created on first use.
    grain: Option<egui::TextureHandle>,
}

/// Draw the whole side panel. Returns the fired action plus the
/// `+` popup's fired page action, mirroring the retired rail's
/// contract (see `crate::toolbar`).
pub(crate) fn show(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    copied: bool,
    gate: ToolGate,
    page_actions: bool,
    panel: &mut PanelState,
) -> (ToolbarAction, Option<toolbar::PageChromeAction>) {
    let dt = ui.ctx().input(|i| i.unstable_dt);
    if panel.pod_lift.len() != toolbar::TOOL_PODS.len() {
        panel.pod_lift = vec![0.0; toolbar::TOOL_PODS.len()];
    }

    let frame = egui::Frame::new()
        .fill(theme::palette::PAPER_CREAM)
        .inner_margin(egui::Margin::same(8));
    egui::Panel::right("editor_panel")
        .resizable(false)
        .exact_size(PANEL_WIDTH)
        .frame(frame)
        .show(ui, |ui| {
            paint_grain(ui, panel);
            let (mut action, page_action) = new_capture_chip(ui, page_actions);
            ui.add_space(theme::POD_GAP);
            tool_pods(ui, state, gate, panel, dt);
            bay(ui, state);
            let bottom = bottom_actions(ui, copied, state.selected_index.is_some());
            if action == ToolbarAction::None {
                action = bottom;
            }
            (action, page_action)
        })
        .inner
}

/// Stamp the grain tile over the panel body — first widget call so
/// it lands under every control. The canvas panel is never
/// touched: this painter belongs to the side panel's `Ui`.
fn paint_grain(ui: &mut egui::Ui, panel: &mut PanelState) {
    if panel.grain.is_none() {
        let image = egui::ColorImage::new(
            [GRAIN_TILE as usize, GRAIN_TILE as usize],
            theme::grain::tile(GRAIN_SEED, GRAIN_TILE),
        );
        let options = egui::TextureOptions {
            wrap_mode: egui::TextureWrapMode::Repeat,
            ..Default::default()
        };
        panel.grain = Some(ui.ctx().load_texture("panel-grain", image, options));
    }
    if let Some(texture) = panel.grain.as_ref() {
        theme::grain::paint_grain(ui.painter(), ui.max_rect(), texture);
    }
}

/// The five tool pods as lift cards. Clicks switch tools through
/// core `set_tool` (leaving Move drops the persistent selection);
/// the gate greys locked chips without hiding them.
fn tool_pods(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    gate: ToolGate,
    panel: &mut PanelState,
    dt: f32,
) {
    for (i, pod) in toolbar::TOOL_PODS.iter().enumerate() {
        let resp = lift_frame(panel.pod_lift[i]).show(ui, |ui| {
            ui.horizontal(|ui| {
                for spec in pod.tools {
                    let enabled = gate.contains(spec.tool);
                    let selected = state.tool == spec.tool;
                    let tooltip = if enabled {
                        format!("{} · {}", spec.name, spec.key)
                    } else {
                        format!("{} · {} · {}", spec.name, spec.key, toolbar::LOCK_CLAUSE)
                    };
                    // Move renders labeled (pod emphasis); ids stay
                    // key-derived so tooltips and clicks are
                    // unchanged from the rail era.
                    let resp = if spec.tool == Tool::Move {
                        toolbar::labeled_chip(
                            ui,
                            egui::Id::new(("tool", spec.key)),
                            &tooltip,
                            selected,
                            spec.name,
                            spec.glyph,
                            enabled,
                        )
                    } else {
                        toolbar::chip(
                            ui,
                            egui::Id::new(("tool", spec.key)),
                            &tooltip,
                            selected,
                            spec.glyph,
                            enabled,
                        )
                    };
                    if resp.clicked() {
                        state.set_tool(spec.tool);
                    }
                }
            })
        });
        let target = f32::from(u8::from(resp.response.hovered()));
        panel.pod_lift[i] =
            theme::ease::approach(panel.pod_lift[i], target, dt, HOVER_LIFT_DURATION);
        ui.add_space(theme::POD_GAP);
    }
}

/// A cutout card at hover-lift progress `t`: the spring eases both
/// the shadow growth (resting → lifted) and an up-translation —
/// the outer margin shifts from "top padding" to "bottom padding"
/// so the card rises [`LIFT_PX`] inside its reserved space without
/// moving the layout below.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the margin is LIFT_PX-e by construction, 0..=4"
)]
fn lift_frame(t: f32) -> egui::Frame {
    let eased = theme::ease::spring(t).clamp(0.0, 1.0);
    let lift = eased * LIFT_PX;
    egui::Frame::new()
        .fill(theme::palette::CUTOUT_WHITE)
        .corner_radius(egui::CornerRadius::same(theme::POD_RADIUS))
        .inner_margin(egui::Margin::same(theme::POD_PAD))
        .stroke(theme::HAIRLINE_STROKE)
        .shadow(shadow_lerp(
            theme::SHADOW_RESTING,
            theme::SHADOW_LIFTED,
            eased,
        ))
        .outer_margin(egui::Margin {
            left: 0,
            right: 0,
            top: (LIFT_PX - lift) as i8,
            bottom: lift as i8,
        })
}

/// Linear shadow blend between the two token levels.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "shadow offsets/spreads blend between token values well inside i8 range"
)]
fn shadow_lerp(a: egui::Shadow, b: egui::Shadow, t: f32) -> egui::Shadow {
    let mix = |x: f32, y: f32| x + (y - x) * t;
    egui::Shadow {
        offset: [
            mix(f32::from(a.offset[0]), f32::from(b.offset[0])) as i8,
            mix(f32::from(a.offset[1]), f32::from(b.offset[1])) as i8,
        ],
        blur: mix(f32::from(a.blur), f32::from(b.blur)) as u8,
        spread: mix(f32::from(a.spread), f32::from(b.spread)) as u8,
        color: a.color.lerp_to_gamma(b.color, t),
    }
}

/// The contextual bay: only the selected tool's controls, in
/// panel-width rows, plus the per-tool hint. Temporary composition
/// (the contextual section redesign lands with its own task).
fn bay(ui: &mut egui::Ui, state: &mut EditorState) {
    ui.separator();
    match state.tool {
        Tool::Select => toolbar::select_bay(ui, state),
        // Move has no adjustable properties — nothing to grab,
        // nothing to drag — so its bay is just the hint.
        Tool::Move => {}
        Tool::Text => {
            ui.horizontal(|ui| toolbar::swatch_row(ui, &mut state.text_color));
            ui.horizontal(|ui| toolbar::text_style_row(ui, state));
        }
        Tool::Blur => toolbar::blur_bay(ui, state),
        Tool::Number => {
            ui.horizontal(|ui| toolbar::swatch_row(ui, &mut state.color));
        }
        // Future `Tool` variants (the enum is `#[non_exhaustive]`)
        // and the six drawing tools share the generic controls.
        _ => {
            ui.horizontal(|ui| toolbar::swatch_row(ui, &mut state.color));
            ui.horizontal(|ui| toolbar::stroke_row(ui, state));
        }
    }
    toolbar::hint(ui, toolbar::tool_hint(state.tool));
}

/// The bottom cluster: history (undo / redo / delete) above the
/// outputs (Save / Upload / the forest Copy primary). Restyled as
/// the export cluster with its own task; chips relocated as-is.
fn bottom_actions(ui: &mut egui::Ui, copied: bool, delete_enabled: bool) -> ToolbarAction {
    let mut action = ToolbarAction::None;
    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
        toolbar::pod_island(ui, |ui| {
            if toolbar::chip(
                ui,
                egui::Id::new("save"),
                "Save · Ctrl+S",
                false,
                toolbar::g_save,
                true,
            )
            .clicked()
            {
                action = ToolbarAction::Save;
            }
            if toolbar::chip(
                ui,
                egui::Id::new("upload"),
                "Upload",
                false,
                toolbar::g_upload,
                true,
            )
            .clicked()
            {
                action = ToolbarAction::Upload;
            }
            if toolbar::copy_chip(ui, copied).clicked() {
                action = ToolbarAction::Copy;
            }
        });
        ui.add_space(theme::POD_GAP);
        // History pod: Delete alone — Undo / Redo live on the
        // floating on-canvas cluster now (`crate::floating`).
        toolbar::pod_island(ui, |ui| {
            // Enabled iff an annotation is selected (persistent
            // selection); disabled reads greyed / hover-only via the
            // same `enabled` path the locked tool chips use.
            if toolbar::chip(
                ui,
                egui::Id::new("delete"),
                "Delete · Del",
                false,
                toolbar::g_trash,
                delete_enabled,
            )
            .clicked()
            {
                action = ToolbarAction::Delete;
            }
        });
    });
    action
}

/// The Win11-style "new snip" affordance leading the panel. A
/// popup (not an immediate action) so the home window's three
/// actions — Capture / Open… / Paste — hang off one chip (home
/// parity); Ctrl+N is the keyboard fast path. In a page epoch the
/// popup entries fire page actions; the worker window, with no
/// bridge to route Open/Paste, offers Capture alone through the
/// classic `NewCapture` exit.
fn new_capture_chip(
    ui: &mut egui::Ui,
    page_actions: bool,
) -> (ToolbarAction, Option<toolbar::PageChromeAction>) {
    let mut action = ToolbarAction::None;
    let mut page_action = None;
    let new_chip = toolbar::chip(
        ui,
        egui::Id::new("new-capture"),
        "New capture · Ctrl+N",
        false,
        toolbar::g_plus,
        true,
    );
    let popup_id = new_chip.id.with("popup");
    egui::Popup::from_toggle_button_response(&new_chip)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(160.0);
            ui.vertical(|ui| {
                let mut fired = false;
                if page_actions {
                    if popup_button(ui, egui::Id::new("plus-capture"), "Capture  [C]", None) {
                        page_action = Some(toolbar::PageChromeAction::Capture);
                        fired = true;
                    }
                    if popup_button(
                        ui,
                        egui::Id::new("plus-open"),
                        "Open…  [O]",
                        Some(toolbar::REPLACE_HINT),
                    ) {
                        page_action = Some(toolbar::PageChromeAction::Open);
                        fired = true;
                    }
                    if popup_button(
                        ui,
                        egui::Id::new("plus-paste"),
                        "Paste",
                        Some(toolbar::REPLACE_HINT),
                    ) {
                        page_action = Some(toolbar::PageChromeAction::Paste);
                        fired = true;
                    }
                } else if popup_button(ui, egui::Id::new("plus-capture"), "Capture · Ctrl+N", None)
                {
                    action = ToolbarAction::NewCapture(NewCaptureMode::Region);
                    fired = true;
                }
                if fired {
                    // A capture closes the editor (the popup dies with
                    // it), but page-mode Open/Paste keep the window —
                    // the popup must not linger open over them.
                    egui::Popup::close_id(ui.ctx(), popup_id);
                }
            });
        });
    (action, page_action)
}

/// One text row in the `+` popup: an explicit-id button (the
/// two-pass click-test pattern reads these ids back), quiet at
/// rest and cream-tinted on hover — a menu row, not a chip.
/// Fixed-width rows keep the popup at its `set_min_width`.
fn popup_button(ui: &mut egui::Ui, id: egui::Id, label: &str, hint: Option<&str>) -> bool {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(160.0, 20.0), egui::Sense::hover());
    let mut resp = ui.interact(rect, id, egui::Sense::click());
    if let Some(hint) = hint {
        resp = resp.on_hover_text(hint);
    }
    let fill = if resp.hovered() {
        theme::palette::HOLIDAY_FAINT
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(4),
        fill,
        egui::Stroke::NONE,
        egui::StrokeKind::Middle,
    );
    ui.painter().text(
        rect.left_center() + egui::vec2(6.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(theme::TYPE_BODY),
        theme::palette::INK,
    );
    resp.clicked()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolbar::{PageChromeAction, ToolGate, TOOL_PODS};
    use flintshot_core::{Annotation, Color, Rect as CoreRect, Stroke as CoreStroke, Tool};

    fn panel_state() -> EditorState {
        let img = image::RgbaImage::from_raw(64, 48, vec![255; 64 * 48 * 4])
            .expect("raw buffer matches 64*48*4");
        EditorState::new(img)
    }

    fn screen(w: f32, h: f32) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(w, h),
            )),
            ..egui::RawInput::default()
        }
    }

    fn draw(
        ctx: &egui::Context,
        state: &mut EditorState,
        panel: &mut PanelState,
        gate: ToolGate,
        page_actions: bool,
        events: Vec<egui::Event>,
    ) -> (ToolbarAction, Option<PageChromeAction>) {
        let raw = egui::RawInput {
            events,
            ..screen(1024.0, 768.0)
        };
        let mut fired = (ToolbarAction::None, None);
        let _ = ctx.run_ui(raw, |ui| {
            fired = show(ui, state, false, gate, page_actions, panel);
        });
        fired
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

    // --- Tool switching + gate (migrated from the rail era) ---

    fn tool_after_pen_click(gate: ToolGate) -> Tool {
        let mut state = panel_state();
        state.tool = Tool::Select;
        let ctx = egui::Context::default();
        let mut panel = PanelState::default();
        let _ = draw(&ctx, &mut state, &mut panel, gate, false, vec![]);
        let pen = ctx
            .read_response(egui::Id::new(("tool", "P")))
            .expect("pass 1 registered the Pencil chip");
        let _ = draw(
            &ctx,
            &mut state,
            &mut panel,
            gate,
            false,
            click_events(pen.rect.center()),
        );
        state.tool
    }

    #[test]
    fn select_only_click_cannot_switch_tool() {
        assert_eq!(
            tool_after_pen_click(ToolGate::SelectOnly),
            Tool::Select,
            "a gated-off chip must render hover-only and stay inert"
        );
    }

    #[test]
    fn all_gate_click_switches_tool() {
        assert_eq!(
            tool_after_pen_click(ToolGate::All),
            Tool::Pencil,
            "control: the same click switches under All"
        );
    }

    // --- Delete chip (migrated from the rail era) ---

    fn selected_state() -> EditorState {
        let mut state = panel_state();
        state.pending = Some(Annotation::Rect {
            rect: CoreRect::from_xywh(10.0, 10.0, 40.0, 30.0),
            stroke: CoreStroke::new(Color::RED, 3.0),
            fill: None,
        });
        state.commit_annotation();
        state.selected_index = Some(0);
        state
    }

    fn action_after_delete_click(state: &mut EditorState) -> ToolbarAction {
        let ctx = egui::Context::default();
        let mut panel = PanelState::default();
        let _ = draw(&ctx, state, &mut panel, ToolGate::All, false, vec![]);
        let del = ctx
            .read_response(egui::Id::new("delete"))
            .expect("pass 1 registered the Delete chip");
        let at = del.rect.center();
        draw(
            &ctx,
            state,
            &mut panel,
            ToolGate::All,
            false,
            click_events(at),
        )
        .0
    }

    #[test]
    fn delete_chip_click_emits_delete_with_selection() {
        let mut state = selected_state();
        assert_eq!(
            action_after_delete_click(&mut state),
            ToolbarAction::Delete,
            "a selected annotation makes the chip live and clicking it emits Delete"
        );
    }

    #[test]
    fn delete_chip_click_inert_without_selection() {
        let mut state = panel_state();
        assert_eq!(
            action_after_delete_click(&mut state),
            ToolbarAction::None,
            "no selection: the chip must render greyed / hover-only and stay inert"
        );
    }

    // --- `+` popup home parity (migrated from the rail era) ---

    fn open_plus_popup(page_actions: bool) -> egui::Context {
        let mut state = panel_state();
        let ctx = egui::Context::default();
        let mut panel = PanelState::default();
        let _ = draw(
            &ctx,
            &mut state,
            &mut panel,
            ToolGate::All,
            page_actions,
            vec![],
        );
        let chip = ctx
            .read_response(egui::Id::new("new-capture"))
            .expect("pass 1 registered the + chip");
        draw(
            &ctx,
            &mut state,
            &mut panel,
            ToolGate::All,
            page_actions,
            click_events(chip.rect.center()),
        );
        ctx
    }

    fn plus_popup_action_after_click_on(
        id: &str,
        page_actions: bool,
    ) -> (ToolbarAction, Option<PageChromeAction>) {
        let ctx = open_plus_popup(page_actions);
        let mut state = panel_state();
        let mut panel = PanelState::default();
        // Two settle passes: the popup anchors at the screen centre
        // on its open pass and re-anchors below the chip one pass
        // later — reading entry rects before the re-anchor would
        // click where the popup no longer is.
        for _ in 0..2 {
            let _ = draw(
                &ctx,
                &mut state,
                &mut panel,
                ToolGate::All,
                page_actions,
                vec![],
            );
        }
        let entry = ctx
            .read_response(egui::Id::new(id))
            .unwrap_or_else(|| panic!("the open popup registers {id}"));
        draw(
            &ctx,
            &mut state,
            &mut panel,
            ToolGate::All,
            page_actions,
            click_events(entry.rect.center()),
        )
    }

    #[test]
    fn plus_popup_page_mode_maps_capture_to_the_capture_exit() {
        assert_eq!(
            plus_popup_action_after_click_on("plus-capture", true),
            (ToolbarAction::None, Some(PageChromeAction::Capture)),
            "the popup's Capture routes the same action the home's twin does"
        );
    }

    #[test]
    fn plus_popup_page_mode_maps_open_to_the_page_bridge() {
        assert_eq!(
            plus_popup_action_after_click_on("plus-open", true),
            (ToolbarAction::None, Some(PageChromeAction::Open)),
            "the popup's Open routes the page bridge's open_image contract"
        );
    }

    #[test]
    fn plus_popup_page_mode_maps_paste_to_the_page_bridge() {
        assert_eq!(
            plus_popup_action_after_click_on("plus-paste", true),
            (ToolbarAction::None, Some(PageChromeAction::Paste)),
            "the popup's Paste routes the page bridge's request_paste contract"
        );
    }

    #[test]
    fn plus_popup_page_mode_offers_exactly_the_home_actions() {
        let ctx = open_plus_popup(true);
        for id in ["plus-capture", "plus-open", "plus-paste"] {
            assert!(
                ctx.read_response(egui::Id::new(id)).is_some(),
                "the popup must offer {id}"
            );
        }
        assert!(
            ctx.read_response(egui::Id::new("plus-fullscreen"))
                .is_none(),
            "the popup must not offer capture-kind variants"
        );
    }

    #[test]
    fn plus_popup_worker_mode_offers_capture_only() {
        let ctx = open_plus_popup(false);
        assert!(
            ctx.read_response(egui::Id::new("plus-capture")).is_some(),
            "worker windows still offer Capture"
        );
        for id in ["plus-open", "plus-paste"] {
            assert!(
                ctx.read_response(egui::Id::new(id)).is_none(),
                "worker windows have no bridge — {id} must not render"
            );
        }
    }

    #[test]
    fn plus_popup_worker_mode_capture_is_the_new_capture_exit() {
        assert_eq!(
            plus_popup_action_after_click_on("plus-capture", false),
            (ToolbarAction::NewCapture(NewCaptureMode::Region), None),
            "worker capture keeps the NewCapture exit (Region is the internal default)"
        );
    }

    /// The Pin affordance is gone from the panel (remove-pin-to-
    /// desktop): no chip may register under the retired id.
    #[test]
    fn panel_has_no_pin_chip() {
        let mut state = panel_state();
        let ctx = egui::Context::default();
        let mut panel = PanelState::default();
        let _ = draw(&ctx, &mut state, &mut panel, ToolGate::All, false, vec![]);
        assert!(
            ctx.read_response(egui::Id::new("pin")).is_none(),
            "the pin chip must not register a response"
        );
    }

    // --- Panel geometry (the structural pivot's own contracts) ---

    /// Collect the white cutout rect shapes a frame painted
    /// (frames with shadows wrap in `Shape::Vec`, so recurse).
    fn island_rects(output: &egui::FullOutput) -> Vec<egui::Rect> {
        fn collect<'a>(shapes: impl Iterator<Item = &'a egui::Shape>, out: &mut Vec<egui::Rect>) {
            for shape in shapes {
                match shape {
                    egui::Shape::Rect(r)
                        if r.fill == theme::palette::CUTOUT_WHITE
                            && r.corner_radius == egui::CornerRadius::same(theme::POD_RADIUS) =>
                    {
                        out.push(r.rect);
                    }
                    egui::Shape::Vec(v) => collect(v.iter(), out),
                    _ => {}
                }
            }
        }
        let mut rects = Vec::new();
        collect(output.shapes.iter().map(|cs| &cs.shape), &mut rects);
        rects
    }

    /// Every pod renders as its own white cutout island, chips
    /// within a pod share a row, and the pods stack strictly
    /// top-to-bottom — the vertical rhythm that replaced the rail.
    #[test]
    fn panel_pods_stack_as_separate_islands() {
        let mut state = panel_state();
        let ctx = egui::Context::default();
        let mut panel = PanelState::default();
        let output = {
            let raw = screen(1024.0, 768.0);
            ctx.run_ui(raw, |ui| {
                let _ = show(ui, &mut state, false, ToolGate::All, false, &mut panel);
            })
        };
        // five tool pods + history + outputs.
        assert_eq!(
            island_rects(&output).len(),
            TOOL_PODS.len() + 2,
            "every pod renders as its own white cutout island"
        );
        // Tool chips inside one pod share a row; the first chips of
        // consecutive pods stack strictly downward.
        let first_chip = |pod: usize| {
            let spec = TOOL_PODS[pod].tools.first().expect("non-empty pod");
            ctx.read_response(egui::Id::new(("tool", spec.key)))
                .unwrap_or_else(|| panic!("pod {pod} registered its first chip"))
                .rect
        };
        for pod in TOOL_PODS.windows(2) {
            let a = ctx
                .read_response(egui::Id::new((
                    "tool",
                    pod[0].tools.first().expect("non-empty pod").key,
                )))
                .expect("chip rect")
                .rect;
            let b = ctx
                .read_response(egui::Id::new((
                    "tool",
                    pod[1].tools.first().expect("non-empty pod").key,
                )))
                .expect("chip rect")
                .rect;
            assert!(
                a.max.y <= b.min.y,
                "pods must stack top-to-bottom: {a:?} vs {b:?}"
            );
        }
        let _ = first_chip(0);
        // Chips within the shapes pod share a row.
        let shapes = TOOL_PODS[1].tools;
        let a = ctx
            .read_response(egui::Id::new(("tool", shapes[0].key)))
            .expect("rect chip")
            .rect;
        let b = ctx
            .read_response(egui::Id::new(("tool", shapes[1].key)))
            .expect("ellipse chip")
            .rect;
        assert!(
            (a.center().y - b.center().y).abs() < 0.01,
            "intra-pod chips share a row"
        );
    }

    /// The 1024×640 editor page default must leave ≥ ~700 px of
    /// canvas beside the panel (spec: the panel never starves the
    /// screenshot).
    #[test]
    fn panel_leaves_canvas_at_least_700px_at_the_editor_default() {
        let mut state = panel_state();
        let ctx = egui::Context::default();
        let mut panel = PanelState::default();
        let raw = screen(1024.0, 640.0);
        let _ = ctx.run_ui(raw, |ui| {
            let _ = show(ui, &mut state, false, ToolGate::All, false, &mut panel);
            let central = egui::CentralPanel::default().show(ui, |ui| {
                ui.allocate_space(egui::vec2(1.0, 1.0));
            });
            assert!(
                central.response.rect.width() >= 700.0,
                "canvas starved: {:?} wide at the 1024 default",
                central.response.rect.width()
            );
        });
    }

    /// Hovering a pod drives its lift progress toward 1; leaving it
    /// decays back toward 0 — the spring microinteraction is wired.
    #[test]
    fn hover_lift_progress_rises_and_falls() {
        let mut state = panel_state();
        let ctx = egui::Context::default();
        let mut panel = PanelState::default();
        let _ = draw(&ctx, &mut state, &mut panel, ToolGate::All, false, vec![]);
        let move_chip = ctx
            .read_response(egui::Id::new(("tool", "V")))
            .expect("the selection pod's leading chip");
        let at = move_chip.rect.center();
        let hover = egui::Event::PointerMoved(at);
        for _ in 0..30 {
            let _ = draw(
                &ctx,
                &mut state,
                &mut panel,
                ToolGate::All,
                false,
                vec![hover.clone()],
            );
        }
        assert!(
            panel.pod_lift[0] > 0.9,
            "a held hover must ramp the lift to ~1, got {}",
            panel.pod_lift[0]
        );
        for _ in 0..30 {
            let _ = draw(
                &ctx,
                &mut state,
                &mut panel,
                ToolGate::All,
                false,
                vec![egui::Event::PointerMoved(egui::pos2(5.0, 700.0))],
            );
        }
        assert!(
            panel.pod_lift[0] < 0.1,
            "leaving the pod must decay the lift toward 0, got {}",
            panel.pod_lift[0]
        );
    }
}
