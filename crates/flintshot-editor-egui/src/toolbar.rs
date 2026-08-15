//! The editor toolbar: a two-row instrument panel.
//!
//! **Rail** (top, stable across tools): the ten tool chips,
//! undo / redo, and the output actions — Save, Pin, Upload, and the
//! ember Copy primary. **Bay** (bottom, per-tool): only the controls
//! the active tool can use, plus a contextual hint on the right.
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
    BlurKind, Color, EditorState, FontFamily, FontStyle, FontWeight, TextAlign, TextOutline, Tool,
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
}

/// Paint a glyph inside the chip rect. Coordinates are glyph-space:
/// `(0, 0)` is the rect centre, `±7` the edges, +Y is down — matching
/// egui's screen space so glyphs can be reasoned about like the
/// strokes they represent.
type Glyph = fn(&egui::Painter, egui::Rect, Color32);

/// One rail tool chip: variant, display name, hotkey hint, glyph.
struct ToolSpec {
    /// Tool selected on click.
    tool: Tool,
    /// Human name (tooltip).
    name: &'static str,
    /// Single-key shortcut (tooltip + doc).
    key: &'static str,
    /// Painter glyph.
    glyph: Glyph,
}

const TOOLS: &[ToolSpec] = &[
    ToolSpec {
        tool: Tool::Select,
        name: "Select",
        key: "S",
        glyph: g_select,
    },
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
        tool: Tool::Arrow,
        name: "Arrow",
        key: "A",
        glyph: g_arrow,
    },
    ToolSpec {
        tool: Tool::Line,
        name: "Line",
        key: "L",
        glyph: g_line,
    },
    ToolSpec {
        tool: Tool::Marker,
        name: "Marker",
        key: "M",
        glyph: g_marker,
    },
    ToolSpec {
        tool: Tool::Pencil,
        name: "Pencil",
        key: "P",
        glyph: g_pencil,
    },
    ToolSpec {
        tool: Tool::Text,
        name: "Text",
        key: "T",
        glyph: g_text,
    },
    ToolSpec {
        tool: Tool::Blur,
        name: "Blur",
        key: "B",
        glyph: g_blur,
    },
    ToolSpec {
        tool: Tool::Number,
        name: "Number",
        key: "N",
        glyph: g_number,
    },
];

/// Draw the whole toolbar (rail + bay) as the window's top panel.
///
/// The horizontal scroll on the rail is the narrow-capture fallback:
/// region captures can be far narrower than the ten chips + actions,
/// and a clipped toolbar hides actions; a scrollbar-less scroll keeps
/// every control reachable via wheel / drag.
pub(crate) fn show(ui: &mut egui::Ui, state: &mut EditorState, copied: bool) -> ToolbarAction {
    let frame = egui::Frame::new()
        .fill(theme::GRAPHITE_900)
        .inner_margin(egui::Margin::symmetric(8, 5));
    egui::Panel::top("editor_toolbar")
        .frame(frame)
        .show(ui, |ui| {
            let action = draw_rail(ui, state, copied);
            draw_bay(ui, state);
            action
        })
        .inner
}

// ------------------------------------------------------------------
// Rail
// ------------------------------------------------------------------

fn draw_rail(ui: &mut egui::Ui, state: &mut EditorState, copied: bool) -> ToolbarAction {
    egui::ScrollArea::horizontal()
        .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for spec in TOOLS {
                    let selected = state.tool == spec.tool;
                    let tooltip = format!("{} · {}", spec.name, spec.key);
                    let resp = chip(
                        ui,
                        egui::Id::new(("tool", spec.key)),
                        tooltip.as_str(),
                        selected,
                        spec.glyph,
                    );
                    if resp.clicked() {
                        state.tool = spec.tool;
                    }
                }
                ui.separator();
                let mut action = ToolbarAction::None;
                if chip(ui, egui::Id::new("undo"), "Undo · Ctrl+Z", false, g_undo).clicked() {
                    action = ToolbarAction::Undo;
                }
                if chip(ui, egui::Id::new("redo"), "Redo · Ctrl+Y", false, g_redo).clicked() {
                    action = ToolbarAction::Redo;
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

/// Output actions, right-aligned: Save / Pin / Upload as quiet icon
/// chips, Copy as the ember primary (the dominant terminal action
/// for screenshot flows). On a successful copy the glyph morphs
/// sheet→check and the label reads "Copied".
fn rail_actions(ui: &mut egui::Ui, copied: bool) -> ToolbarAction {
    let mut action = ToolbarAction::None;
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if copy_chip(ui, copied).clicked() {
            action = ToolbarAction::Copy;
        }
        if chip(ui, egui::Id::new("upload"), "Upload", false, g_upload).clicked() {
            action = ToolbarAction::Upload;
        }
        if chip(ui, egui::Id::new("pin"), "Pin to desktop", false, g_pin).clicked() {
            action = ToolbarAction::Pin;
        }
        if chip(ui, egui::Id::new("save"), "Save · Ctrl+S", false, g_save).clicked() {
            action = ToolbarAction::Save;
        }
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
/// the Copy primary.
fn chip(
    ui: &mut egui::Ui,
    id: egui::Id,
    tooltip: &str,
    selected: bool,
    glyph: Glyph,
) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), Sense::hover());
    let resp = ui.interact(rect, id, Sense::click()).on_hover_text(tooltip);
    let fill = if selected {
        theme::EMBER
    } else if resp.hovered() {
        theme::GRAPHITE_500
    } else {
        theme::GRAPHITE_700
    };
    let border = if selected {
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
    let color = if selected {
        theme::INK
    } else if resp.hovered() {
        theme::CHALK
    } else {
        theme::SLATE
    };
    glyph(ui.painter(), rect, color);
    resp
}

/// The ember Copy primary: wider chip with a glyph + label. The
/// glyph morphs to a checkmark once a copy has succeeded this
/// session, pairing with the "Copied" label.
fn copy_chip(ui: &mut egui::Ui, copied: bool) -> egui::Response {
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
fn g_undo(p: &egui::Painter, r: egui::Rect, c: Color32) {
    arc_over_top(p, r, c);
    arrowhead(p, pt(r, -5.5, 0.0), Vec2::new(-0.35, 0.94), c);
}

/// Mirror of undo: barb at the right end.
fn g_redo(p: &egui::Painter, r: egui::Rect, c: Color32) {
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

/// Arrow descending into a tray (download-to-disk).
fn g_save(p: &egui::Painter, r: egui::Rect, c: Color32) {
    seg(p, r, (-6.0, 6.0), (6.0, 6.0), c, 2.0);
    seg(p, r, (-6.0, 6.0), (-6.0, 2.0), c, 2.0);
    seg(p, r, (6.0, 6.0), (6.0, 2.0), c, 2.0);
    seg(p, r, (0.0, -7.0), (0.0, 1.0), c, 2.0);
    arrowhead(p, pt(r, 0.0, 3.0), Vec2::new(0.0, 1.0), c);
}

/// Arrow rising out of a tray.
fn g_upload(p: &egui::Painter, r: egui::Rect, c: Color32) {
    seg(p, r, (-6.0, -6.0), (6.0, -6.0), c, 2.0);
    seg(p, r, (-6.0, -6.0), (-6.0, -2.0), c, 2.0);
    seg(p, r, (6.0, -6.0), (6.0, -2.0), c, 2.0);
    seg(p, r, (0.0, 7.0), (0.0, -1.0), c, 2.0);
    arrowhead(p, pt(r, 0.0, -3.0), Vec2::new(0.0, -1.0), c);
}

/// Tilted pushpin: filled head + needle.
fn g_pin(p: &egui::Painter, r: egui::Rect, c: Color32) {
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

/// Bold check for the post-copy state.
fn g_check(p: &egui::Painter, r: egui::Rect, c: Color32) {
    p.add(egui::Shape::line(
        vec![pt(r, -6.0, -1.0), pt(r, -2.0, 3.0), pt(r, 7.0, -6.0)],
        Stroke::new(2.5, c),
    ));
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
        assert_eq!(TOOLS.len(), 10);
        let mut keys: Vec<&str> = TOOLS.iter().map(|t| t.key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), TOOLS.len());
        for tool in [
            Tool::Select,
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
                TOOLS.iter().any(|t| t.tool == tool),
                "missing rail chip for {tool:?}"
            );
        }
    }

    #[test]
    fn every_tool_has_a_nonempty_hint() {
        for spec in TOOLS {
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
}
