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

use flintshot_core::{EditorState, Tool};

use egui::{Color32, CornerRadius, FontId, Pos2, Sense, Stroke, StrokeKind, Vec2};
use flintshot_core::{BlurKind, Color, FontFamily, FontStyle, FontWeight, TextAlign, TextOutline};

use crate::popover;
use crate::rail::{arrowhead, pt, seg, Glyph};
use crate::theme;
use crate::widgets;

/// Docked panel width, logical px. Wide enough for the widest bay
/// row (the swatch row) plus card dressing; leaves ≥ ~700 px of
/// canvas at the 1024×640 editor page default.
pub(crate) const PANEL_WIDTH: f32 = 236.0;

/// Grain seed — any fixed value; determinism is the contract.
const GRAIN_SEED: u64 = 0x5EED_C0DE;

/// Grain tile edge, texels. 64² repeats unnoticeably at panel
/// scale.
const GRAIN_TILE: u32 = 64;

/// Panel state that persists across frames: the collapse flag and
/// the (lazily created) grain texture handle.
#[derive(Default)]
pub(crate) struct PanelState {
    /// The grain tile, created on first use.
    grain: Option<egui::TextureHandle>,
}

/// The properties panel's contextual sections (spec `SEC_MAP`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Section {
    /// Swatch row + stroke-width wells.
    Stroke,
    /// The fill-shapes switch.
    Fill,
    /// Text size / weight / style / family + the text color pod.
    Text,
    /// The next-number stepper.
    Number,
    /// Blur kind wells + strength.
    Effects,
    /// The Select tool's crop bay.
    Crop,
    /// Danger Delete — always present.
    Selection,
}

/// The binding per-tool section map (spec §Per-tool section map).
/// Pure for tests; the panel renders exactly these, in order.
#[expect(
    clippy::match_same_arms,
    reason = "the wildcard mirrors the Line/Arrow/Marker base kit by design"
)]
pub(crate) fn sections_for(tool: Tool) -> &'static [Section] {
    match tool {
        Tool::Move => &[Section::Selection],
        Tool::Rect | Tool::Ellipse | Tool::Pencil => {
            &[Section::Stroke, Section::Fill, Section::Selection]
        }
        Tool::Line | Tool::Arrow | Tool::Marker => &[Section::Stroke, Section::Selection],
        Tool::Text => &[Section::Stroke, Section::Text, Section::Selection],
        Tool::Number => &[Section::Stroke, Section::Number, Section::Selection],
        Tool::Blur => &[Section::Effects, Section::Selection],
        Tool::Select => &[Section::Crop, Section::Selection],
        // Future variants (the enum is `#[non_exhaustive]`) get the
        // same base kit as Line / Arrow / Marker.
        _ => &[Section::Stroke, Section::Selection],
    }
}

/// The section card's title.
fn section_title(section: Section) -> &'static str {
    match section {
        Section::Stroke => "Stroke",
        Section::Fill => "Fill",
        Section::Text => "Text",
        Section::Number => "Number",
        Section::Effects => "Effects",
        Section::Crop => "Crop",
        Section::Selection => "Selection",
    }
}

/// The tool display name for the panel title (from the rail's
/// vocabulary; the wildcard keeps future variants rendering).
fn tool_display_name(tool: Tool) -> &'static str {
    for pod in crate::rail::TOOL_PODS {
        for spec in pod.tools {
            if spec.tool == tool {
                return spec.name;
            }
        }
    }
    "Editor"
}

/// The Selection section's danger Delete: berry-filled with cream
/// text, disabled (dim, hover-only) without a selection.
fn delete_button(ui: &mut egui::Ui, enabled: bool) -> bool {
    use crate::theme::palette as p;
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), egui::Sense::hover());
    let resp = ui
        .scope(|ui| {
            if !enabled {
                ui.disable();
            }
            ui.interact(rect, egui::Id::new("panel-delete"), egui::Sense::click())
        })
        .inner
        .on_hover_text("Delete the selected annotation · Del");
    let (fill, ink) = if !enabled {
        (
            p::with_alpha(p::BERRY_RED, 60),
            p::with_alpha(p::CREAM_TEXT, 120),
        )
    } else if resp.hovered() {
        (p::BERRY_RED.gamma_multiply(0.8), p::CREAM_TEXT)
    } else {
        (p::BERRY_RED, p::CREAM_TEXT)
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(theme::POD_RADIUS),
        fill,
        egui::Stroke::NONE,
        egui::StrokeKind::Middle,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "🗑 Delete annotation",
        egui::FontId::proportional(theme::TYPE_SM),
        ink,
    );
    enabled && resp.clicked()
}

/// The Number section's next-number stepper.
fn number_row(ui: &mut egui::Ui, state: &mut EditorState) {
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
}

/// Draw the properties panel: the `{Tool} · Properties` title and
/// exactly the binding section map's collapsible cards. Returns the
/// fired history action (the Selection section's Delete).
pub(crate) fn show(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    panel: &mut PanelState,
) -> ToolbarAction {
    let frame = egui::Frame::new()
        .fill(theme::palette::PAPER)
        .inner_margin(egui::Margin::same(8));
    let mut action = ToolbarAction::None;
    egui::Panel::right("editor_panel")
        .resizable(false)
        .exact_size(PANEL_WIDTH)
        .frame(frame)
        .show(ui, |ui| {
            paint_grain(ui, panel);
            let title = format!("{} · Properties", tool_display_name(state.tool));
            let galley = ui.painter().layout_no_wrap(
                title.to_uppercase(),
                egui::FontId::proportional(theme::TYPE_SM),
                theme::palette::INK,
            );
            let (rect, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
            ui.painter().galley(rect.min, galley, theme::palette::INK);
            ui.add_space(theme::SPACE_SCALE[1]);
            for section in sections_for(state.tool) {
                let id = egui::Id::new(("panel-section", section_title(*section)));
                let title = section_title(*section);
                widgets::section(ui, id, title, true, |body| match section {
                    Section::Stroke => {
                        swatch_row(body, &mut state.color);
                        stroke_chips(body, state);
                    }
                    Section::Fill => {
                        let resp = widgets::switch(
                            body,
                            egui::Id::new("fill-switch"),
                            state.fill_shapes,
                            false,
                        );
                        state.fill_shapes = resp.on;
                    }
                    Section::Text => {
                        text_style_row(body, state);
                        popover::pod_button(body, state, popover::ColorTarget::Text);
                    }
                    Section::Number => {
                        number_row(body, state);
                    }
                    Section::Effects => {
                        blur_bay(body, state);
                    }
                    Section::Crop => {
                        select_bay(body, state);
                        hint(body, tool_hint(state.tool));
                    }
                    Section::Selection => {
                        if delete_button(body, state.selected_index.is_some()) {
                            action = ToolbarAction::Delete;
                        }
                    }
                });
            }
        });
    action
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

/// What a panel control fired this frame. The panel itself only
/// ever constructs `None` / `Delete` today (Save / Copy / Upload
/// live on the topbar; the overlay's accept row still constructs
/// its own set — out of redesign scope).
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
}

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

/// The swatch row + the custom-color chip, flowing left-to-right and
/// wrapping when the body narrows (the section body itself is a
/// justified top-down column — without this wrapper every swatch
/// stacks on its own row).
pub(crate) fn swatch_row(ui: &mut egui::Ui, color: &mut Color) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
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
    });
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
/// communicates the effect faster than numbers. Flows left-to-right
/// (same wrapper rationale as [`swatch_row`]).
fn stroke_chips(ui: &mut egui::Ui, state: &mut EditorState) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
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
    });
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

#[cfg(test)]
mod tests {
    use super::*;
    use flintshot_core::{Annotation, Color, Rect as CoreRect, Stroke as CoreStroke};

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
        events: Vec<egui::Event>,
    ) -> ToolbarAction {
        let raw = egui::RawInput {
            events,
            ..screen(1024.0, 768.0)
        };
        let mut fired = ToolbarAction::None;
        let _ = ctx.run_ui(raw, |ui| {
            fired = show(ui, state, panel);
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

    /// The Stroke section's swatches flow left-to-right in a row (the
    /// section body is a justified top-down column — without the
    /// wrapper every swatch stacks on its own row, the "stroke
    /// section is a column" regression).
    #[test]
    fn stroke_section_swatches_flow_in_a_row() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut state = panel_state();
        state.tool = Tool::Rect;
        let mut panel = PanelState::default();
        let _ = draw(&ctx, &mut state, &mut panel, vec![]);
        let ids: Vec<egui::Id> = theme::palette::SWATCHES
            .iter()
            .take(3)
            .map(|sw| egui::Id::new(("swatch", sw.r(), sw.g(), sw.b())))
            .collect();
        let rects: Vec<egui::Rect> = ids
            .iter()
            .map(|id| {
                ctx.read_response(*id)
                    .unwrap_or_else(|| panic!("swatch {id:?} registers"))
                    .rect
            })
            .collect();
        let same_row = rects
            .windows(2)
            .all(|w| (w[0].min.y - w[1].min.y).abs() < 1.0 && w[1].min.x > w[0].max.x);
        assert!(same_row, "swatches share a row left-to-right: {rects:?}");
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
        let _ = draw(&ctx, state, &mut panel, vec![]);
        let del = ctx
            .read_response(egui::Id::new("panel-delete"))
            .expect("pass 1 registered the Selection section's Delete button");
        let at = del.rect.center();
        draw(&ctx, state, &mut panel, click_events(at))
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

    #[test]
    fn panel_has_no_pin_chip() {
        let mut state = panel_state();
        let ctx = egui::Context::default();
        let mut panel = PanelState::default();
        let _ = draw(&ctx, &mut state, &mut panel, vec![]);
        assert!(
            ctx.read_response(egui::Id::new("pin")).is_none(),
            "the pin chip must not register a response"
        );
    }

    // --- Panel geometry (the structural pivot's own contracts) ---
}
