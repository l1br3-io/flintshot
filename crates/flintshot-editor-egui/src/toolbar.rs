//! The editor toolbar: a two-row instrument panel.
//!
//! **Rail** (top, stable across tools): semantically grouped pods,
//! each a rounded graphite island separated by gaps — capture `+` ·
//! selection (Move, Select) · shapes · freehand · labels · redact ·
//! history (undo / redo / delete) · outputs (Save, Pin, Upload, and
//! the ember Copy primary, right-aligned). **Bay** (bottom,
//! per-tool): only the controls the active tool can use, plus a
//! contextual hint on the right.
//!
//! The chips, glyphs, and `ToolPod` table are shared with the Phase
//! 5 overlay's floating toolbar (plan decision 4: reuse the chips,
//! new layout only) — hence `pub(crate)` on the primitives below.
//!
//! All icons are painter-drawn vector glyphs (see
//! `docs/adrs/ADR-0027-painter-drawn-toolbar-glyphs.md`): they share
//! the 2px stroke language of the annotations themselves, render
//! identically on every platform, and add no icon-font dependency.
//! No emoji in labels — tofu boxes on font-constrained systems are a
//! known failure mode of the previous toolbar.

use egui::containers::scroll_area::ScrollBarVisibility;
use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Sense, Stroke, StrokeKind, Vec2};
use flintshot_core::{
    BlurKind, Color, EditorState, FontFamily, FontStyle, FontWeight, NewCaptureMode, TextAlign,
    TextOutline, Tool,
};

use crate::theme;

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
    /// Pin button — compose, write temp PNG, spawn pin worker.
    Pin,
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
type Glyph = fn(&egui::Painter, egui::Rect, Color32);

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
/// The pod arrays below are the single source of truth; the editor
/// rail walks [`TOOL_PODS`] and the overlay's floating rail walks
/// [`flat_tools`] until it adopts the pods (toolbar-groups task
/// 002).
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

/// The pod table walked flat, rail order — the data source for the
/// overlay's floating rail until it adopts the pods itself
/// (toolbar-groups task 002).
pub(crate) fn flat_tools() -> impl Iterator<Item = &'static ToolSpec> {
    TOOL_PODS.iter().flat_map(|pod| pod.tools.iter())
}

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
const LOCK_CLAUSE: &str = "locked — Enter crops to unlock";

/// Draw the whole toolbar (rail + bay) as the window's top panel.
///
/// The horizontal scroll on the rail is the narrow-capture fallback:
/// region captures can be far narrower than the chips + actions,
/// and a clipped toolbar hides actions; a scrollbar-less scroll keeps
/// every control reachable via wheel / drag.
pub(crate) fn show(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    copied: bool,
    gate: ToolGate,
) -> ToolbarAction {
    let frame = egui::Frame::new()
        .fill(theme::GRAPHITE_900)
        .inner_margin(egui::Margin::symmetric(8, 5));
    egui::Panel::top("editor_toolbar")
        .frame(frame)
        .show(ui, |ui| {
            let action = draw_rail(ui, state, copied, gate);
            draw_bay(ui, state);
            action
        })
        .inner
}

// ------------------------------------------------------------------
// Rail
// ------------------------------------------------------------------

/// Wrap a pod's chips in a rounded graphite island — the grouping
/// container shared by the editor rail and the overlay's floating
/// tool row. The inner layout is pinned left-to-right so the helper
/// works inside the outputs island's right-to-left anchor layout
/// too.
pub(crate) fn pod_island<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(theme::GRAPHITE_800)
        .corner_radius(CornerRadius::same(theme::POD_CORNER))
        .inner_margin(egui::Margin::same(theme::POD_PAD))
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
/// boundaries dominate.
fn pod_gap(ui: &mut egui::Ui) {
    ui.add_space(theme::POD_GAP);
}

fn draw_rail(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    copied: bool,
    gate: ToolGate,
) -> ToolbarAction {
    egui::ScrollArea::horizontal()
        .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let mut action = pod_island(ui, new_capture_chip);
                pod_gap(ui);
                for pod in TOOL_PODS {
                    pod_island(ui, |ui| {
                        for spec in pod.tools {
                            let enabled = gate.contains(spec.tool);
                            let selected = state.tool == spec.tool;
                            let tooltip = if enabled {
                                format!("{} · {}", spec.name, spec.key)
                            } else {
                                format!("{} · {} · {LOCK_CLAUSE}", spec.name, spec.key)
                            };
                            // Move renders labeled (rail emphasis); ids stay
                            // key-derived so tooltips and clicks are unchanged.
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
                                // Core `set_tool` owns the "the selection is
                                // Move-tool chrome" invariant: leaving Move
                                // drops the persistent selection.
                                state.set_tool(spec.tool);
                            }
                        }
                    });
                    pod_gap(ui);
                }
                let history = pod_island(ui, |ui| {
                    let mut action = ToolbarAction::None;
                    if chip(
                        ui,
                        egui::Id::new("undo"),
                        "Undo · Ctrl+Z",
                        false,
                        g_undo,
                        true,
                    )
                    .clicked()
                    {
                        action = ToolbarAction::Undo;
                    }
                    if chip(
                        ui,
                        egui::Id::new("redo"),
                        "Redo · Ctrl+Y",
                        false,
                        g_redo,
                        true,
                    )
                    .clicked()
                    {
                        action = ToolbarAction::Redo;
                    }
                    // Enabled iff an annotation is selected (persistent
                    // selection, task 001); disabled reads greyed /
                    // hover-only via the same `enabled` path the locked
                    // tool chips use.
                    if chip(
                        ui,
                        egui::Id::new("delete"),
                        "Delete · Del",
                        false,
                        g_trash,
                        state.selected_index.is_some(),
                    )
                    .clicked()
                    {
                        action = ToolbarAction::Delete;
                    }
                    action
                });
                if action == ToolbarAction::None {
                    action = history;
                }
                let output = rail_actions(ui, copied);
                if action == ToolbarAction::None {
                    action = output;
                }
                action
            })
            .inner
        })
        .inner
}

/// The Win11-style "new snip" affordance leading the rail. A popup
/// (not an immediate action) so both capture kinds hang off one
/// chip; Ctrl+N is the keyboard fast path for the region default.
/// The editor closes on selection, which tears this popup down with
/// it — no explicit close needed.
fn new_capture_chip(ui: &mut egui::Ui) -> ToolbarAction {
    let mut action = ToolbarAction::None;
    let new_chip = chip(
        ui,
        egui::Id::new("new-capture"),
        "New capture · Ctrl+N",
        false,
        g_plus,
        true,
    );
    egui::Popup::from_toggle_button_response(&new_chip)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(160.0);
            ui.vertical(|ui| {
                if ui.button("Region · Ctrl+N").clicked() {
                    action = ToolbarAction::NewCapture(NewCaptureMode::Region);
                }
                if ui.button("Fullscreen").clicked() {
                    action = ToolbarAction::NewCapture(NewCaptureMode::Fullscreen);
                }
            });
        });
    action
}

/// Output actions as the right-aligned outputs island: Save / Pin /
/// Upload as quiet icon chips, Copy as the ember primary (the
/// dominant terminal action for screenshot flows). The right-to-
/// left anchor layout only places the island; inside it the chips
/// run left-to-right in visual order. On a successful copy the
/// glyph morphs sheet→check and the label reads "Copied".
fn rail_actions(ui: &mut egui::Ui, copied: bool) -> ToolbarAction {
    let mut action = ToolbarAction::None;
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        pod_island(ui, |ui| {
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
            if chip(
                ui,
                egui::Id::new("pin"),
                "Pin to desktop",
                false,
                g_pin,
                true,
            )
            .clicked()
            {
                action = ToolbarAction::Pin;
            }
            if chip(ui, egui::Id::new("upload"), "Upload", false, g_upload, true).clicked() {
                action = ToolbarAction::Upload;
            }
            if copy_chip(ui, copied).clicked() {
                action = ToolbarAction::Copy;
            }
        });
    });
    action
}

// ------------------------------------------------------------------
// Bay
// ------------------------------------------------------------------

fn draw_bay(ui: &mut egui::Ui, state: &mut EditorState) {
    hairline(ui);
    ui.add_space(3.0);
    ui.horizontal(|ui| {
        match state.tool {
            Tool::Select => select_bay(ui, state),
            // Move has no adjustable properties — nothing to grab,
            // nothing to drag — so its bay is just the hint.
            Tool::Move => {}
            Tool::Text => text_bay(ui, state),
            Tool::Blur => blur_bay(ui, state),
            Tool::Number => swatch_row(ui, &mut state.color),
            // Future `Tool` variants (the enum is `#[non_exhaustive]`)
            // and the six drawing tools share the generic controls.
            _ => draw_controls(ui, state),
        }
        hint(ui, tool_hint(state.tool));
    });
}

/// Full-width 1px rule between rail and bay.
fn hairline(ui: &mut egui::Ui) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 1.0), Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0, theme::GRAPHITE_500),
    );
}

/// Select: live selection dims + crop. Without a selection the bay
/// shows only the hint — no dead controls.
fn select_bay(ui: &mut egui::Ui, state: &mut EditorState) {
    let sel = state.selection.map(flintshot_core::Rect::normalized);
    if let Some(n) = sel.filter(|n| n.width().abs() >= 1.0 && n.height().abs() >= 1.0) {
        ui.label(
            egui::RichText::new(dims_label(n))
                .color(theme::CHALK)
                .size(12.0),
        );
        if ui.button("Crop · Enter").clicked() {
            state.confirm_selection();
        }
    }
}

/// Draw tools: color swatches, stroke presets, width slider.
fn draw_controls(ui: &mut egui::Ui, state: &mut EditorState) {
    swatch_row(ui, &mut state.color);
    ui.separator();
    stroke_chips(ui, state);
    ui.add_sized(
        [92.0, 20.0],
        egui::Slider::new(&mut state.stroke_width, 1.0..=20.0),
    );
}

/// Text: its own swatches (decoupled from the drawing color, so red
/// arrows and white labels coexist), font-size stepper, and the
/// `Aa` typography popover.
fn text_bay(ui: &mut egui::Ui, state: &mut EditorState) {
    swatch_row(ui, &mut state.text_color);
    ui.separator();
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
    ui.separator();
    let btn = ui.add(egui::Button::new(egui::RichText::new("Aa").strong()));
    egui::Popup::from_toggle_button_response(&btn)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| text_style_popup(ui, state));
}

/// Blur kind segmented control; the selected segment inherits the
/// ember selection visuals from the theme.
fn blur_bay(ui: &mut egui::Ui, state: &mut EditorState) {
    ui.selectable_value(&mut state.blur_kind, BlurKind::Gaussian, "Gaussian");
    ui.selectable_value(&mut state.blur_kind, BlurKind::Pixelate, "Pixelate");
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
                ui.label(egui::RichText::new(text).size(12.0).color(theme::SLATE));
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

/// Contextual hint, right-aligned in slate. Replaces the old static
/// "Drag to draw · Ctrl+Z undo · Esc cancel" row with guidance that
/// is true for the selected tool.
fn hint(ui: &mut egui::Ui, text: &str) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.label(egui::RichText::new(text).color(theme::SLATE).size(12.0));
    });
}

/// Per-tool hint copy. Kept pure for testing.
fn tool_hint(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => "drag a region · Copy and Pin use it · Enter crops",
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

/// A 28×28 icon chip: graphite at rest, lighter on hover, ember
/// when selected. The selected glyph flips to ink, reading as a
/// spark cut out of the panel — the toolbar's only color besides
/// the Copy primary. A disabled chip (`enabled == false`) drops
/// its border, dims its glyph, and downgrades to hover-only so the
/// tooltip (lock clause included) still explains itself.
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
        theme::EMBER
    } else if enabled && resp.hovered() {
        theme::GRAPHITE_500
    } else {
        theme::GRAPHITE_700
    };
    let border = if selected || !enabled {
        Stroke::NONE
    } else {
        Stroke::new(1.0, theme::GRAPHITE_500)
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(6),
        fill,
        border,
        StrokeKind::Middle,
    );
    let color = if !enabled {
        theme::SLATE.gamma_multiply(0.5)
    } else if selected {
        theme::INK
    } else if resp.hovered() {
        theme::CHALK
    } else {
        theme::SLATE
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
/// `chip` (ember selected, graphite otherwise). The label is the
/// emphasis, not new color. Renders the Move chip in both rails.
/// Disabled state mirrors [`chip`]'s (no border, dimmed, hover-only).
pub(crate) fn labeled_chip(
    ui: &mut egui::Ui,
    id: egui::Id,
    tooltip: &str,
    selected: bool,
    label: &str,
    glyph: Glyph,
    enabled: bool,
) -> egui::Response {
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_owned(), FontId::proportional(13.0), theme::CHALK);
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
        theme::EMBER
    } else if enabled && resp.hovered() {
        theme::GRAPHITE_500
    } else {
        theme::GRAPHITE_700
    };
    let border = if selected || !enabled {
        Stroke::NONE
    } else {
        Stroke::new(1.0, theme::GRAPHITE_500)
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(6),
        fill,
        border,
        StrokeKind::Middle,
    );
    let color = if !enabled {
        theme::SLATE.gamma_multiply(0.5)
    } else if selected {
        theme::INK
    } else if resp.hovered() {
        theme::CHALK
    } else {
        theme::SLATE
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

/// The ember Copy primary: wider chip with a glyph + label. The
/// glyph morphs to a checkmark once a copy has succeeded this
/// session, pairing with the "Copied" label. Shared with the
/// overlay's accept row.
pub(crate) fn copy_chip(ui: &mut egui::Ui, copied: bool) -> egui::Response {
    let label = if copied { "Copied" } else { "Copy" };
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_owned(), FontId::proportional(13.0), theme::INK);
    let galley_size = galley.size();
    let width = 10.0 + 18.0 + 5.0 + galley_size.x + 10.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 28.0), Sense::hover());
    let resp = ui
        .interact(rect, egui::Id::new("copy_action"), Sense::click())
        .on_hover_text("Copy · Ctrl+C");
    ui.painter().rect(
        rect,
        CornerRadius::same(6),
        theme::EMBER,
        Stroke::NONE,
        StrokeKind::Middle,
    );
    let glyph_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 9.0, rect.top()),
        egui::vec2(18.0, 28.0),
    );
    let glyph = if copied { g_check } else { g_copy };
    glyph(ui.painter(), glyph_rect, theme::INK);
    let pos = egui::pos2(
        glyph_rect.right() + 5.0,
        rect.center().y - galley_size.y / 2.0,
    );
    ui.painter().galley(pos, galley, theme::INK);
    resp
}

/// The seven color presets plus a custom-color chip whose popup
/// hosts egui's full sRGB editor.
fn swatch_row(ui: &mut egui::Ui, color: &mut Color) {
    for sw in theme::SWATCHES {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), Sense::hover());
        let selected = to_color32(*color) == sw;
        let resp = ui.interact(
            rect,
            egui::Id::new(("swatch", sw.r(), sw.g(), sw.b())),
            Sense::click(),
        );
        let ring = if selected {
            Stroke::new(2.0, theme::EMBER)
        } else {
            Stroke::new(1.0, theme::GRAPHITE_500)
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
        theme::GRAPHITE_700,
        Stroke::new(1.0, theme::GRAPHITE_500),
        StrokeKind::Middle,
    );
    g_plus(ui.painter(), rect, theme::CHALK);
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
            theme::GRAPHITE_500
        } else {
            theme::GRAPHITE_700
        };
        ui.painter().rect(
            rect,
            CornerRadius::same(5),
            fill,
            Stroke::new(1.0, theme::GRAPHITE_500),
            StrokeKind::Middle,
        );
        let dot = if selected { theme::EMBER } else { theme::CHALK };
        ui.painter()
            .circle(rect.center(), preset / 2.0 + 1.2, dot, Stroke::NONE);
        if resp.clicked() {
            state.stroke_width = *preset;
        }
    }
}

fn to_color32(c: Color) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
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
        FontId::proportional(15.0),
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
        FontId::proportional(10.0),
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

/// Tilted pushpin: filled head + needle.
pub(crate) fn g_pin(p: &egui::Painter, r: egui::Rect, c: Color32) {
    p.circle(pt(r, 4.0, -4.0), 3.0, c, Stroke::NONE);
    seg(p, r, (1.8, -1.8), (-6.0, 6.0), c, 2.0);
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

fn g_plus(p: &egui::Painter, r: egui::Rect, c: Color32) {
    seg(p, r, (-4.0, 0.0), (4.0, 0.0), c, 2.0);
    seg(p, r, (0.0, -4.0), (0.0, 4.0), c, 2.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_cover_every_variant_with_unique_keys() {
        let flat: Vec<&ToolSpec> = flat_tools().collect();
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
        let flat: Vec<&ToolSpec> = flat_tools().collect();
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
        for spec in flat_tools() {
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

    /// A minimal state whose tool the rail can switch.
    fn rail_state() -> EditorState {
        let img = image::RgbaImage::from_raw(64, 48, vec![255; 64 * 48 * 4])
            .expect("raw buffer matches 64*48*4");
        EditorState::new(img)
    }

    /// Drive the rail through a synthetic context: pass 1 lays out
    /// (registering chip rects by id), pass 2 injects a primary
    /// click at the Pencil chip's centre. Returns the tool after.
    fn tool_after_pen_click(gate: ToolGate) -> Tool {
        let mut state = rail_state();
        state.tool = Tool::Select;
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = show(ui, &mut state, false, gate);
        });
        let pen = ctx
            .read_response(egui::Id::new(("tool", "P")))
            .expect("pass 1 registered the Pencil chip");
        let at = pen.rect.center();
        let click = egui::RawInput {
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
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(click, |ui| {
            let _ = show(ui, &mut state, false, gate);
        });
        state.tool
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

    // --- Delete chip (annotation-delete Task 002) ---

    /// State with one committed rect annotation kept selected (the
    /// persistent-selection contract task 001 added: a Move-tool
    /// click leaves `selected_index` set past release).
    fn selected_state() -> EditorState {
        let mut state = rail_state();
        state.pending = Some(flintshot_core::Annotation::Rect {
            rect: flintshot_core::Rect::from_xywh(10.0, 10.0, 40.0, 30.0),
            stroke: flintshot_core::Stroke::new(flintshot_core::Color::RED, 3.0),
            fill: None,
        });
        state.commit_annotation();
        state.selected_index = Some(0);
        state
    }

    /// Drive the rail through a synthetic context and click the
    /// Delete chip's centre — the `tool_after_pen_click` pattern
    /// pointed at the new chip. Returns the collected action.
    fn action_after_delete_click(state: &mut EditorState) -> ToolbarAction {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = show(ui, state, false, ToolGate::All);
        });
        let del = ctx
            .read_response(egui::Id::new("delete"))
            .expect("pass 1 registered the Delete chip");
        let at = del.rect.center();
        let click = egui::RawInput {
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
            ..egui::RawInput::default()
        };
        let mut action = ToolbarAction::None;
        let _ = ctx.run_ui(click, |ui| {
            action = show(ui, state, false, ToolGate::All);
        });
        action
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
        let mut state = rail_state();
        assert_eq!(
            action_after_delete_click(&mut state),
            ToolbarAction::None,
            "no selection: the chip must render greyed / hover-only and stay inert"
        );
    }

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
}
