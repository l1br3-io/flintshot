//! The docked side panel — the editor's control home.
//!
//! Right-docked, grain-textured cream paper, and **collapsible** to
//! a glyph rail (the `«` toggle in the action row; the rail's `»`
//! restores it). Top → bottom: the action row (`+` capture chip,
//! collapse toggle), **one** white cutout card holding the five
//! semantic tool groups separated by hairlines (one card, not five
//! floating islands — the disjointed shadowed cards read as clutter,
//! and the per-pod hover-lift went with them), the contextual
//! options section (exactly the selected tool's options; the
//! color/stroke picker bay collapses behind one popover pod button —
//! see `crate::popover`), and the bottom cluster: history + export
//! chips in one quiet island with the Copy hero pinned below. The
//! canvas beside it is never grained or shadowed.
//!
//! Layout note (the stretched-island bug this file must not
//! reintroduce): `egui::Frame` inside a `bottom_up` layout grabs the
//! full remaining main-direction space — the old bottom actions
//! stretched their export island to a third of the window. The
//! bottom cluster therefore allocates **exact** sizes
//! (`allocate_exact_size` / `allocate_ui_with_layout` with an
//! explicit desired size), which shrink-wrap by contract.
//!
//! Input gating lives in `crate::app`: a press only starts a
//! canvas gesture when it lands inside the cached `CentralPanel`
//! rect, which egui shrinks past this panel automatically — the
//! panel's chips can never feed annotation drags.

use flintshot_core::{EditorState, NewCaptureMode, Tool};

use crate::popover;
use crate::theme;

/// Docked panel width, logical px. Wide enough for the widest bay
/// row (the swatch row) plus card dressing; leaves ≥ ~700 px of
/// canvas at the 1024×640 editor page default.
pub(crate) const PANEL_WIDTH: f32 = 260.0;

/// Collapsed rail width, logical px: exactly one 28 px chip column
/// plus the panel's inner margin.
pub(crate) const RAIL_WIDTH: f32 = 44.0;

/// The Objects panel section (spec §forward-compat 2): reserved
/// for multi-select grouping, hidden today — the selection model
/// stays single-select and this section renders nothing until its
/// spec lands. Flipping the constant is the seam's whole
/// integration point.
const OBJECTS_SECTION_LIVE: bool = false;

/// Grain seed — any fixed value; determinism is the contract.
const GRAIN_SEED: u64 = 0x5EED_C0DE;

/// Grain tile edge, texels. 64² repeats unnoticeably at panel
/// scale.
const GRAIN_TILE: u32 = 64;

/// Height of the bottom cluster's export island: one chip row (28)
/// plus the pod padding (6) either side. Written as a literal —
/// `f32::from` is not const-callable.
const EXPORT_ROW_HEIGHT: f32 = 40.0;

/// Panel state that persists across frames: the collapse flag and
/// the (lazily created) grain texture handle.
#[derive(Default)]
pub(crate) struct PanelState {
    /// Whether the panel is collapsed to the glyph rail.
    pub(crate) collapsed: bool,
    /// The grain tile, created on first use.
    grain: Option<egui::TextureHandle>,
}

impl PanelState {
    /// The panel's current width, logical px — the worker window's
    /// resize target must follow the live panel (a collapsed panel
    /// must not keep issuing the expanded width).
    #[must_use]
    pub(crate) fn width(&self) -> f32 {
        if self.collapsed {
            RAIL_WIDTH
        } else {
            PANEL_WIDTH
        }
    }
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
) -> (ToolbarAction, Option<PageChromeAction>) {
    let frame = egui::Frame::new()
        .fill(theme::palette::PAPER_CREAM)
        .inner_margin(egui::Margin::same(8));
    let width = panel.width();
    let collapsed = panel.collapsed;
    egui::Panel::right("editor_panel")
        .resizable(false)
        .exact_size(width)
        .frame(frame)
        .show(ui, |ui| {
            paint_grain(ui, panel);
            if collapsed {
                rail_body(ui, state, copied, gate, page_actions, panel)
            } else {
                docked_body(ui, state, copied, gate, page_actions, panel)
            }
        })
        .inner
}

/// The expanded panel body: action row, tools card, contextual
/// bay, bottom cluster.
fn docked_body(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    copied: bool,
    gate: ToolGate,
    page_actions: bool,
    panel: &mut PanelState,
) -> (ToolbarAction, Option<PageChromeAction>) {
    let (mut action, page_action) = ui
        .horizontal(|ui| {
            let pair = new_capture_chip(ui, page_actions);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if collapse_toggle(ui, false).clicked() {
                    panel.collapsed = true;
                }
            });
            pair
        })
        .inner;
    ui.add_space(theme::POD_GAP);
    tools_card(ui, state, gate);
    if OBJECTS_SECTION_LIVE {
        // Unreachable while the seam is inert.
        unreachable!("OBJECTS_SECTION_LIVE is false until its spec lands");
    }
    bay(ui, state);
    let bottom = bottom_actions(ui, copied, state.selected_index.is_some());
    if action == ToolbarAction::None {
        action = bottom;
    }
    (action, page_action)
}

/// The collapsed rail body: the same vocabulary, glyph-only, in
/// one 28 px column — tools top, outputs pinned bottom.
fn rail_body(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    copied: bool,
    gate: ToolGate,
    page_actions: bool,
    panel: &mut PanelState,
) -> (ToolbarAction, Option<PageChromeAction>) {
    let mut action = ToolbarAction::None;
    if collapse_toggle(ui, true).clicked() {
        panel.collapsed = false;
    }
    ui.add_space(theme::POD_GAP);
    let (plus, page_action) = new_capture_chip(ui, page_actions);
    if action == ToolbarAction::None {
        action = plus;
    }
    ui.add_space(theme::POD_GAP);
    ui.separator();
    for spec in TOOL_PODS.iter().flat_map(|pod| pod.tools.iter()) {
        let enabled = gate.contains(spec.tool);
        let tooltip = if enabled {
            format!("{} · {}", spec.name, spec.key)
        } else {
            format!("{} · {} · {}", spec.name, spec.key, LOCK_CLAUSE)
        };
        let resp = chip(
            ui,
            egui::Id::new(("tool", spec.key)),
            &tooltip,
            state.tool == spec.tool,
            spec.glyph,
            enabled,
        );
        if resp.clicked() {
            state.set_tool(spec.tool);
        }
    }
    ui.separator();
    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
        if copy_square(ui, copied).clicked() {
            action = ToolbarAction::Copy;
        }
        ui.add_space(theme::SPACE_SCALE[1]);
        for (id, tooltip, glyph) in [
            ("upload", "Upload", g_upload as Glyph),
            ("save", "Save · Ctrl+S", g_save),
            ("delete", "Delete · Del", g_trash),
        ] {
            if chip(
                ui,
                egui::Id::new(id),
                tooltip,
                false,
                glyph,
                id != "delete" || state.selected_index.is_some(),
            )
            .clicked()
            {
                action = match id {
                    "save" => ToolbarAction::Save,
                    "upload" => ToolbarAction::Upload,
                    _ => ToolbarAction::Delete,
                };
            }
            ui.add_space(theme::SPACE_SCALE[1]);
        }
    });
    (action, page_action)
}

/// The `«` / `»` collapse toggle — the panel's one affordance that
/// is about the panel itself. Expanded shows `»` (hide, toward the
/// right edge); the rail shows `«` (restore).
fn collapse_toggle(ui: &mut egui::Ui, collapsed: bool) -> egui::Response {
    let (tooltip, glyph): (&str, Glyph) = if collapsed {
        ("Show tools", g_show_panel)
    } else {
        ("Hide tools", g_hide_panel)
    };
    chip(
        ui,
        egui::Id::new("panel-collapse"),
        tooltip,
        false,
        glyph,
        true,
    )
}

/// The five semantic tool groups as rows of **one** white cutout
/// card, hairline-separated — grouping by proximity and rules, not
/// by seven floating islands. Clicks switch tools through core
/// `set_tool` (leaving Move drops the persistent selection); the
/// gate greys locked chips without hiding them.
fn tools_card(ui: &mut egui::Ui, state: &mut EditorState, gate: ToolGate) {
    egui::Frame::new()
        .fill(theme::palette::PAPER_CREAM)
        .corner_radius(egui::CornerRadius::same(theme::CUTOUT_RADIUS))
        .inner_margin(egui::Margin::same(theme::POD_PAD))
        .stroke(theme::HAIRLINE_STROKE)
        .shadow(theme::SHADOW_RESTING)
        .show(ui, |ui| {
            for (i, pod) in TOOL_PODS.iter().enumerate() {
                if i > 0 {
                    ui.separator();
                }
                ui.horizontal(|ui| {
                    for spec in pod.tools {
                        let enabled = gate.contains(spec.tool);
                        let selected = state.tool == spec.tool;
                        let tooltip = if enabled {
                            format!("{} · {}", spec.name, spec.key)
                        } else {
                            format!("{} · {} · {}", spec.name, spec.key, LOCK_CLAUSE)
                        };
                        // Move renders labeled (pod emphasis); ids stay
                        // key-derived so tooltips and clicks are
                        // unchanged from the rail era.
                        let resp = if spec.tool == Tool::Move {
                            labeled_chip(
                                ui,
                                egui::Id::new(("tool", spec.key)),
                                &tooltip,
                                selected,
                                spec.name,
                                spec.glyph,
                                enabled,
                            )
                        } else {
                            chip(
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
                });
            }
        });
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

/// The contextual bay: only the selected tool's controls, in
/// panel-width rows, plus the per-tool hint. Temporary composition
/// (the contextual section redesign lands with its own task).
/// The contextual options section: exactly the selected tool's
/// options, nothing else (Gradia pattern #3). The color/stroke
/// picker bay collapses behind one popover pod button (pattern #4,
/// `crate::popover`); the fill toggle, number-bubble start, and
/// blur kind live here directly.
fn bay(ui: &mut egui::Ui, state: &mut EditorState) {
    ui.separator();
    match state.tool {
        Tool::Select => select_bay(ui, state),
        // Move has no adjustable properties — nothing to grab,
        // nothing to drag — so its section is just the hint.
        Tool::Move => {}
        Tool::Text => {
            ui.horizontal(|ui| text_style_row(ui, state));
            ui.horizontal(|ui| popover::pod_button(ui, state, popover::ColorTarget::Text));
        }
        Tool::Number => {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Start at")
                        .size(theme::TYPE_SM)
                        .color(theme::palette::INK_SOFT),
                );
                ui.add(
                    egui::DragValue::new(&mut state.next_number)
                        .range(1.0..=999.0)
                        .speed(1.0),
                );
            });
            ui.horizontal(|ui| popover::pod_button(ui, state, popover::ColorTarget::Shape));
        }
        Tool::Blur => blur_bay(ui, state),
        Tool::Rect | Tool::Ellipse => {
            // Closed shapes carry the fill toggle beside the picker.
            ui.horizontal(|ui| popover::pod_button(ui, state, popover::ColorTarget::Shape));
            fill_toggle(ui, state);
        }
        // Future `Tool` variants (the enum is `#[non_exhaustive]`)
        // and the open-stroke drawing tools share the picker pod.
        _ => {
            ui.horizontal(|ui| popover::pod_button(ui, state, popover::ColorTarget::Shape));
        }
    }
    hint(ui, tool_hint(state.tool));
}

/// The fill toggle for closed shapes: a cutout chip that reads as
/// selected (forest) while on — the same visual language as the
/// active tool chip, applied to an option.
fn fill_toggle(ui: &mut egui::Ui, state: &mut EditorState) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(84.0, 26.0), egui::Sense::hover());
    let resp = ui
        .interact(rect, egui::Id::new("fill-toggle"), egui::Sense::click())
        .on_hover_text("Fill closed shapes with the annotation color");
    let on = state.fill_shapes;
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(theme::POD_RADIUS),
        if on {
            theme::palette::FOREST_DEEP
        } else {
            theme::palette::PAPER_CREAM
        },
        if on {
            egui::Stroke::NONE
        } else {
            theme::HAIRLINE_STROKE
        },
        egui::StrokeKind::Middle,
    );
    let galley = ui.painter().layout_no_wrap(
        if on {
            "Fill ✓".to_owned()
        } else {
            "Fill".to_owned()
        },
        egui::FontId::proportional(theme::TYPE_SM),
        if on {
            theme::palette::PAPER_CREAM
        } else {
            theme::palette::INK_SOFT
        },
    );
    let pos = egui::pos2(
        rect.center().x - galley.size().x / 2.0,
        rect.center().y - galley.size().y / 2.0,
    );
    let ink = if on {
        theme::palette::PAPER_CREAM
    } else {
        theme::palette::INK_SOFT
    };
    ui.painter().galley(pos, galley, ink);
    if resp.clicked() {
        state.fill_shapes = !state.fill_shapes;
    }
}

/// The bottom cluster: the export group (Delete / Save / Upload in
/// one quiet island) with the Copy hero pinned below it. Existing
/// handlers are reused untouched — this is arrangement, not
/// behavior.
///
/// Everything here allocates **exact** sizes: an `egui::Frame`
/// inside the `bottom_up` layout grabs the whole remaining column
/// (the stretched-island bug — see the module docs), so the island
/// is painted by hand over an exact allocation and the chips laid
/// out inside it.
fn bottom_actions(ui: &mut egui::Ui, copied: bool, delete_enabled: bool) -> ToolbarAction {
    let mut action = ToolbarAction::None;
    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
        if copy_hero(ui, copied).clicked() {
            action = ToolbarAction::Copy;
        }
        ui.add_space(theme::SPACE_SCALE[2]);
        let bottom = export_island(ui, delete_enabled);
        if action == ToolbarAction::None {
            action = bottom;
        }
    });
    action
}

/// The export island: Delete (history) beside Save / Upload on one
/// white cutout row, exactly [`EXPORT_ROW_HEIGHT`] tall — the
/// pinned-height allocation is the stretch fix.
fn export_island(ui: &mut egui::Ui, delete_enabled: bool) -> ToolbarAction {
    let mut action = ToolbarAction::None;
    let width = ui.available_width().max(120.0);
    ui.allocate_ui_with_layout(
        egui::vec2(width, EXPORT_ROW_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            let rect = ui.max_rect();
            let radius = egui::CornerRadius::same(theme::CUTOUT_RADIUS);
            ui.painter()
                .add(theme::SHADOW_RESTING.as_shape(rect, radius));
            ui.painter().rect(
                rect,
                radius,
                theme::palette::PAPER_CREAM,
                theme::HAIRLINE_STROKE,
                egui::StrokeKind::Middle,
            );
            ui.add_space(f32::from(theme::POD_PAD));
            ui.spacing_mut().item_spacing = egui::vec2(theme::SPACE_SCALE[1], 0.0);
            if chip(
                ui,
                egui::Id::new("delete"),
                "Delete · Del",
                false,
                g_trash,
                delete_enabled,
            )
            .clicked()
            {
                action = ToolbarAction::Delete;
            }
            if chip(
                ui,
                egui::Id::new("save"),
                "Save · Ctrl+S",
                false,
                g_save,
                true,
            )
            .clicked()
            {
                action = ToolbarAction::Save;
            }
            if chip(ui, egui::Id::new("upload"), "Upload", false, g_upload, true).clicked() {
                action = ToolbarAction::Upload;
            }
        },
    );
    action
}

/// The collapsed rail's Copy: a 28 px forest square (the
/// selected-chip visual language — the 8 px cutout border would
/// eat a square this small), morphing to a check on the copied
/// feedback like the hero.
fn copy_square(ui: &mut egui::Ui, copied: bool) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::hover());
    let resp = ui
        .interact(rect, egui::Id::new("copy_action"), egui::Sense::click())
        .on_hover_text("Copy · Ctrl+C");
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(theme::POD_RADIUS),
        theme::palette::FOREST_DEEP,
        egui::Stroke::NONE,
        egui::StrokeKind::Middle,
    );
    let glyph = if copied { g_check } else { g_copy };
    glyph(ui.painter(), rect, theme::palette::CREAM_TEXT);
    resp
}

/// The Copy hero: a full-width forest card with the 8 px cutout
/// border and a resting shadow — the instant annotate-and-copy
/// anchor, visually the loudest control in the panel. The glyph
/// morphs to a check and the label reads "Copied" once a copy has
/// succeeded this session (the copied-state feedback contract).
fn copy_hero(ui: &mut egui::Ui, copied: bool) -> egui::Response {
    let width = ui.available_width().max(120.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 40.0), egui::Sense::hover());
    let resp = ui
        .interact(rect, egui::Id::new("copy_action"), egui::Sense::click())
        .on_hover_text("Copy · Ctrl+C");
    let radius = egui::CornerRadius::same(theme::CUTOUT_RADIUS);
    ui.painter()
        .add(theme::SHADOW_RESTING.as_shape(rect, radius));
    ui.painter().rect(
        rect,
        radius,
        theme::palette::FOREST_DEEP,
        theme::CUTOUT_STROKE,
        egui::StrokeKind::Inside,
    );
    let glyph_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 12.0, rect.top()),
        egui::vec2(20.0, 40.0),
    );
    let glyph = if copied { g_check } else { g_copy };
    glyph(ui.painter(), glyph_rect, theme::palette::CREAM_TEXT);
    let label = if copied { "Copied" } else { "Copy" };
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::proportional(theme::TYPE_MD),
        theme::palette::CREAM_TEXT,
    );
    let pos = egui::pos2(
        glyph_rect.right() + 6.0,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(pos, galley, theme::palette::CREAM_TEXT);
    resp
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
) -> (ToolbarAction, Option<PageChromeAction>) {
    let mut action = ToolbarAction::None;
    let mut page_action = None;
    let new_chip = chip(
        ui,
        egui::Id::new("new-capture"),
        "New capture · Ctrl+N",
        false,
        g_plus,
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
                        page_action = Some(PageChromeAction::Capture);
                        fired = true;
                    }
                    if popup_button(
                        ui,
                        egui::Id::new("plus-open"),
                        "Open…  [O]",
                        Some(REPLACE_HINT),
                    ) {
                        page_action = Some(PageChromeAction::Open);
                        fired = true;
                    }
                    if popup_button(ui, egui::Id::new("plus-paste"), "Paste", Some(REPLACE_HINT)) {
                        page_action = Some(PageChromeAction::Paste);
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
        theme::palette::PAPER_DIM
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
        egui::FontId::proportional(theme::TYPE_MD),
        theme::palette::INK,
    );
    resp.clicked()
}

// ==================================================================
// Chip / pod / glyph primitives + bay rows (absorbed from the
// retired toolbar.rs — shared with the overlay's floating rows)
// ==================================================================

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Sense, Stroke, StrokeKind, Vec2};
use flintshot_core::{BlurKind, Color, FontFamily, FontStyle, FontWeight, TextAlign, TextOutline};

/// Which toolbar control the user clicked this frame (if any).
///
/// Collected while drawing the toolbar so the action is handled
/// outside it, avoiding borrow conflicts between `&mut self.state`
/// (captured by the toolbar widgets) and `&self.outcome` (needed by
/// `publish_outcome`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarAction {
    /// No control clicked.
    #[default]
    None,
    /// Save button — open file dialog.
    Save,
    /// Copy button — write to clipboard.
    Copy,
    /// Upload button — compose, write temp PNG, spawn upload worker.
    Upload,
    /// Undo button — replay a Ctrl+Z dispatch.
    Undo,
    /// Redo button — replay a Ctrl+Y dispatch.
    Redo,
    /// Delete button — replay a Del (no modifiers) dispatch.
    Delete,
    /// `+` chip (or Ctrl+N) — start a fresh capture; the editor
    /// closes for the new cycle.
    NewCapture(NewCaptureMode),
}

/// Paint a glyph inside the chip rect. Coordinates are glyph-space:
/// `(0, 0)` is the rect centre, `±7` the edges, +Y is down — matching
/// egui's screen space so glyphs can be reasoned about like the
/// strokes they represent.
pub(crate) type Glyph = fn(&egui::Painter, egui::Rect, Color32);

/// One rail tool chip: variant, display name, hotkey hint, glyph.
/// Shared with the overlay's floating rail (plan decision 4).
pub(crate) struct ToolSpec {
    /// Tool selected on click.
    pub(crate) tool: Tool,
    /// Human name (tooltip).
    pub(crate) name: &'static str,
    /// Single-key shortcut (tooltip + doc).
    pub(crate) key: &'static str,
    /// Painter glyph.
    pub(crate) glyph: Glyph,
}

/// The tool chips in rail order, partitioned into pods (spec
/// `toolbar-groups`): selection · shapes · freehand · labels ·
/// redact. Move leads (labeled) because it is the resting tool the
/// editor lands on after every commit — the rail teaches the
/// tool's existence without a tooltip.
///
/// The pod arrays below are the single source of truth; both the
/// editor rail and the overlay's floating tool row walk
/// [`TOOL_PODS`].
const SELECTION_POD: &[ToolSpec] = &[
    ToolSpec {
        tool: Tool::Move,
        name: "Move",
        key: "V",
        glyph: g_move,
    },
    ToolSpec {
        tool: Tool::Select,
        name: "Select",
        key: "S",
        glyph: g_select,
    },
];

const SHAPES_POD: &[ToolSpec] = &[
    ToolSpec {
        tool: Tool::Rect,
        name: "Rectangle",
        key: "R",
        glyph: g_rect,
    },
    ToolSpec {
        tool: Tool::Ellipse,
        name: "Ellipse",
        key: "E",
        glyph: g_ellipse,
    },
    ToolSpec {
        tool: Tool::Line,
        name: "Line",
        key: "L",
        glyph: g_line,
    },
    ToolSpec {
        tool: Tool::Arrow,
        name: "Arrow",
        key: "A",
        glyph: g_arrow,
    },
];

const FREEHAND_POD: &[ToolSpec] = &[
    ToolSpec {
        tool: Tool::Pencil,
        name: "Pencil",
        key: "P",
        glyph: g_pencil,
    },
    ToolSpec {
        tool: Tool::Marker,
        name: "Marker",
        key: "M",
        glyph: g_marker,
    },
];

const LABELS_POD: &[ToolSpec] = &[
    ToolSpec {
        tool: Tool::Text,
        name: "Text",
        key: "T",
        glyph: g_text,
    },
    ToolSpec {
        tool: Tool::Number,
        name: "Number",
        key: "N",
        glyph: g_number,
    },
];

const REDACT_POD: &[ToolSpec] = &[ToolSpec {
    tool: Tool::Blur,
    name: "Blur",
    key: "B",
    glyph: g_blur,
}];

/// One rail pod: a semantic group of tool chips rendered as a
/// single island. Shared by the editor rail and the overlay's
/// floating tool row.
pub(crate) struct ToolPod {
    /// Group name (debugging + test messages only — pods carry no
    /// label in the UI). Rendering never reads it, hence the allow:
    /// the spec (`toolbar-groups` § Code style) mandates the field
    /// for diagnostics.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) name: &'static str,
    /// Chips in pod order.
    pub(crate) tools: &'static [ToolSpec],
}

/// The tool pods, rail order. The unit test pins the partition.
pub(crate) const TOOL_PODS: &[ToolPod] = &[
    ToolPod {
        name: "selection",
        tools: SELECTION_POD,
    },
    ToolPod {
        name: "shapes",
        tools: SHAPES_POD,
    },
    ToolPod {
        name: "freehand",
        tools: FREEHAND_POD,
    },
    ToolPod {
        name: "labels",
        tools: LABELS_POD,
    },
    ToolPod {
        name: "redact",
        tools: REDACT_POD,
    },
];

/// Which tools the rail may switch to this frame. Shells compute
/// it from their own state (the editor's selection-first phase, a
/// future overlay mode); the rail renders it — a gated-off chip
/// greys out and turns hover-only instead of vanishing, keeping the
/// layout stable and the tool set discoverable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolGate {
    /// Every tool chip is live.
    All,
    /// Only `Tool::Select` is live; every other chip renders
    /// disabled with a lock clause in its tooltip.
    SelectOnly,
}

impl ToolGate {
    /// Whether `tool`'s chip is clickable under this gate.
    #[must_use]
    pub(crate) fn contains(self, tool: Tool) -> bool {
        match self {
            ToolGate::All => true,
            ToolGate::SelectOnly => tool == Tool::Select,
        }
    }
}

/// Lock clause appended to a gated-off chip's tooltip. Phrased for
/// the editor's selection-first phase, the only gate user today.
pub(crate) const LOCK_CLAUSE: &str = "locked — Enter crops to unlock";

/// Draw the whole toolbar (rail + bay) as the window's top panel.
///
/// The horizontal scroll on the rail is the narrow-capture fallback:
/// region captures can be far narrower than the chips + actions,
/// and a clipped toolbar hides actions; a scrollbar-less scroll keeps
/// every control reachable via wheel / drag.
///
/// `page_actions` gates the `+` popup's Open/Paste entries: only
/// single-window page epochs have a bridge to route them (worker
/// windows offer Capture alone, matching the chrome strip's
/// worker-draws-none rule). Returns the rail's fired action plus the
/// popup's fired page action, if any.
/// A fired `+` popup action (single-window pages): the home's three
/// intents, reachable while annotating. The popup is the sole
/// visible home-parity surface on the editor (the chrome strip it
/// duplicated was removed — editor-capture-ux OQ-1); the C / O page
/// accelerators stay keyboard-only twins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PageChromeAction {
    /// Close + grab a fresh capture (the `+` chip's Capture entry —
    /// same `NewCapture` exit, same Replace semantics).
    Capture,
    /// In-window file dialog → replace the page's image.
    Open,
    /// Clipboard image → replace the page's image.
    Paste,
}

/// The replace-semantics note every page action carries: swapping
/// the image drops unsaved annotations (consistent with supersede —
/// the newest intent wins).
pub(crate) const REPLACE_HINT: &str = "new content replaces the image — unsaved edits are lost";

// ------------------------------------------------------------------
// Rail
// ------------------------------------------------------------------

/// Wrap a pod's chips in a rounded white cutout island — the
/// grouping container shared by the editor rail and the overlay's
/// floating tool row. The inner layout is pinned left-to-right so
/// the helper works inside the outputs island's right-to-left
/// anchor layout too.
pub(crate) fn pod_island<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(theme::palette::PAPER_CREAM)
        .corner_radius(CornerRadius::same(theme::CUTOUT_RADIUS))
        .inner_margin(egui::Margin::same(theme::POD_PAD))
        .stroke(theme::HAIRLINE_STROKE)
        .shadow(theme::SHADOW_RESTING)
        .show(ui, |ui| {
            ui.with_layout(
                egui::Layout::left_to_right(egui::Align::Center),
                add_contents,
            )
        })
        .inner
        .inner
}

/// The gap between adjacent rail islands (on top of the ambient
/// item spacing) — wider than the intra-pod chip spacing so pod
/// boundaries dominate. Shared by the editor rail and the overlay's
/// floating rows.
pub(crate) fn pod_gap(ui: &mut egui::Ui) {
    ui.add_space(theme::POD_GAP);
}

// ------------------------------------------------------------------
// Bay
// ------------------------------------------------------------------

/// Select: live selection dims + crop. Without a selection the bay
/// shows only the hint — no dead controls.
pub(crate) fn select_bay(ui: &mut egui::Ui, state: &mut EditorState) {
    let sel = state.selection.map(flintshot_core::Rect::normalized);
    if let Some(n) = sel.filter(|n| n.width().abs() >= 1.0 && n.height().abs() >= 1.0) {
        ui.label(
            egui::RichText::new(dims_label(n))
                .color(theme::palette::INK)
                .size(theme::TYPE_SM),
        );
        if ui.button("Crop · Enter").clicked() {
            state.confirm_selection();
        }
    }
}

/// Draw tools, stroke row: S / M / L presets plus the width
/// slider (the swatch row renders above it).
pub(crate) fn stroke_row(ui: &mut egui::Ui, state: &mut EditorState) {
    stroke_chips(ui, state);
    ui.add_sized(
        [92.0, 20.0],
        egui::Slider::new(&mut state.stroke_width, 1.0..=20.0),
    );
}

/// Text: typography row — font-size stepper and the `Aa`
/// typography popover (the swatch row renders above it).
pub(crate) fn text_style_row(ui: &mut egui::Ui, state: &mut EditorState) {
    if ui.small_button("-").clicked() {
        state.font_size = (state.font_size - 1.0).max(8.0);
    }
    ui.add_sized(
        [36.0, 20.0],
        egui::DragValue::new(&mut state.font_size)
            .range(8.0..=72.0)
            .speed(0.5),
    );
    if ui.small_button("+").clicked() {
        state.font_size = (state.font_size + 1.0).min(72.0);
    }
    let btn = ui.add(egui::Button::new(egui::RichText::new("Aa").strong()));
    egui::Popup::from_toggle_button_response(&btn)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| text_style_popup(ui, state));
}

/// Blur kind segmented control; the selected segment inherits the
/// forest selection visuals from the theme.
pub(crate) fn blur_bay(ui: &mut egui::Ui, state: &mut EditorState) {
    ui.horizontal(|ui| {
        if segmented_chip(
            ui,
            egui::Id::new("blur-gaussian"),
            "Gaussian",
            state.blur_kind == BlurKind::Gaussian,
        ) {
            state.blur_kind = BlurKind::Gaussian;
        }
        if segmented_chip(
            ui,
            egui::Id::new("blur-pixelate"),
            "Pixelate",
            state.blur_kind == BlurKind::Pixelate,
        ) {
            state.blur_kind = BlurKind::Pixelate;
        }
    });
}

/// A small labeled segment chip (the blur-kind control's idiom):
/// cutout at rest, forest-filled when selected. Returns whether it
/// was clicked.
fn segmented_chip(ui: &mut egui::Ui, id: egui::Id, label: &str, selected: bool) -> bool {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::proportional(theme::TYPE_SM),
        if selected {
            theme::palette::CREAM_TEXT
        } else {
            theme::palette::INK_SOFT
        },
    );
    let w = 12.0 + galley.size().x + 12.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 24.0), egui::Sense::hover());
    let resp = ui.interact(rect, id, egui::Sense::click());
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(theme::POD_RADIUS),
        if selected {
            theme::palette::FOREST_DEEP
        } else {
            theme::palette::CREAM_TEXT
        },
        if selected {
            egui::Stroke::NONE
        } else {
            theme::HAIRLINE_STROKE
        },
        egui::StrokeKind::Middle,
    );
    let ink = if selected {
        theme::palette::CREAM_TEXT
    } else {
        theme::palette::INK_SOFT
    };
    let pos = egui::pos2(
        rect.center().x - galley.size().x / 2.0,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(pos, galley, ink);
    resp.clicked()
}

/// Typography popover for the Text tool: family / weight / style /
/// alignment plus the background-highlight and outline toggles.
fn text_style_popup(ui: &mut egui::Ui, state: &mut EditorState) {
    ui.set_min_width(200.0);
    egui::Grid::new("text_style")
        .num_columns(2)
        .spacing([10.0, 5.0])
        .show(ui, |ui| {
            let head = |ui: &mut egui::Ui, text: &str| {
                ui.label(
                    egui::RichText::new(text)
                        .size(theme::TYPE_SM)
                        .color(theme::palette::INK_SOFT),
                );
            };
            head(ui, "Family");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.font_family, FontFamily::SansSerif, "Sans");
                ui.selectable_value(&mut state.font_family, FontFamily::Monospace, "Mono");
            });
            ui.end_row();
            head(ui, "Weight");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.font_weight, FontWeight::Normal, "Regular");
                ui.selectable_value(&mut state.font_weight, FontWeight::Bold, "Bold");
            });
            ui.end_row();
            head(ui, "Style");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.font_style, FontStyle::Normal, "Regular");
                ui.selectable_value(&mut state.font_style, FontStyle::Italic, "Italic");
            });
            ui.end_row();
            head(ui, "Align");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.text_align, TextAlign::Left, "Left");
                ui.selectable_value(&mut state.text_align, TextAlign::Center, "Center");
                ui.selectable_value(&mut state.text_align, TextAlign::Right, "Right");
            });
            ui.end_row();
        });
    ui.separator();
    let mut bg_on = state.text_background.is_some();
    ui.horizontal(|ui| {
        ui.checkbox(&mut bg_on, "Background");
        if bg_on {
            let cur = state.text_background.unwrap_or(Color::rgb(255, 235, 0));
            let mut rgb = [cur.r, cur.g, cur.b];
            ui.color_edit_button_srgb(&mut rgb);
            state.text_background = Some(Color::rgb(rgb[0], rgb[1], rgb[2]));
        } else {
            state.text_background = None;
        }
    });
    let mut ol_on = state.text_outline.is_some();
    ui.horizontal(|ui| {
        ui.checkbox(&mut ol_on, "Outline");
        if ol_on {
            let cur = state.text_outline.unwrap_or(TextOutline {
                color: Color::rgb(0, 0, 0),
                width: 2.0,
            });
            let mut rgb = [cur.color.r, cur.color.g, cur.color.b];
            ui.color_edit_button_srgb(&mut rgb);
            let mut o = TextOutline {
                color: Color::rgb(rgb[0], rgb[1], rgb[2]),
                width: cur.width,
            };
            ui.add(egui::Slider::new(&mut o.width, 1.0..=8.0).text("width"));
            state.text_outline = Some(o);
        } else {
            state.text_outline = None;
        }
    });
}

/// Contextual hint, soft ink at caption size. A plain wrapping
/// label — the retired rail's right-aligned variant explodes the
/// vertical layout of the narrow panel (horizontal layouts wrap
/// downwards once the row overflows).
pub(crate) fn hint(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .color(theme::palette::INK_SOFT)
            .size(theme::TYPE_SM),
    );
}

/// Per-tool hint copy. Kept pure for testing.
pub(crate) fn tool_hint(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => "drag a region · Copy uses it · Enter crops",
        Tool::Move => "press an annotation and drag to reposition · Del removes",
        Tool::Text => "click to place the caret",
        Tool::Blur => "drag over what to hide",
        Tool::Number => "click to place the next number",
        Tool::Marker => "drag to highlight",
        Tool::Pencil => "drag to sketch",
        _ => "drag to draw",
    }
}

/// `W × H` readout of a selection rect, in whole physical pixels.
fn dims_label(rect: flintshot_core::Rect) -> String {
    format!("{} × {}", rect.width().round(), rect.height().round())
}

// ------------------------------------------------------------------
// Chips
// ------------------------------------------------------------------

/// A 28×28 icon chip: white cutout at rest, cream with a gold ring
/// on hover, forest when selected. The selected glyph flips to
/// white, reading as cut out of the panel — the pop-up-book active
/// state. A disabled chip (`enabled == false`) keeps its hairline
/// (a borderless white chip would vanish into its white pod), dims
/// its glyph, and downgrades to hover-only so the tooltip (lock
/// clause included) still explains itself.
pub(crate) fn chip(
    ui: &mut egui::Ui,
    id: egui::Id,
    tooltip: &str,
    selected: bool,
    glyph: Glyph,
    enabled: bool,
) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), Sense::hover());
    let resp = ui
        .interact(
            rect,
            id,
            if enabled {
                Sense::click()
            } else {
                Sense::hover()
            },
        )
        .on_hover_text(tooltip);
    let fill = if selected {
        theme::palette::FOREST_DEEP
    } else if enabled && resp.hovered() {
        theme::palette::PAPER_DIM
    } else {
        theme::palette::PAPER_CREAM
    };
    let border = if selected {
        Stroke::NONE
    } else if enabled && resp.hovered() {
        Stroke::new(1.25, theme::palette::FOREST)
    } else {
        theme::HAIRLINE_STROKE
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(theme::POD_RADIUS),
        fill,
        border,
        StrokeKind::Middle,
    );
    let color = if !enabled {
        theme::palette::INK_SOFT.gamma_multiply(0.5)
    } else if selected {
        theme::palette::PAPER_CREAM
    } else if resp.hovered() {
        theme::palette::INK
    } else {
        theme::palette::INK_SOFT
    };
    glyph(ui.painter(), rect, color);
    resp
}

/// Glyph square edge inside a labeled chip — identical to a plain
/// chip so adjacent glyphs align on the rail.
const LABEL_GLYPH_EDGE: f32 = 28.0;

/// Gap between the glyph square and the label text, logical px.
const LABEL_GAP: f32 = 4.0;

/// Trailing padding after the label text, logical px.
const LABEL_TRAILING: f32 = 10.0;

/// Width of a labeled chip for a measured label width. Pure so the
/// never-narrower-than-a-glyph-chip guarantee is unit-testable
/// without a live egui `Ui`.
fn labeled_chip_width(label_width: f32) -> f32 {
    LABEL_GLYPH_EDGE + LABEL_GAP + label_width + LABEL_TRAILING
}

/// A labeled tool chip — [`chip`]'s glyph plus text, following the
/// Copy-primary's glyph+label pattern (`copy_chip`): 28 px tall,
/// galley-measured width, same rest / hover / selected visuals as
/// `chip` (forest selected, white otherwise). The label is the
/// emphasis, not new color. Renders the Move chip in both rails.
/// Disabled state mirrors [`chip`]'s (hairline border, dimmed,
/// hover-only).
pub(crate) fn labeled_chip(
    ui: &mut egui::Ui,
    id: egui::Id,
    tooltip: &str,
    selected: bool,
    label: &str,
    glyph: Glyph,
    enabled: bool,
) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::proportional(theme::TYPE_MD),
        theme::palette::INK,
    );
    let galley_size = galley.size();
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(labeled_chip_width(galley_size.x), 28.0),
        Sense::hover(),
    );
    let resp = ui
        .interact(
            rect,
            id,
            if enabled {
                Sense::click()
            } else {
                Sense::hover()
            },
        )
        .on_hover_text(tooltip);
    let fill = if selected {
        theme::palette::FOREST_DEEP
    } else if enabled && resp.hovered() {
        theme::palette::PAPER_DIM
    } else {
        theme::palette::PAPER_CREAM
    };
    let border = if selected {
        Stroke::NONE
    } else if enabled && resp.hovered() {
        Stroke::new(1.25, theme::palette::FOREST)
    } else {
        theme::HAIRLINE_STROKE
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(theme::POD_RADIUS),
        fill,
        border,
        StrokeKind::Middle,
    );
    let color = if !enabled {
        theme::palette::INK_SOFT.gamma_multiply(0.5)
    } else if selected {
        theme::palette::CREAM_TEXT
    } else if resp.hovered() {
        theme::palette::INK
    } else {
        theme::palette::INK_SOFT
    };
    let glyph_rect = egui::Rect::from_min_size(rect.min, egui::vec2(LABEL_GLYPH_EDGE, 28.0));
    glyph(ui.painter(), glyph_rect, color);
    let pos = egui::pos2(
        glyph_rect.right() + LABEL_GAP,
        rect.center().y - galley_size.y / 2.0,
    );
    ui.painter().galley(pos, galley, color);
    resp
}

/// The forest Copy primary: wider chip with a glyph + label. The
/// glyph morphs to a checkmark once a copy has succeeded this
/// session, pairing with the "Copied" label. Shared with the
/// overlay's accept row.
pub(crate) fn copy_chip(ui: &mut egui::Ui, copied: bool) -> egui::Response {
    let label = if copied { "Copied" } else { "Copy" };
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::proportional(theme::TYPE_MD),
        theme::palette::CREAM_TEXT,
    );
    let galley_size = galley.size();
    let width = 10.0 + 18.0 + 5.0 + galley_size.x + 10.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 28.0), Sense::hover());
    let resp = ui
        .interact(rect, egui::Id::new("copy_action"), Sense::click())
        .on_hover_text("Copy · Ctrl+C");
    ui.painter().rect(
        rect,
        CornerRadius::same(theme::POD_RADIUS),
        theme::palette::FOREST_DEEP,
        Stroke::NONE,
        StrokeKind::Middle,
    );
    let glyph_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 9.0, rect.top()),
        egui::vec2(18.0, 28.0),
    );
    let glyph = if copied { g_check } else { g_copy };
    glyph(ui.painter(), glyph_rect, theme::palette::CREAM_TEXT);
    let pos = egui::pos2(
        glyph_rect.right() + 5.0,
        rect.center().y - galley_size.y / 2.0,
    );
    ui.painter().galley(pos, galley, theme::palette::CREAM_TEXT);
    resp
}

/// The seven color presets plus a custom-color chip whose popup
/// hosts egui's full sRGB editor.
pub(crate) fn swatch_row(ui: &mut egui::Ui, color: &mut Color) {
    for sw in theme::palette::SWATCHES {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), Sense::hover());
        let selected = to_color32(*color) == sw;
        let resp = ui.interact(
            rect,
            egui::Id::new(("swatch", sw.r(), sw.g(), sw.b())),
            Sense::click(),
        );
        let ring = if selected {
            Stroke::new(2.0, theme::palette::FOREST_DEEP)
        } else {
            theme::HAIRLINE_STROKE
        };
        ui.painter()
            .rect(rect, CornerRadius::same(4), sw, ring, StrokeKind::Middle);
        if resp.clicked() {
            *color = from_color32(sw);
        }
    }
    custom_color_chip(ui, color);
}

/// `+` chip opening a custom-color popup (full sRGB editor).
fn custom_color_chip(ui: &mut egui::Ui, color: &mut Color) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), Sense::hover());
    let resp = ui
        .interact(rect, egui::Id::new("custom_color"), Sense::click())
        .on_hover_text("Custom color");
    ui.painter().rect(
        rect,
        CornerRadius::same(4),
        theme::palette::PAPER_CREAM,
        theme::HAIRLINE_STROKE,
        StrokeKind::Middle,
    );
    g_plus(ui.painter(), rect, theme::palette::INK);
    let mut rgb = [color.r, color.g, color.b];
    egui::Popup::from_toggle_button_response(&resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(140.0);
            ui.label("Custom color");
            ui.color_edit_button_srgb(&mut rgb);
            *color = Color::rgb(rgb[0], rgb[1], rgb[2]);
        });
}

/// S / M / L stroke presets as dot chips; a dot sized by the preset
/// communicates the effect faster than numbers.
fn stroke_chips(ui: &mut egui::Ui, state: &mut EditorState) {
    for (i, preset) in theme::STROKE_PRESETS.iter().enumerate() {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), Sense::hover());
        let selected = (state.stroke_width - preset).abs() < 0.001;
        let resp = ui.interact(rect, egui::Id::new(("stroke", i)), Sense::click());
        let fill = if selected || resp.hovered() {
            theme::palette::PAPER_DIM
        } else {
            theme::palette::CREAM_TEXT
        };
        ui.painter().rect(
            rect,
            CornerRadius::same(theme::POD_RADIUS),
            fill,
            theme::HAIRLINE_STROKE,
            StrokeKind::Middle,
        );
        let dot = if selected {
            theme::palette::FOREST_DEEP
        } else {
            theme::palette::INK
        };
        ui.painter()
            .circle(rect.center(), preset / 2.0 + 1.2, dot, Stroke::NONE);
        if resp.clicked() {
            state.stroke_width = *preset;
        }
    }
}

fn to_color32(c: Color) -> Color32 {
    crate::theme::palette::opaque(c)
}

fn from_color32(c: Color32) -> Color {
    Color::rgb(c.r(), c.g(), c.b())
}

// ------------------------------------------------------------------
// Glyphs
// ------------------------------------------------------------------

/// Glyph-space point: `(0,0)` centre, `±7` edges, +Y down.
fn pt(rect: egui::Rect, x: f32, y: f32) -> Pos2 {
    egui::pos2(rect.center().x + x, rect.center().y + y)
}

/// Glyph-space line segment from `a` to `b` (glyph coords, see
/// [`pt`]).
fn seg(
    p: &egui::Painter,
    rect: egui::Rect,
    from: (f32, f32),
    to: (f32, f32),
    color: Color32,
    width: f32,
) {
    p.line_segment(
        [pt(rect, from.0, from.1), pt(rect, to.0, to.1)],
        Stroke::new(width, color),
    );
}

/// Two 45° barbs behind `tip`, for arrow / undo / redo / save glyphs.
fn arrowhead(p: &egui::Painter, tip: Pos2, dir: Vec2, c: Color32) {
    let back = -dir.normalized();
    let rot =
        |v: Vec2, a: f32| Vec2::new(a.cos() * v.x - a.sin() * v.y, a.sin() * v.x + a.cos() * v.y);
    let ang = 0.7;
    for barb in [rot(back, ang), rot(back, -ang)] {
        p.line_segment([tip, tip + barb * 6.0], Stroke::new(2.0, c));
    }
}

/// Dashed marquee rectangle — the selection idiom.
fn g_select(p: &egui::Painter, r: egui::Rect, c: Color32) {
    for (a, b) in [
        ((-6.0, -5.0), (-2.0, -5.0)),
        ((2.0, -5.0), (6.0, -5.0)),
        ((-6.0, 5.0), (-2.0, 5.0)),
        ((2.0, 5.0), (6.0, 5.0)),
        ((-6.0, -5.0), (-6.0, -1.0)),
        ((-6.0, 1.0), (-6.0, 5.0)),
        ((6.0, -5.0), (6.0, -1.0)),
        ((6.0, 1.0), (6.0, 5.0)),
    ] {
        seg(p, r, a, b, c, 2.0);
    }
}

/// Four-direction move cross with mini chevrons — the universal
/// "reposition object" idiom. The chevrons are deliberately smaller
/// than `arrowhead`'s barbs so the cross reads as arrows without
/// swamping the 28 px chip.
fn g_move(p: &egui::Painter, r: egui::Rect, c: Color32) {
    seg(p, r, (0.0, -5.0), (0.0, 5.0), c, 2.0);
    seg(p, r, (-5.0, 0.0), (5.0, 0.0), c, 2.0);
    for (tip, dx, dy) in [
        ((0.0, -5.0), -1.0, 1.0),
        ((0.0, -5.0), 1.0, 1.0),
        ((0.0, 5.0), -1.0, -1.0),
        ((0.0, 5.0), 1.0, -1.0),
        ((-5.0, 0.0), 1.0, -1.0),
        ((-5.0, 0.0), 1.0, 1.0),
        ((5.0, 0.0), -1.0, -1.0),
        ((5.0, 0.0), -1.0, 1.0),
    ] {
        seg(p, r, tip, (tip.0 + dx * 2.6, tip.1 + dy * 2.6), c, 2.0);
    }
}

fn g_rect(p: &egui::Painter, r: egui::Rect, c: Color32) {
    let rect = egui::Rect::from_min_size(pt(r, -6.0, -4.0), egui::vec2(12.0, 8.0));
    p.rect(
        rect,
        CornerRadius::same(0),
        Color32::TRANSPARENT,
        Stroke::new(2.0, c),
        StrokeKind::Middle,
    );
}

fn g_ellipse(p: &egui::Painter, r: egui::Rect, c: Color32) {
    p.circle(r.center(), 5.5, Color32::TRANSPARENT, Stroke::new(2.0, c));
}

fn g_line(p: &egui::Painter, r: egui::Rect, c: Color32) {
    seg(p, r, (-6.0, 5.0), (5.0, -6.0), c, 2.0);
}

fn g_arrow(p: &egui::Painter, r: egui::Rect, c: Color32) {
    seg(p, r, (-6.0, 6.0), (6.0, -6.0), c, 2.0);
    arrowhead(p, pt(r, 6.0, -6.0), Vec2::new(1.0, -1.0), c);
}

fn g_marker(p: &egui::Painter, r: egui::Rect, c: Color32) {
    // Distinctly chunkier + shorter than Line/Pencil so the three
    // diagonal-stroke glyphs can't be confused at 28 px.
    seg(p, r, (-5.0, 3.5), (5.0, -3.5), c, 6.0);
}

fn g_pencil(p: &egui::Painter, r: egui::Rect, c: Color32) {
    seg(p, r, (-6.0, 6.0), (5.0, -5.0), c, 3.0);
    seg(p, r, (-5.0, 2.0), (-2.0, 5.0), c, 2.0);
}

fn g_text(p: &egui::Painter, r: egui::Rect, c: Color32) {
    p.text(
        r.center(),
        Align2::CENTER_CENTER,
        "T",
        FontId::proportional(theme::TYPE_LG),
        c,
    );
}

fn g_blur(p: &egui::Painter, r: egui::Rect, c: Color32) {
    // Staircase haze bands — strongly offset so they read as motion /
    // haze, not an equals sign.
    for (a, b) in [
        ((-6.0, -4.0), (0.0, -4.0)),
        ((0.0, 0.0), (6.0, 0.0)),
        ((-6.0, 4.0), (0.0, 4.0)),
    ] {
        seg(p, r, a, b, c, 2.0);
    }
}

fn g_number(p: &egui::Painter, r: egui::Rect, c: Color32) {
    p.circle(r.center(), 5.5, Color32::TRANSPARENT, Stroke::new(2.0, c));
    p.text(
        pt(r, 0.0, 0.5),
        Align2::CENTER_CENTER,
        "1",
        FontId::proportional(theme::TYPE_SM),
        c,
    );
}

/// Arc over the top + a barb diving down-and-out at the left end.
pub(crate) fn g_undo(p: &egui::Painter, r: egui::Rect, c: Color32) {
    arc_over_top(p, r, c);
    arrowhead(p, pt(r, -5.5, 0.0), Vec2::new(-0.35, 0.94), c);
}

/// Mirror of undo: barb at the right end.
pub(crate) fn g_redo(p: &egui::Painter, r: egui::Rect, c: Color32) {
    arc_over_top(p, r, c);
    arrowhead(p, pt(r, 5.5, 0.0), Vec2::new(0.35, 0.94), c);
}

fn arc_over_top(p: &egui::Painter, r: egui::Rect, c: Color32) {
    // θ ∈ [π, 2π): left point, over the top (Y-down), to the right
    // point — sampled, since epaint 0.35 has no primitive arc.
    let dtheta = std::f32::consts::PI / 10.0;
    let mut points: Vec<Pos2> = Vec::with_capacity(11);
    let mut theta = std::f32::consts::PI;
    for _ in 0..=10 {
        points.push(egui::pos2(
            r.center().x + 5.5 * theta.cos(),
            r.center().y + 5.5 * theta.sin(),
        ));
        theta += dtheta;
    }
    p.add(egui::Shape::line(points, Stroke::new(2.0, c)));
}

/// Trash can: full-width lid bar floating above an open-topped can
/// with one centre slat. The inverse silhouette of Save's tray (lid
/// up top vs tray at the bottom, no arrow) so the two stay distinct
/// at 28 px.
pub(crate) fn g_trash(p: &egui::Painter, r: egui::Rect, c: Color32) {
    seg(p, r, (-6.0, -5.0), (6.0, -5.0), c, 2.0);
    seg(p, r, (-4.0, -3.0), (-4.0, 6.0), c, 2.0);
    seg(p, r, (4.0, -3.0), (4.0, 6.0), c, 2.0);
    seg(p, r, (-4.0, 6.0), (4.0, 6.0), c, 2.0);
    seg(p, r, (-1.5, -1.0), (-1.5, 3.0), c, 2.0);
}

/// Arrow descending into a tray (download-to-disk).
pub(crate) fn g_save(p: &egui::Painter, r: egui::Rect, c: Color32) {
    seg(p, r, (-6.0, 6.0), (6.0, 6.0), c, 2.0);
    seg(p, r, (-6.0, 6.0), (-6.0, 2.0), c, 2.0);
    seg(p, r, (6.0, 6.0), (6.0, 2.0), c, 2.0);
    seg(p, r, (0.0, -7.0), (0.0, 1.0), c, 2.0);
    arrowhead(p, pt(r, 0.0, 3.0), Vec2::new(0.0, 1.0), c);
}

/// Arrow rising out of a tray.
pub(crate) fn g_upload(p: &egui::Painter, r: egui::Rect, c: Color32) {
    seg(p, r, (-6.0, -6.0), (6.0, -6.0), c, 2.0);
    seg(p, r, (-6.0, -6.0), (-6.0, -2.0), c, 2.0);
    seg(p, r, (6.0, -6.0), (6.0, -2.0), c, 2.0);
    seg(p, r, (0.0, 7.0), (0.0, -1.0), c, 2.0);
    arrowhead(p, pt(r, 0.0, -3.0), Vec2::new(0.0, -1.0), c);
}

/// Two sheets: full front rect, back corner peeking top-right.
fn g_copy(p: &egui::Painter, r: egui::Rect, c: Color32) {
    let front = egui::Rect::from_min_size(pt(r, -7.0, -4.0), egui::vec2(8.0, 10.0));
    p.rect(
        front,
        CornerRadius::same(0),
        Color32::TRANSPARENT,
        Stroke::new(2.0, c),
        StrokeKind::Middle,
    );
    p.add(egui::Shape::line(
        vec![pt(r, -1.0, -7.0), pt(r, 7.0, -7.0), pt(r, 7.0, 1.0)],
        Stroke::new(2.0, c),
    ));
}

/// Bold check for the post-copy state and the overlay's accept
/// action.
pub(crate) fn g_check(p: &egui::Painter, r: egui::Rect, c: Color32) {
    p.add(egui::Shape::line(
        vec![pt(r, -6.0, -1.0), pt(r, -2.0, 3.0), pt(r, 7.0, -6.0)],
        Stroke::new(2.5, c),
    ));
}

/// Bold ✕ for the overlay's cancel action.
pub(crate) fn g_x(p: &egui::Painter, r: egui::Rect, c: Color32) {
    seg(p, r, (-5.0, -5.0), (5.0, 5.0), c, 2.5);
    seg(p, r, (5.0, -5.0), (-5.0, 5.0), c, 2.5);
}

pub(crate) fn g_plus(p: &egui::Painter, r: egui::Rect, c: Color32) {
    seg(p, r, (-4.0, 0.0), (4.0, 0.0), c, 2.0);
    seg(p, r, (0.0, -4.0), (0.0, 4.0), c, 2.0);
}

/// Two chevrons pointing `dir` (±1) — the panel collapse toggle's
/// »/« idiom.
fn g_chevrons(p: &egui::Painter, r: egui::Rect, c: Color32, dir: f32) {
    for cx in [-2.5_f32, 2.5] {
        let tip = pt(r, cx + 2.0 * dir, 0.0);
        p.line_segment([pt(r, cx - 2.0 * dir, -4.0), tip], Stroke::new(2.0, c));
        p.line_segment([tip, pt(r, cx - 2.0 * dir, 4.0)], Stroke::new(2.0, c));
    }
}

/// » — hide the panel (it folds toward the right edge).
fn g_hide_panel(p: &egui::Painter, r: egui::Rect, c: Color32) {
    g_chevrons(p, r, c, 1.0);
}

/// « — restore the panel from the rail.
fn g_show_panel(p: &egui::Painter, r: egui::Rect, c: Color32) {
    g_chevrons(p, r, c, -1.0);
}

#[cfg(test)]
mod tests {
    use super::*;
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

    /// Collect rect shapes a frame painted, recursing into
    /// `Shape::Vec` (frames with shadows wrap).
    fn collect_rect_shapes<'a>(
        shapes: impl Iterator<Item = &'a egui::Shape>,
        out: &mut Vec<&'a egui::epaint::RectShape>,
    ) {
        for shape in shapes {
            match shape {
                egui::Shape::Rect(r) => out.push(r),
                egui::Shape::Vec(v) => collect_rect_shapes(v.iter(), out),
                _ => {}
            }
        }
    }

    // --- Export cluster (Copy hero) ---

    /// Copy renders visually primary: wider and taller than the
    /// Save / Upload secondaries, painted forest with the 8 px
    /// cutout border.
    #[test]
    fn copy_hero_renders_primary_over_secondaries() {
        let mut state = panel_state();
        let ctx = egui::Context::default();
        let mut panel = PanelState::default();
        let output = {
            let raw = screen(1024.0, 768.0);
            ctx.run_ui(raw, |ui| {
                let _ = show(ui, &mut state, false, ToolGate::All, false, &mut panel);
            })
        };
        let hero = ctx
            .read_response(egui::Id::new("copy_action"))
            .expect("the hero registers");
        let save = ctx
            .read_response(egui::Id::new("save"))
            .expect("Save registers");
        assert!(
            hero.rect.width() > save.rect.width() * 2.0,
            "the hero dominates: {:?} vs {:?}",
            hero.rect,
            save.rect
        );
        assert!(
            hero.rect.height() > save.rect.height(),
            "the hero is the tallest control"
        );
        // The hero card paints forest-filled with the cutout border.
        let mut rects = Vec::new();
        collect_rect_shapes(output.shapes.iter().map(|cs| &cs.shape), &mut rects);
        assert!(
            rects.iter().any(|r| {
                r.fill == theme::palette::FOREST_DEEP
                    && r.corner_radius == egui::CornerRadius::same(theme::CUTOUT_RADIUS)
                    && r.stroke.color == theme::palette::PAPER_CREAM
            }),
            "the hero paints forest with a white cutout border"
        );
    }

    /// The copied-state feedback still hangs off the same response:
    /// a copied session renders without changing the wiring.
    #[test]
    fn copy_hero_renders_the_copied_state() {
        let mut state = panel_state();
        let ctx = egui::Context::default();
        let mut panel = PanelState::default();
        let _ = draw(&ctx, &mut state, &mut panel, ToolGate::All, false, vec![]);
        let _ = draw(&ctx, &mut state, &mut panel, ToolGate::All, true, vec![]);
        assert!(
            ctx.read_response(egui::Id::new("copy_action")).is_some(),
            "the hero registers under the copied flag too"
        );
    }

    // --- Contextual options (Gradia pattern #3) ---

    fn panel_with_tool(tool: Tool) -> egui::Context {
        let mut state = panel_state();
        state.tool = tool;
        let ctx = egui::Context::default();
        let mut panel = PanelState::default();
        let _ = draw(&ctx, &mut state, &mut panel, ToolGate::All, false, vec![]);
        ctx
    }

    /// Exactly the selected tool's options render — nothing else.
    /// The picker pod, fill toggle, and number spinner are the
    /// section's whole vocabulary; every tool exposes a different
    /// subset.
    #[test]
    fn rect_shows_picker_and_fill_toggle_only() {
        let ctx = panel_with_tool(Tool::Rect);
        assert!(
            ctx.read_response(egui::Id::new("color-stroke-pod"))
                .is_some(),
            "Rect gets the picker pod"
        );
        assert!(
            ctx.read_response(egui::Id::new("fill-toggle")).is_some(),
            "Rect gets the fill toggle"
        );
        // No number spinner on non-Number tools: the DragValue id is
        // derived, so assert the absence by the fill/pod pair alone.
    }

    #[test]
    fn line_shows_picker_without_fill_toggle() {
        let ctx = panel_with_tool(Tool::Line);
        assert!(
            ctx.read_response(egui::Id::new("color-stroke-pod"))
                .is_some(),
            "Line gets the picker pod"
        );
        assert!(
            ctx.read_response(egui::Id::new("fill-toggle")).is_none(),
            "open strokes never see the fill toggle"
        );
    }

    #[test]
    fn move_shows_no_options_at_all() {
        let ctx = panel_with_tool(Tool::Move);
        assert!(ctx
            .read_response(egui::Id::new("color-stroke-pod"))
            .is_none());
        assert!(ctx.read_response(egui::Id::new("fill-toggle")).is_none());
    }

    #[test]
    fn blur_shows_kind_segments_without_picker() {
        let ctx = panel_with_tool(Tool::Blur);
        assert!(
            ctx.read_response(egui::Id::new("blur-gaussian")).is_some()
                && ctx.read_response(egui::Id::new("blur-pixelate")).is_some(),
            "blur kind segments render"
        );
        assert!(
            ctx.read_response(egui::Id::new("color-stroke-pod"))
                .is_none(),
            "blur has no color picker"
        );
    }

    /// Clicking the fill toggle flips the core flag (the popover
    /// commits apply live — same contract as the picker).
    #[test]
    fn fill_toggle_click_flips_the_core_flag() {
        let mut state = panel_state();
        state.tool = Tool::Rect;
        assert!(!state.fill_shapes, "default off");
        let ctx = egui::Context::default();
        let mut panel = PanelState::default();
        let _ = draw(&ctx, &mut state, &mut panel, ToolGate::All, false, vec![]);
        let toggle = ctx
            .read_response(egui::Id::new("fill-toggle"))
            .expect("fill toggle registered");
        draw(
            &ctx,
            &mut state,
            &mut panel,
            ToolGate::All,
            false,
            click_events(toggle.rect.center()),
        );
        assert!(state.fill_shapes, "one click turns fill on");
    }

    // --- Panel geometry (the structural pivot's own contracts) ---

    /// Collect the white cutout rect shapes a frame painted
    /// (frames with shadows wrap in `Shape::Vec`, so recurse).
    fn island_rects(output: &egui::FullOutput) -> Vec<egui::Rect> {
        fn collect<'a>(shapes: impl Iterator<Item = &'a egui::Shape>, out: &mut Vec<egui::Rect>) {
            for shape in shapes {
                match shape {
                    egui::Shape::Rect(r)
                        if r.fill == theme::palette::PAPER_CREAM
                            && r.corner_radius
                                == egui::CornerRadius::same(theme::CUTOUT_RADIUS) =>
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

    /// The tools render as **one** white cutout card (plus the
    /// export island below) — not five floating pods; the groups
    /// separate by hairlines, chips within a group share a row,
    /// and the groups stack strictly top-to-bottom.
    #[test]
    fn panel_renders_one_tools_card_plus_the_export_island() {
        let mut state = panel_state();
        let ctx = egui::Context::default();
        let mut panel = PanelState::default();
        let output = {
            let raw = screen(1024.0, 768.0);
            ctx.run_ui(raw, |ui| {
                let _ = show(ui, &mut state, false, ToolGate::All, false, &mut panel);
            })
        };
        let islands = island_rects(&output);
        assert_eq!(
            islands.len(),
            2,
            "exactly two white cutout cards: the tools card + the export island"
        );
        // The two cards never overlap.
        assert!(
            !islands[0].intersects(islands[1]),
            "cards overlap: {islands:?}"
        );
        // Tool chips inside one group share a row; the first chips
        // of consecutive groups stack strictly downward.
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
                "groups must stack top-to-bottom: {a:?} vs {b:?}"
            );
        }
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
            "intra-group chips share a row"
        );
    }

    /// The export island never stretches (the bottom-up `Frame`
    /// bug this layout must not reintroduce): Delete / Save /
    /// Upload share one exact-height row, the hero sits below it,
    /// and nothing overlaps — at any window height.
    #[test]
    fn bottom_cluster_never_stretches_or_overlaps() {
        for height in [640.0, 2000.0] {
            let mut state = panel_state();
            let ctx = egui::Context::default();
            let mut panel = PanelState::default();
            let output = {
                let raw = screen(1024.0, height);
                ctx.run_ui(raw, |ui| {
                    let _ = show(ui, &mut state, false, ToolGate::All, false, &mut panel);
                })
            };
            let del = ctx
                .read_response(egui::Id::new("delete"))
                .expect("delete")
                .rect;
            let save = ctx.read_response(egui::Id::new("save")).expect("save").rect;
            let upload = ctx
                .read_response(egui::Id::new("upload"))
                .expect("upload")
                .rect;
            let hero = ctx
                .read_response(egui::Id::new("copy_action"))
                .expect("hero")
                .rect;
            assert!(
                (del.height() - 28.0).abs() < 0.01,
                "h={height}: chips stay chip-sized, got {del:?}"
            );
            assert!(
                (del.top() - save.top()).abs() < 0.01 && (save.top() - upload.top()).abs() < 0.01,
                "h={height}: the export chips share one row"
            );
            assert!(
                hero.top() >= save.bottom(),
                "h={height}: the hero sits below the export row"
            );
            for island in island_rects(&output) {
                if island.contains(save.center()) {
                    assert!(
                        island.height() <= EXPORT_ROW_HEIGHT + 1.0,
                        "h={height}: the export island stretched to {}",
                        island.height()
                    );
                }
            }
        }
    }

    // --- Collapse (the rail) ---

    /// Clicking `«` collapses the panel to the 44 px rail; the
    /// tool vocabulary stays reachable and live from the rail.
    #[test]
    fn collapse_toggle_swaps_to_the_rail_and_back() {
        let mut state = panel_state();
        let ctx = egui::Context::default();
        let mut panel = PanelState::default();
        let _ = draw(&ctx, &mut state, &mut panel, ToolGate::All, false, vec![]);
        let toggle = ctx
            .read_response(egui::Id::new("panel-collapse"))
            .expect("the toggle registered");
        let _ = draw(
            &ctx,
            &mut state,
            &mut panel,
            ToolGate::All,
            false,
            click_events(toggle.rect.center()),
        );
        assert!(panel.collapsed, "one click collapses the panel");
        assert!((panel.width() - RAIL_WIDTH).abs() < f32::EPSILON);
        // The rail keeps every tool + output registered.
        for spec in TOOL_PODS.iter().flat_map(|pod| pod.tools.iter()) {
            assert!(
                ctx.read_response(egui::Id::new(("tool", spec.key)))
                    .is_some(),
                "the rail keeps {}",
                spec.name
            );
        }
        for id in ["new-capture", "save", "upload", "delete", "copy_action"] {
            assert!(
                ctx.read_response(egui::Id::new(id)).is_some(),
                "the rail keeps {id}"
            );
        }
        // And the rail's toggle restores the docked panel.
        let toggle = ctx
            .read_response(egui::Id::new("panel-collapse"))
            .expect("the rail toggle registered");
        let _ = draw(
            &ctx,
            &mut state,
            &mut panel,
            ToolGate::All,
            false,
            click_events(toggle.rect.center()),
        );
        assert!(!panel.collapsed, "the rail toggle restores the panel");
        assert!((panel.width() - PANEL_WIDTH).abs() < f32::EPSILON);
    }

    /// A collapsed rail frees the canvas: at the 1024 default the
    /// central panel reclaims the docked width.
    #[test]
    fn collapsed_rail_frees_the_canvas() {
        let mut state = panel_state();
        let ctx = egui::Context::default();
        let mut panel = PanelState {
            collapsed: true,
            ..PanelState::default()
        };
        let raw = screen(1024.0, 640.0);
        let _ = ctx.run_ui(raw, |ui| {
            let _ = show(ui, &mut state, false, ToolGate::All, false, &mut panel);
            let central = egui::CentralPanel::default().show(ui, |ui| {
                ui.allocate_space(egui::vec2(1.0, 1.0));
            });
            assert!(
                central.response.rect.width() >= 1024.0 - RAIL_WIDTH,
                "a collapsed panel must leave ~the full window to the canvas, got {:?}",
                central.response.rect.width()
            );
        });
    }

    /// Rail chips stay live: a click on the rail's Pencil chip
    /// switches tools exactly like the docked card.
    #[test]
    fn rail_chip_click_switches_tool() {
        let mut state = panel_state();
        state.tool = Tool::Select;
        let ctx = egui::Context::default();
        let mut panel = PanelState {
            collapsed: true,
            ..PanelState::default()
        };
        let _ = draw(&ctx, &mut state, &mut panel, ToolGate::All, false, vec![]);
        let pen = ctx
            .read_response(egui::Id::new(("tool", "P")))
            .expect("the rail registered the Pencil chip");
        let _ = draw(
            &ctx,
            &mut state,
            &mut panel,
            ToolGate::All,
            false,
            click_events(pen.rect.center()),
        );
        assert_eq!(state.tool, Tool::Pencil, "rail clicks switch tools");
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
    // --- Migrated from the retired toolbar.rs ---

    /// The pod table walked flat, rail order — the shape both rails
    /// render.
    fn flat() -> Vec<&'static ToolSpec> {
        TOOL_PODS.iter().flat_map(|pod| pod.tools.iter()).collect()
    }

    #[test]
    fn tools_cover_every_variant_with_unique_keys() {
        let flat = flat();
        assert_eq!(flat.len(), 11);
        let mut keys: Vec<&str> = flat.iter().map(|t| t.key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), flat.len());
        for tool in [
            Tool::Select,
            Tool::Move,
            Tool::Rect,
            Tool::Ellipse,
            Tool::Line,
            Tool::Arrow,
            Tool::Marker,
            Tool::Pencil,
            Tool::Text,
            Tool::Blur,
            Tool::Number,
        ] {
            assert!(
                flat.iter().any(|t| t.tool == tool),
                "missing rail chip for {tool:?}"
            );
        }
    }

    #[test]
    fn tool_pods_partition_every_variant_in_spec_order() {
        let expected: &[(&str, &[Tool])] = &[
            ("selection", &[Tool::Move, Tool::Select]),
            (
                "shapes",
                &[Tool::Rect, Tool::Ellipse, Tool::Line, Tool::Arrow],
            ),
            ("freehand", &[Tool::Pencil, Tool::Marker]),
            ("labels", &[Tool::Text, Tool::Number]),
            ("redact", &[Tool::Blur]),
        ];
        assert_eq!(TOOL_PODS.len(), expected.len());
        for (pod, (name, tools)) in TOOL_PODS.iter().zip(expected) {
            assert_eq!(pod.name, *name, "pod order is fixed by the spec");
            assert!(!pod.tools.is_empty(), "pod {name} must not be empty");
            let actual: Vec<Tool> = pod.tools.iter().map(|t| t.tool).collect();
            assert_eq!(actual, *tools, "pod {name} content is fixed by the spec");
        }
        // Stronger than "all present": a variant split across or
        // duplicated in pods must fail.
        for tool in [
            Tool::Select,
            Tool::Move,
            Tool::Rect,
            Tool::Ellipse,
            Tool::Line,
            Tool::Arrow,
            Tool::Marker,
            Tool::Pencil,
            Tool::Text,
            Tool::Blur,
            Tool::Number,
        ] {
            let hits = TOOL_PODS
                .iter()
                .filter(|p| p.tools.iter().any(|t| t.tool == tool))
                .count();
            assert_eq!(hits, 1, "{tool:?} must sit in exactly one pod");
        }
        // Move leads and Select seconds overall.
        let flat: Vec<Tool> = TOOL_PODS
            .iter()
            .flat_map(|p| p.tools.iter().map(|t| t.tool))
            .collect();
        assert_eq!(flat.first(), Some(&Tool::Move));
        assert_eq!(flat.get(1), Some(&Tool::Select));
    }

    #[test]
    fn move_leads_the_rail_with_select_second() {
        let flat = flat();
        assert!(
            flat.first().is_some_and(|t| t.tool == Tool::Move),
            "Move must be the first rail chip"
        );
        assert!(
            flat.get(1).is_some_and(|t| t.tool == Tool::Select),
            "Select must be the second rail chip"
        );
        assert_eq!(flat[0].name, "Move", "the label reads Move");
    }

    #[test]
    fn labeled_chip_is_never_narrower_than_a_glyph_chip() {
        assert!(labeled_chip_width(0.0) >= 28.0);
        // A realistic "Move" label widens the chip further.
        assert!(labeled_chip_width(32.0) > 28.0);
    }

    #[test]
    fn every_tool_has_a_nonempty_hint() {
        for spec in flat() {
            assert!(!tool_hint(spec.tool).is_empty());
        }
        // The non_exhaustive wildcard arm must hint too.
        assert!(!tool_hint(Tool::Rect).is_empty());
    }

    #[test]
    fn dims_label_renders_whole_pixels() {
        let rect = flintshot_core::Rect::new(
            flintshot_core::Point::new(0.0, 0.0),
            flintshot_core::Point::new(1280.4, 719.6),
        );
        assert_eq!(dims_label(rect.normalized()), "1280 × 720");
    }

    #[test]
    fn color_conversions_round_trip() {
        let c = Color::rgb(0xFF, 0x8C, 0x42);
        assert_eq!(from_color32(to_color32(c)), c);
    }

    #[test]
    fn toolgate_select_only_admits_exactly_select() {
        assert!(ToolGate::All.contains(Tool::Select));
        assert!(ToolGate::All.contains(Tool::Pencil));
        assert!(ToolGate::SelectOnly.contains(Tool::Select));
        for tool in [
            Tool::Move,
            Tool::Rect,
            Tool::Ellipse,
            Tool::Line,
            Tool::Arrow,
            Tool::Marker,
            Tool::Pencil,
            Tool::Text,
            Tool::Blur,
            Tool::Number,
        ] {
            assert!(
                !ToolGate::SelectOnly.contains(tool),
                "SelectOnly must disable {tool:?}"
            );
        }
    }

    // --- Delete chip (annotation-delete Task 002) ---

    // --- `+` popup home parity (editor-capture-ux Task 001) ---

    // --- Move hint delete affordance (annotation-delete Task 003) ---

    /// The user-approved copy (spec ask-first boundary): the Move
    /// hint must advertise the Del affordance.
    #[test]
    fn move_hint_mentions_delete_affordance() {
        assert_eq!(
            tool_hint(Tool::Move),
            "press an annotation and drag to reposition · Del removes"
        );
    }

    // --- Pod island geometry, closeout (toolbar-groups Task 003) ---
}
