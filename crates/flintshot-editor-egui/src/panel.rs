//! The docked side panel — the editor's control home.
//!
//! Right-docked, grain-textured cream paper, with a
//! `PROPERTIES | HISTORY` tab header as its first row (segmented
//! chips; the Properties tab is today's panel body, the History tab
//! the labeled edit timeline — click any row to time-travel via
//! `restore_to`). The panel is **collapsible** to
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

use flintshot_core::{Annotation, EditorState, Tool};

use egui::{Color32, CornerRadius, FontId, Pos2, Sense, Stroke, StrokeKind, Vec2};
use flintshot_core::{BlurKind, Color, FontFamily, FontStyle, FontWeight, TextAlign, TextOutline};
use flintshot_core::{NUMBER_MAX_DIAMETER, NUMBER_MIN_DIAMETER};

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

/// Which panel tab is active. Per-window state living in
/// [`PanelState`] like the other live flags — deliberately not
/// persisted to disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) enum PanelTab {
    /// Tool defaults / selection inspector (today's panel body).
    #[default]
    Properties,
    /// The edit-history timeline.
    History,
}

impl PanelTab {
    /// The header chip's uppercase label.
    fn label(self) -> &'static str {
        match self {
            Self::Properties => "PROPERTIES",
            Self::History => "HISTORY",
        }
    }
}

/// Panel state that persists across frames: the active tab, the
/// collapse flag, and the (lazily created) grain texture handle.
#[derive(Default)]
pub(crate) struct PanelState {
    /// The active tab (Properties by default).
    tab: PanelTab,
    /// The grain tile, created on first use.
    grain: Option<egui::TextureHandle>,
    /// Whether the selection inspector's custom-colour popover ran
    /// this frame — drives `end_property_edit` bracketing when the
    /// popover closes (a continuous widget's burst must end when
    /// its popover does).
    color_pop_live: bool,
    /// Same bracketing flag for the text-style popover.
    text_pop_live: bool,
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
    /// The Crop tool's region bay.
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
        // Hand is pan-only: no sections (it never selects, so the
        // Selection inspector would render a dead empty state).
        Tool::Hand => &[],
        Tool::Select => &[Section::Selection],
        Tool::Crop => &[Section::Crop],
        Tool::Rect | Tool::Ellipse | Tool::Pencil => {
            &[Section::Stroke, Section::Fill, Section::Selection]
        }
        Tool::Line | Tool::Arrow | Tool::Marker => &[Section::Stroke, Section::Selection],
        Tool::Text => &[Section::Stroke, Section::Text, Section::Selection],
        Tool::Number => &[Section::Stroke, Section::Number, Section::Selection],
        Tool::Blur => &[Section::Effects, Section::Selection],
        // Future variants (the enum is `#[non_exhaustive]`) get the
        // same base kit as Line / Arrow / Marker.
        _ => &[Section::Stroke, Section::Selection],
    }
}

/// The panel title for this frame: with a live Hand-tool selection
/// the panel is a selection inspector (`N selected · Properties`);
/// without, byte-for-byte the tool-defaults title (the Text tool's
/// singleton selection keeps the tool-defaults layout). Pure for
/// tests.
pub(crate) fn panel_title(state: &EditorState) -> String {
    if inspector_mode(state) {
        format!("{} selected · Properties", state.selected.len())
    } else {
        format!("{} · Properties", tool_display_name(state.tool))
    }
}

/// Whether the panel renders as the Select-tool selection inspector.
/// The Text tool's singleton selection is a different contract — the
/// tool-defaults panel stays up and the Text surfaces retarget to
/// the selection in place (see [`effective_text_style`]); the
/// inspector (intersection sections, `set_selected_*` mutators) is
/// the Select tool's multi-selection surface alone.
fn inspector_mode(state: &EditorState) -> bool {
    state.tool == Tool::Select && !state.selected.is_empty()
}

/// The inspector's section map: the intersection sections the
/// selection supports (via [`EditorState::selected_capabilities`]),
/// in panel order, plus the always-present Selection section. Pure
/// for tests.
pub(crate) fn selection_sections(caps: flintshot_core::SelectionCapabilities) -> Vec<Section> {
    let mut sections = Vec::new();
    if caps.stroke_color {
        sections.push(Section::Stroke);
    }
    if caps.fill {
        sections.push(Section::Fill);
    }
    if caps.text_style {
        sections.push(Section::Text);
    }
    if caps.effects {
        sections.push(Section::Effects);
    }
    if caps.number_size {
        sections.push(Section::Number);
    }
    sections.push(Section::Selection);
    sections
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
/// text, disabled (dim, hover-only) without a selection. The label
/// pluralizes in inspector mode.
fn delete_button(ui: &mut egui::Ui, enabled: bool, label: &str) -> bool {
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
        label,
        egui::FontId::proportional(theme::TYPE_SM),
        ink,
    );
    enabled && resp.clicked()
}

/// The Number section's rows: the next-number stepper and the
/// bubble-size stepper. Both are plain session tool-state edits —
/// no history entries, nothing observable on already-committed
/// bubbles until the next placement.
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
    // The range comes from core so the clamp bound lives beside the
    // default it constrains, not as a duplicated panel literal.
    // egui's DragValue clamps user edits AND stale out-of-range
    // values to the range on render (clamp_existing_to_range
    // defaults on) — the panel tests pin that through the bound field.
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Size")
                .size(theme::TYPE_SM)
                .color(theme::palette::INK_SOFT),
        );
        ui.add(
            egui::DragValue::new(&mut state.number_diameter)
                .range(NUMBER_MIN_DIAMETER..=NUMBER_MAX_DIAMETER)
                .speed(1.0),
        );
    });
}

/// Draw the properties panel. Two modes (multi-select-properties
/// spec): with a live selection the panel is a **selection
/// inspector** — title `N selected · Properties`, the
/// intersection-of-properties sections, edits routed through the
/// core mutators (undoable, defaults-syncing). Without a selection
/// it is byte-for-byte today's tool-defaults panel: the `{Tool} ·
/// Properties` title and exactly `sections_for(tool)`.
///
/// Returns the fired history action (the Selection section's
/// Delete).
pub(crate) fn show(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    panel: &mut PanelState,
) -> ToolbarAction {
    let frame = egui::Frame::new()
        .fill(theme::palette::PAPER)
        .inner_margin(egui::Margin::same(8));
    // End-of-burst bracketing inputs: which continuous popovers ran
    // last frame vs. this frame. A popover that ran last frame but
    // not this one just closed — close its edit burst too.
    let color_pop_was_live = panel.color_pop_live;
    let text_pop_was_live = panel.text_pop_live;
    panel.color_pop_live = false;
    panel.text_pop_live = false;
    let inspector = inspector_mode(state);
    let caps = state.selected_capabilities();
    let mut action = ToolbarAction::None;
    egui::Panel::right("editor_panel")
        .resizable(false)
        .exact_size(PANEL_WIDTH)
        .frame(frame)
        .show(ui, |ui| {
            paint_grain(ui, panel);
            tab_header(ui, panel);
            match panel.tab {
                PanelTab::Properties => {
                    action = properties_body(ui, state, panel, inspector, caps);
                }
                PanelTab::History => history_body(ui, state),
            }
        });
    if color_pop_was_live && !panel.color_pop_live {
        state.end_property_edit();
    }
    if text_pop_was_live && !panel.text_pop_live {
        state.end_property_edit();
    }
    action
}

/// The PROPERTIES | HISTORY tab header: two segmented chips in the
/// panel's first row (cutout at rest, ink-filled when active — the
/// blur-kind control's idiom).
fn tab_header(ui: &mut egui::Ui, panel: &mut PanelState) {
    ui.horizontal(|ui| {
        for tab in [PanelTab::Properties, PanelTab::History] {
            if segmented_chip(
                ui,
                egui::Id::new(("panel-tab", tab)),
                tab.label(),
                panel.tab == tab,
            ) {
                panel.tab = tab;
            }
        }
    });
    ui.add_space(theme::SPACE_SCALE[1]);
}

/// The History tab's body: the edit timeline. Past rows (`undo`,
/// oldest first) click to jump back; the current-state marker row
/// sits at the boundary; dimmed future rows (`redo`, nearest first)
/// click to jump forward. One `restore_to` per click — the only
/// mutation path, so the tab can never diverge from chord-stepping.
fn history_body(ui: &mut egui::Ui, state: &mut EditorState) {
    if state.undo.is_empty() && state.redo.is_empty() {
        hint(ui, "No edits yet");
        return;
    }
    // Labels collected up front: the interaction loop must not hold
    // a borrow over the `restore_to` call it feeds.
    let past: Vec<String> = state.undo.iter().map(|s| s.kind.to_string()).collect();
    let future: Vec<String> = state
        .redo
        .iter()
        .rev()
        .map(|s| s.kind.to_string())
        .collect();
    let mut jump: Option<usize> = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, label) in past.iter().enumerate() {
            if history_row(ui, egui::Id::new(("history-row", i)), label, false) {
                jump = Some(i + 1);
            }
        }
        current_marker(ui);
        for (j, label) in future.iter().enumerate() {
            if history_row(ui, egui::Id::new(("history-future", j)), label, true) {
                jump = Some(past.len() + j + 1);
            }
        }
    });
    if let Some(target) = jump {
        state.restore_to(target);
    }
}

/// One timeline row. Future rows render dimmed (half-alpha ink) but
/// stay clickable. Returns whether the row was clicked.
fn history_row(ui: &mut egui::Ui, id: egui::Id, label: &str, future: bool) -> bool {
    let ink = if future {
        theme::palette::INK_SOFT.gamma_multiply(0.5)
    } else {
        theme::palette::INK
    };
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::proportional(theme::TYPE_SM),
        ink,
    );
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 22.0), Sense::hover());
    let resp = ui.interact(rect, id, Sense::click());
    if resp.hovered() {
        ui.painter().rect(
            rect,
            CornerRadius::same(theme::POD_RADIUS),
            theme::palette::PAPER_DIM,
            Stroke::NONE,
            StrokeKind::Middle,
        );
    }
    let pos = egui::pos2(rect.left() + 2.0, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(pos, galley, ink);
    resp.clicked()
}

/// The current-state marker row: a selection-ink dot + "Current",
/// not clickable.
fn current_marker(ui: &mut egui::Ui) {
    let galley = ui.painter().layout_no_wrap(
        "Current".to_owned(),
        egui::FontId::proportional(theme::TYPE_SM),
        theme::palette::INK_SOFT,
    );
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 22.0), Sense::hover());
    ui.interact(rect, egui::Id::new("history-current"), Sense::hover());
    ui.painter().circle(
        egui::pos2(rect.left() + 6.0, rect.center().y),
        3.0,
        theme::palette::FOREST_DEEP,
        Stroke::NONE,
    );
    let pos = egui::pos2(rect.left() + 14.0, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(pos, galley, theme::palette::INK_SOFT);
}

/// The Properties tab's body: the contextual title plus exactly the
/// section map. Byte-for-byte today's panel content beneath the
/// header.
fn properties_body(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    panel: &mut PanelState,
    inspector: bool,
    caps: flintshot_core::SelectionCapabilities,
) -> ToolbarAction {
    let title = panel_title(state);
    let galley = ui.painter().layout_no_wrap(
        title.to_uppercase(),
        egui::FontId::proportional(theme::TYPE_SM),
        theme::palette::INK,
    );
    let (rect, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
    ui.painter().galley(rect.min, galley, theme::palette::INK);
    ui.add_space(theme::SPACE_SCALE[1]);
    let mut action = ToolbarAction::None;
    let sections: Vec<Section> = if inspector {
        selection_sections(caps)
    } else {
        sections_for(state.tool).to_vec()
    };
    for section in sections {
        let id = egui::Id::new(("panel-section", section_title(section)));
        let title = section_title(section);
        widgets::section(ui, id, title, true, |body| match section {
            Section::Stroke if inspector => selection_stroke(body, state, caps, panel),
            Section::Stroke => {
                swatch_row(body, &mut state.color);
                stroke_chips(body, state);
            }
            Section::Fill if inspector => selection_fill(body, state),
            Section::Fill => {
                let resp =
                    widgets::switch(body, egui::Id::new("fill-switch"), state.fill_shapes, false);
                state.fill_shapes = resp.on;
            }
            Section::Text if inspector => selection_text(body, state, panel),
            Section::Text => {
                text_style_row(body, state);
                popover::pod_button(body, state, popover::ColorTarget::Text);
            }
            Section::Number if inspector => selection_number(body, state),
            Section::Number => {
                number_row(body, state);
            }
            Section::Effects if inspector => selection_effects(body, state),
            Section::Effects => {
                blur_bay(body, state);
            }
            Section::Crop => {
                select_bay(body, state);
                hint(body, tool_hint(state.tool));
            }
            Section::Selection => {
                let clicked = if inspector {
                    selection_selection_row(body, state)
                } else {
                    delete_button(body, !state.selected.is_empty(), "🗑 Delete annotation")
                };
                if clicked {
                    action = ToolbarAction::Delete;
                }
            }
        });
    }
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
    // Source order (text-select-edit): re-edit session draft >
    // Text-tool selection > session defaults. Controls mutate a local
    // copy; the write-back routes through core's gesture API when a
    // draft / selection target exists, else the plain defaults writes
    // of today.
    let mut st = effective_text_style(state);
    let before = st;
    ui.horizontal(|ui| {
        if segmented_chip(ui, egui::Id::new("text-size-minus"), "−", false) {
            st.size = (st.size - 1.0).max(8.0);
        }
        ui.add_sized(
            [36.0, 20.0],
            egui::DragValue::new(&mut st.size)
                .range(8.0..=72.0)
                .speed(0.5),
        );
        if segmented_chip(ui, egui::Id::new("text-size-plus"), "+", false) {
            st.size = (st.size + 1.0).min(72.0);
        }
    });
    // Gesture bracketing: a discrete click (or typed value) opens and
    // closes within the frame — one entry; a `DragValue` drag stays
    // open across drag frames (`begin` is first-wins) and closes on
    // the first frame the primary button is up again. The button
    // state (not this render's `dragged()`) is the close signal —
    // the section body renders twice per frame (measure probe +
    // visible) and only one of the two `DragValue` instances owns the
    // drag interaction.
    let pointer_down = ui.input(|i| i.pointer.primary_down());
    if st != before {
        state.begin_style_gesture();
        write_text_style(state, &st);
    }
    if state.style_gesture_snapshot.is_some() && !pointer_down {
        state.end_style_gesture();
    }
    let btn = ui.add(egui::Button::new(egui::RichText::new("Aa").strong()));
    egui::Popup::from_toggle_button_response(&btn)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| text_style_popup(ui, state));
}

/// The Text tool's singleton selected `Text`, if any — the
/// tool-defaults Text surfaces' restyle target. A Hand-tool
/// selection (singleton or not) is the inspector's domain, never
/// this path's; a non-Text member is a miss for the Text tool.
fn text_selection(state: &EditorState) -> Option<&Annotation> {
    if state.tool != Tool::Text {
        return None;
    }
    match state.selected.as_slice() {
        [i] => state
            .anns
            .get(*i)
            .filter(|ann| matches!(ann, Annotation::Text { .. })),
        _ => None,
    }
}

/// Whether a restyle target exists: a live re-edit session draft or
/// the Text tool's singleton selected `Text` annotation. Everything
/// else (no selection, a non-Text selection, a Hand-tool inspector
/// selection, a *fresh* draft) falls back to the defaults path —
/// fresh drafts deliberately do not live-restyle (spec open
/// question default).
pub(crate) fn has_restyle_target(state: &EditorState) -> bool {
    if state.text_reedit.is_some() {
        return true;
    }
    text_selection(state).is_some()
}

/// The style the Text surfaces read this frame: the re-edit session
/// draft's, else the Text-tool singleton selection's, else the
/// session defaults. Byte-identical to the pre-selection behaviour
/// whenever neither draft nor selection exists.
pub(crate) fn effective_text_style(state: &EditorState) -> flintshot_core::text::TextStyle {
    if state.text_reedit.is_some() {
        if let Some(ann) = &state.pending {
            if let Annotation::Text { .. } = ann {
                return ann.text_style();
            }
        }
    }
    if let Some(ann) = text_selection(state) {
        return ann.text_style();
    }
    session_text_style(state)
}

/// Assemble the session-default style the pre-selection panel showed.
fn session_text_style(state: &EditorState) -> flintshot_core::text::TextStyle {
    flintshot_core::text::TextStyle {
        size: state.font_size,
        color: state.text_color,
        family: state.font_family,
        weight: state.font_weight,
        style: state.font_style,
        align: state.text_align,
        line_spacing: state.line_spacing,
        outline: state.text_outline,
        background: state.text_background,
    }
}

/// Write a changed style through the right path: core's restyle
/// (which also syncs the session defaults) when a target exists,
/// else the direct defaults writes of today.
fn write_text_style(state: &mut EditorState, st: &flintshot_core::text::TextStyle) {
    if has_restyle_target(state) {
        state.restyle_selected_text(*st);
        return;
    }
    state.font_size = st.size;
    state.text_color = st.color;
    state.font_family = st.family;
    state.font_weight = st.weight;
    state.font_style = st.style;
    state.text_align = st.align;
    state.line_spacing = st.line_spacing;
    state.text_outline = st.outline;
    state.text_background = st.background;
}

/// Apply a discrete (one-frame) style change: one begin / write / end
/// gesture — exactly one undo entry when a target exists, plain
/// defaults writes otherwise.
pub(crate) fn commit_text_style_change(
    state: &mut EditorState,
    st: flintshot_core::text::TextStyle,
) {
    state.begin_style_gesture();
    write_text_style(state, &st);
    state.end_style_gesture();
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
/// Reads from / writes to the same source the size row does
/// (session draft > selection > defaults) via one discrete gesture.
fn text_style_popup(ui: &mut egui::Ui, state: &mut EditorState) {
    ui.set_min_width(200.0);
    let mut st = effective_text_style(state);
    let before = st;
    typography_grid(ui, &mut st);
    ui.separator();
    let mut bg_on = st.background.is_some();
    ui.horizontal(|ui| {
        ui.checkbox(&mut bg_on, "Background");
        if bg_on {
            let cur = st.background.unwrap_or(Color::rgb(255, 235, 0));
            let mut rgb = [cur.r, cur.g, cur.b];
            ui.color_edit_button_srgb(&mut rgb);
            st.background = Some(Color::rgb(rgb[0], rgb[1], rgb[2]));
        } else {
            st.background = None;
        }
    });
    let mut ol_on = st.outline.is_some();
    ui.horizontal(|ui| {
        ui.checkbox(&mut ol_on, "Outline");
        if ol_on {
            let cur = st.outline.unwrap_or(TextOutline {
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
            st.outline = Some(o);
        } else {
            st.outline = None;
        }
    });
    if st != before {
        commit_text_style_change(state, st);
    }
}

/// The popup's family / weight / style / alignment grid, mutating the
/// effective style in place.
fn typography_grid(ui: &mut egui::Ui, st: &mut flintshot_core::text::TextStyle) {
    let chip = |ui: &mut egui::Ui, id: &str, label: &str, selected: bool| {
        segmented_chip(ui, egui::Id::new(id), label, selected)
    };
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
                if chip(
                    ui,
                    "text-family-sans",
                    "Sans",
                    st.family == FontFamily::SansSerif,
                ) {
                    st.family = FontFamily::SansSerif;
                }
                if chip(
                    ui,
                    "text-family-mono",
                    "Mono",
                    st.family == FontFamily::Monospace,
                ) {
                    st.family = FontFamily::Monospace;
                }
            });
            ui.end_row();
            head(ui, "Weight");
            ui.horizontal(|ui| {
                if chip(
                    ui,
                    "text-weight-normal",
                    "Regular",
                    st.weight == FontWeight::Normal,
                ) {
                    st.weight = FontWeight::Normal;
                }
                if chip(
                    ui,
                    "text-weight-bold",
                    "Bold",
                    st.weight == FontWeight::Bold,
                ) {
                    st.weight = FontWeight::Bold;
                }
            });
            ui.end_row();
            head(ui, "Style");
            ui.horizontal(|ui| {
                if chip(
                    ui,
                    "text-style-normal",
                    "Regular",
                    st.style == FontStyle::Normal,
                ) {
                    st.style = FontStyle::Normal;
                }
                if chip(
                    ui,
                    "text-style-italic",
                    "Italic",
                    st.style == FontStyle::Italic,
                ) {
                    st.style = FontStyle::Italic;
                }
            });
            ui.end_row();
            head(ui, "Align");
            ui.horizontal(|ui| {
                if chip(ui, "text-align-left", "Left", st.align == TextAlign::Left) {
                    st.align = TextAlign::Left;
                }
                if chip(
                    ui,
                    "text-align-center",
                    "Center",
                    st.align == TextAlign::Center,
                ) {
                    st.align = TextAlign::Center;
                }
                if chip(
                    ui,
                    "text-align-right",
                    "Right",
                    st.align == TextAlign::Right,
                ) {
                    st.align = TextAlign::Right;
                }
            });
            ui.end_row();
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
        Tool::Select => {
            "click selects · Shift/Ctrl+click toggles · drag marquees · drag on selection moves · Del removes"
        }
        Tool::Crop => "drag a region · Copy uses it · Enter crops",
        Tool::Hand => "drag pans · wheel zooms",
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

/// Crop: live region dims + crop. Without a region the bay shows
/// only the hint — no dead controls.
pub(crate) fn select_bay(ui: &mut egui::Ui, state: &mut EditorState) {
    let sel = state.crop_region.map(flintshot_core::Rect::normalized);
    if let Some(n) = sel.filter(|n| n.width().abs() >= 1.0 && n.height().abs() >= 1.0) {
        ui.label(
            egui::RichText::new(dims_label(n))
                .color(theme::palette::INK)
                .size(theme::TYPE_SM),
        );
        if ui.button("Crop · Enter").clicked() {
            state.confirm_crop();
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
/// emphasis, not new color. Renders the Hand chip in both rails.
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
            if swatch_chip(ui, sw, to_color32(*color) == sw) {
                *color = from_color32(sw);
            }
        }
        let mut pop_live = false;
        custom_color_chip(ui, color, &mut pop_live);
    });
}

/// `+` chip opening a custom-color popup (full sRGB editor).
/// `pop_live` reports whether the popup body ran (the inspector's
/// end-of-burst bracket).
fn custom_color_chip(ui: &mut egui::Ui, color: &mut Color, pop_live: &mut bool) {
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
            *pop_live = true;
            ui.set_min_width(140.0);
            ui.label("Custom color");
            ui.color_edit_button_srgb(&mut rgb);
            *color = Color::rgb(rgb[0], rgb[1], rgb[2]);
        });
}

/// S / M / L stroke presets as dot chips; a dot sized by the preset
/// communicates the effect faster than numbers. Returns the clicked
/// preset, if any — the shared core of the tool-defaults and
/// inspector stroke rows.
fn stroke_wells(ui: &mut egui::Ui, current: f32) -> Option<f32> {
    let mut clicked = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        for (i, preset) in theme::STROKE_PRESETS.iter().enumerate() {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), Sense::hover());
            let selected = (current - preset).abs() < 0.001;
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
                clicked = Some(*preset);
            }
        }
    });
    clicked
}

/// The tool-defaults stroke row (wells bound to the session
/// `stroke_width`).
fn stroke_chips(ui: &mut egui::Ui, state: &mut EditorState) {
    if let Some(preset) = stroke_wells(ui, state.stroke_width) {
        state.stroke_width = preset;
    }
}

pub(crate) fn to_color32(c: Color) -> Color32 {
    crate::theme::palette::opaque(c)
}

fn from_color32(c: Color32) -> Color {
    Color::rgb(c.r(), c.g(), c.b())
}

// ------------------------------------------------------------------
// Selection inspector (multi-select-properties)
// ------------------------------------------------------------------

/// The lowest-index selected member — the inspector's representative
/// for display values (edits still write every member).
fn first_selected(state: &EditorState) -> Option<&Annotation> {
    let &index = state.selected.first()?;
    state.anns.get(index)
}

/// The representative stroke colour: the first member's opaque
/// colour (a marker's translucency is display-only).
fn selection_color(state: &EditorState) -> Color {
    let raw = match first_selected(state) {
        Some(
            Annotation::Rect { stroke, .. }
            | Annotation::Ellipse { stroke, .. }
            | Annotation::Line { stroke, .. }
            | Annotation::Arrow { stroke, .. },
        ) => stroke.color,
        Some(
            Annotation::Marker { color, .. }
            | Annotation::Pencil { color, .. }
            | Annotation::Text { color, .. }
            | Annotation::Number { color, .. },
        ) => *color,
        _ => state.color,
    };
    Color::rgb(raw.r, raw.g, raw.b)
}

/// The representative stroke width.
fn selection_stroke_width(state: &EditorState) -> f32 {
    match first_selected(state) {
        Some(
            Annotation::Rect { stroke, .. }
            | Annotation::Ellipse { stroke, .. }
            | Annotation::Line { stroke, .. }
            | Annotation::Arrow { stroke, .. },
        ) => stroke.width,
        Some(Annotation::Marker { width, .. } | Annotation::Pencil { width, .. }) => *width,
        _ => state.stroke_width,
    }
}

/// Whether the representative member carries an interior fill.
fn selection_fill_on(state: &EditorState) -> bool {
    matches!(
        first_selected(state),
        Some(Annotation::Rect { fill: Some(_), .. } | Annotation::Ellipse { fill: Some(_), .. })
    )
}

/// The representative blur kind + strength.
fn selection_blur(state: &EditorState) -> (BlurKind, u8) {
    match first_selected(state) {
        Some(Annotation::Blur { kind, strength, .. }) => (*kind, *strength),
        _ => (state.blur_kind, 8),
    }
}

/// The representative bubble diameter.
fn selection_number_diameter(state: &EditorState) -> f32 {
    match first_selected(state) {
        Some(Annotation::Number { diameter, .. }) => *diameter,
        _ => state.number_diameter,
    }
}

/// One discrete swatch: paint + click detection. Shared by the
/// tool-defaults swatch row and the inspector's.
fn swatch_chip(ui: &mut egui::Ui, sw: Color32, selected: bool) -> bool {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), Sense::hover());
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
    resp.clicked()
}

/// The inspector's Stroke section: the same swatch row + stroke
/// wells, bound to the selection. Discrete swatch / well clicks
/// commit and close their burst (one undo entry each); the
/// custom-colour popover commits continuously under kind-based
/// coalescing and closes its burst when the popover closes
/// (bracketed via [`PanelState::color_pop_live`]).
fn selection_stroke(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    caps: flintshot_core::SelectionCapabilities,
    panel: &mut PanelState,
) {
    let current = selection_color(state);
    let mut pending = current;
    let mut pop_live = false;
    // No early return on a discrete click: skipping the remaining
    // widgets on the click frame leaves egui's interaction bookkeeping
    // stale for the next frame's widgets.
    let mut clicked = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        for sw in theme::palette::SWATCHES {
            if swatch_chip(ui, sw, to_color32(current) == sw) {
                clicked = Some(sw);
            }
        }
        custom_color_chip(ui, &mut pending, &mut pop_live);
    });
    if let Some(sw) = clicked {
        state.set_selected_color(from_color32(sw));
        state.end_property_edit();
    } else if pending != current {
        state.set_selected_color(pending);
    }
    panel.color_pop_live = pop_live;
    if caps.stroke_width {
        let width = selection_stroke_width(state);
        if let Some(preset) = stroke_wells(ui, width) {
            state.set_selected_stroke_width(preset);
            state.end_property_edit();
        }
    }
}

/// The inspector's Fill section: the same switch, bound to the
/// selection's interior fill.
fn selection_fill(ui: &mut egui::Ui, state: &mut EditorState) {
    let on = selection_fill_on(state);
    let resp = widgets::switch(ui, egui::Id::new("fill-switch"), on, false);
    if resp.on != on {
        state.set_selected_fill(resp.on);
        state.end_property_edit();
    }
}

/// The inspector's Text section: the size stepper + `Aa` style
/// popover, bound to the selection's [`TextStyle`]. Size edits
/// commit continuously (coalesced); the drag / focus loss and the
/// popover's close each end the burst.
fn selection_text(ui: &mut egui::Ui, state: &mut EditorState, panel: &mut PanelState) {
    let Some(mut style) = first_selected(state).and_then(|ann| match ann {
        Annotation::Text { .. } => Some(ann.text_style()),
        _ => None,
    }) else {
        return;
    };
    let before = style;
    let mut size = style.size;
    let minus = ui.small_button("-");
    let dv = ui.add_sized(
        [36.0, 20.0],
        egui::DragValue::new(&mut size).range(8.0..=72.0).speed(0.5),
    );
    let plus = ui.small_button("+");
    if minus.clicked() {
        size = (size - 1.0).max(8.0);
    }
    if plus.clicked() {
        size = (size + 1.0).min(72.0);
    }
    style.size = size;
    if style != before {
        state.set_selected_text_style(style);
    }
    if dv.drag_stopped() || dv.lost_focus() || minus.clicked() || plus.clicked() {
        state.end_property_edit();
    }
    let btn = ui.add(egui::Button::new(egui::RichText::new("Aa").strong()));
    let mut pop_live = false;
    egui::Popup::from_toggle_button_response(&btn)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            pop_live = true;
            text_style_popup_bound(ui, &mut style);
        });
    panel.text_pop_live = pop_live;
    if style != before {
        state.set_selected_text_style(style);
    }
}

/// The text-style popover grid, bound to a [`TextStyle`] local
/// instead of session defaults (the inspector's twin of
/// [`text_style_popup`]).
fn text_style_popup_bound(ui: &mut egui::Ui, style: &mut flintshot_core::TextStyle) {
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
                ui.selectable_value(&mut style.family, FontFamily::SansSerif, "Sans");
                ui.selectable_value(&mut style.family, FontFamily::Monospace, "Mono");
            });
            ui.end_row();
            head(ui, "Weight");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut style.weight, FontWeight::Normal, "Regular");
                ui.selectable_value(&mut style.weight, FontWeight::Bold, "Bold");
            });
            ui.end_row();
            head(ui, "Style");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut style.style, FontStyle::Normal, "Regular");
                ui.selectable_value(&mut style.style, FontStyle::Italic, "Italic");
            });
            ui.end_row();
            head(ui, "Align");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut style.align, TextAlign::Left, "Left");
                ui.selectable_value(&mut style.align, TextAlign::Center, "Center");
                ui.selectable_value(&mut style.align, TextAlign::Right, "Right");
            });
            ui.end_row();
        });
}

/// The inspector's Number section: the Size stepper bound to the
/// selection's bubbles (the Start-at stepper stays a placement
/// default — it has no committed-bubble meaning).
fn selection_number(ui: &mut egui::Ui, state: &mut EditorState) {
    let mut diameter = selection_number_diameter(state);
    let dv = ui.add(
        egui::DragValue::new(&mut diameter)
            .range(NUMBER_MIN_DIAMETER..=NUMBER_MAX_DIAMETER)
            .speed(1.0),
    );
    if (diameter - selection_number_diameter(state)).abs() > f32::EPSILON {
        state.set_selected_number_diameter(diameter);
    }
    if dv.drag_stopped() || dv.lost_focus() {
        state.end_property_edit();
    }
}

/// The inspector's Effects section: the blur-kind segmented chips
/// bound to the selection (the member's strength rides along).
fn selection_effects(ui: &mut egui::Ui, state: &mut EditorState) {
    let (kind, strength) = selection_blur(state);
    ui.horizontal(|ui| {
        if segmented_chip(
            ui,
            egui::Id::new("blur-gaussian"),
            "Gaussian",
            kind == BlurKind::Gaussian,
        ) {
            state.set_selected_blur(BlurKind::Gaussian, strength);
            state.end_property_edit();
        }
        if segmented_chip(
            ui,
            egui::Id::new("blur-pixelate"),
            "Pixelate",
            kind == BlurKind::Pixelate,
        ) {
            state.set_selected_blur(BlurKind::Pixelate, strength);
            state.end_property_edit();
        }
    });
}

/// The inspector's Selection section: the member count + the
/// pluralized Delete-all.
fn selection_selection_row(ui: &mut egui::Ui, state: &mut EditorState) -> bool {
    let n = state.selected.len();
    // Painter-drawn (the panel title's idiom), not `ui.label`: a
    // click-sensed label directly above the delete button steals the
    // click from it.
    let galley = ui.painter().layout_no_wrap(
        format!("{n} annotation{} selected", if n == 1 { "" } else { "s" }),
        egui::FontId::proportional(theme::TYPE_SM),
        theme::palette::INK_SOFT,
    );
    let (rect, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
    ui.painter()
        .galley(rect.min, galley, theme::palette::INK_SOFT);
    let label = if n == 1 {
        "🗑 Delete annotation".to_owned()
    } else {
        format!("🗑 Delete {n} annotations")
    };
    delete_button(ui, n > 0, &label)
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
/// `pub(crate)`: the topbar's Copy hero reuses this glyph (the
/// glyph-kit rule — no duplicated sheet logic).
pub(crate) fn g_copy(p: &egui::Painter, r: egui::Rect, c: Color32) {
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
#[cfg(test)]
#[allow(
    clippy::float_cmp,
    reason = "the Size-row tests assert equality against the exact const bounds the DragValue clamps to — the value is assigned, never computed"
)]
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
        state.selected = vec![0];
        state
    }

    fn action_after_delete_click(state: &mut EditorState) -> ToolbarAction {
        let ctx = egui::Context::default();
        crate::widgets::set_animations(&ctx, false);
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

    // --- Number section: the Size stepper (number-tool effort) ---

    /// The Size row is bound to `state.number_diameter` and wired to
    /// core's range consts: `DragValue::range` clamps an existing
    /// out-of-range value on render (egui's documented default with
    /// `clamp_existing_to_range`), so a stale diameter snaps into
    /// 16–96 the moment the Number section draws. One assertion
    /// covers render + binding + bounds without addressing the
    /// auto-id'd widget.
    #[test]
    fn number_size_row_clamps_stale_diameter_on_render() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut panel = PanelState::default();

        let mut state = panel_state();
        state.tool = Tool::Number;
        state.number_diameter = 200.0;
        let _ = draw(&ctx, &mut state, &mut panel, vec![]);
        assert_eq!(
            state.number_diameter,
            flintshot_core::NUMBER_MAX_DIAMETER,
            "a diameter above the range clamps down on render"
        );

        let mut state = panel_state();
        state.tool = Tool::Number;
        state.number_diameter = 8.0;
        let _ = draw(&ctx, &mut state, &mut panel, vec![]);
        assert_eq!(
            state.number_diameter,
            flintshot_core::NUMBER_MIN_DIAMETER,
            "a diameter below the range clamps up on render"
        );
    }

    /// The row renders only in the Number section: under `Tool::Rect`
    /// (whose section map has no Number card) an out-of-range value
    /// passes through untouched — no widget exists to clamp it.
    #[test]
    fn number_size_row_leaves_diameter_untouched_for_other_tools() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut state = panel_state();
        state.tool = Tool::Rect;
        state.number_diameter = 200.0;
        let mut panel = PanelState::default();
        let _ = draw(&ctx, &mut state, &mut panel, vec![]);
        assert_eq!(
            state.number_diameter, 200.0,
            "no Size widget renders outside the Number section"
        );
    }

    // --- Selection-inspector mode (multi-select-properties Task 004) ---

    /// A committed rect at a known spot, `n` of them selected, tool
    /// Select (the multi-selection owner).
    fn inspector_state(n_usable: usize) -> EditorState {
        let mut state = panel_state();
        state.tool = Tool::Select;
        state.anns.push(Annotation::Rect {
            rect: CoreRect::from_xywh(10.0, 10.0, 40.0, 30.0),
            stroke: CoreStroke::new(Color::RED, 3.0),
            fill: None,
        });
        state.anns.push(Annotation::Rect {
            rect: CoreRect::from_xywh(10.0, 60.0, 40.0, 30.0),
            stroke: CoreStroke::new(Color::RED, 3.0),
            fill: None,
        });
        state.selected = (0..n_usable).collect();
        state
    }

    /// The remapped section layout: Select owns the Selection
    /// inspector, Crop the region bay, Hand nothing (pan-only — no
    /// dead empty states).
    #[test]
    fn sections_map_follows_the_tool_remap() {
        assert_eq!(sections_for(Tool::Select), &[Section::Selection]);
        assert_eq!(sections_for(Tool::Crop), &[Section::Crop]);
        assert!(
            sections_for(Tool::Hand).is_empty(),
            "Hand renders no sections (pan-only)"
        );
        // The annotation tools are untouched by the remap.
        assert_eq!(
            sections_for(Tool::Rect),
            &[Section::Stroke, Section::Fill, Section::Selection]
        );
    }

    /// The per-tool hints speak the remapped semantics.
    #[test]
    fn tool_hints_match_the_tool_remap() {
        assert!(tool_hint(Tool::Select).contains("click selects"));
        assert!(tool_hint(Tool::Select).contains("marquee"));
        assert!(tool_hint(Tool::Crop).contains("Enter crops"));
        assert_eq!(tool_hint(Tool::Hand), "drag pans · wheel zooms");
    }

    #[test]
    fn panel_title_switches_by_mode() {
        let mut state = panel_state();
        assert_eq!(
            panel_title(&state),
            format!("{} · Properties", tool_display_name(state.tool)),
            "no selection: byte-for-byte today's title"
        );
        state.selected = vec![0, 1];
        assert_eq!(panel_title(&state), "2 selected · Properties");
    }

    fn rect_ann_fixture() -> Annotation {
        Annotation::Rect {
            rect: CoreRect::from_xywh(10.0, 10.0, 40.0, 30.0),
            stroke: CoreStroke::new(Color::RED, 3.0),
            fill: None,
        }
    }

    fn text_ann_fixture() -> Annotation {
        Annotation::text(
            flintshot_core::Point::new(5.0, 5.0),
            "hi",
            flintshot_core::TextStyle::simple(16.0, Color::RED),
        )
    }

    fn line_ann_fixture() -> Annotation {
        Annotation::Line {
            a: flintshot_core::Point::new(0.0, 0.0),
            b: flintshot_core::Point::new(9.0, 9.0),
            stroke: CoreStroke::new(Color::RED, 3.0),
        }
    }

    fn blur_ann_fixture() -> Annotation {
        Annotation::Blur {
            rect: CoreRect::from_xywh(0.0, 0.0, 5.0, 5.0),
            kind: flintshot_core::BlurKind::Gaussian,
            strength: 8,
        }
    }

    fn number_ann_fixture() -> Annotation {
        Annotation::Number {
            pos: flintshot_core::Point::new(5.0, 5.0),
            n: 1,
            color: Color::RED,
            diameter: 28.0,
        }
    }

    #[test]
    fn selection_sections_follow_capabilities() {
        let sections_for_anns = |anns: Vec<Annotation>| {
            let mut s = panel_state();
            s.anns = anns;
            s.selected = (0..s.anns.len()).collect();
            selection_sections(s.selected_capabilities())
        };
        assert_eq!(
            sections_for_anns(vec![rect_ann_fixture()]),
            vec![Section::Stroke, Section::Fill, Section::Selection],
            "a lone rect: the full shape kit"
        );
        assert_eq!(
            sections_for_anns(vec![text_ann_fixture()]),
            vec![Section::Stroke, Section::Text, Section::Selection],
            "a lone text: colour + popover"
        );
        assert_eq!(
            sections_for_anns(vec![blur_ann_fixture()]),
            vec![Section::Effects, Section::Selection],
            "a lone blur: effects only"
        );
        assert_eq!(
            sections_for_anns(vec![number_ann_fixture()]),
            vec![Section::Stroke, Section::Number, Section::Selection],
            "a lone number: colour + size"
        );
        assert_eq!(
            sections_for_anns(vec![rect_ann_fixture(), line_ann_fixture()]),
            vec![Section::Stroke, Section::Selection],
            "spec exit criterion 1: rect + line share colour + width only"
        );
        assert_eq!(
            sections_for_anns(vec![rect_ann_fixture(), blur_ann_fixture()]),
            vec![Section::Selection],
            "empty intersection: count + delete-only"
        );
    }

    #[test]
    fn swatch_click_edits_selection_with_one_undo_entry() {
        let ctx = egui::Context::default();
        crate::widgets::set_animations(&ctx, false);
        let mut state = inspector_state(1);
        let mut panel = PanelState::default();
        let _ = draw(&ctx, &mut state, &mut panel, vec![]);
        let target = theme::palette::SWATCHES[1];
        let id = egui::Id::new(("swatch", target.r(), target.g(), target.b()));
        let rect = ctx
            .read_response(id)
            .expect("pass 1 registered the selection-mode swatches")
            .rect;
        let action = draw(&ctx, &mut state, &mut panel, click_events(rect.center()));
        assert_eq!(
            action,
            ToolbarAction::None,
            "a swatch click fires no toolbar action"
        );
        match &state.anns[0] {
            Annotation::Rect { stroke, .. } => {
                assert_eq!(
                    stroke.color,
                    from_color32(target),
                    "the member recolours live"
                );
            }
            other => panic!("expected Rect, got {other:?}"),
        }
        assert_eq!(state.undo.len(), 1, "one discrete click = one undo entry");
        assert_eq!(
            state.color,
            from_color32(target),
            "the default learns the colour"
        );
        state.undo();
        match &state.anns[0] {
            Annotation::Rect { stroke, .. } => {
                assert_eq!(
                    stroke.color,
                    Color::RED,
                    "undo restores the pre-edit colour"
                );
            }
            other => panic!("expected Rect, got {other:?}"),
        }
    }

    #[test]
    fn separate_swatch_clicks_are_separate_undo_entries() {
        let ctx = egui::Context::default();
        crate::widgets::set_animations(&ctx, false);
        let mut state = inspector_state(2);
        let mut panel = PanelState::default();
        let _ = draw(&ctx, &mut state, &mut panel, vec![]);
        for target in [theme::palette::SWATCHES[1], theme::palette::SWATCHES[2]] {
            let id = egui::Id::new(("swatch", target.r(), target.g(), target.b()));
            let rect = ctx.read_response(id).expect("swatch registered").rect;
            let _ = draw(&ctx, &mut state, &mut panel, click_events(rect.center()));
        }
        assert_eq!(
            state.undo.len(),
            2,
            "each discrete gesture closes its burst"
        );
        for i in 0..2 {
            match &state.anns[i] {
                Annotation::Rect { stroke, .. } => {
                    assert_eq!(stroke.color, from_color32(theme::palette::SWATCHES[2]));
                }
                other => panic!("expected Rect, got {other:?}"),
            }
        }
    }

    #[test]
    fn inspector_delete_click_fires_delete_for_plural_set() {
        let mut state = inspector_state(2);
        assert_eq!(action_after_delete_click(&mut state), ToolbarAction::Delete);
    }

    #[test]
    fn inspector_sections_render_for_mixed_set() {
        // rect + text: Stroke renders, Fill/Text-style stay hidden —
        // the intersection rule — and the panel still renders + the
        // delete button stays reachable.
        let ctx = egui::Context::default();
        let mut state = panel_state();
        state.tool = Tool::Select;
        state.anns.push(Annotation::Rect {
            rect: CoreRect::from_xywh(10.0, 10.0, 40.0, 30.0),
            stroke: CoreStroke::new(Color::RED, 3.0),
            fill: None,
        });
        state.anns.push(Annotation::text(
            flintshot_core::Point::new(5.0, 5.0),
            "hi",
            flintshot_core::TextStyle::simple(16.0, Color::RED),
        ));
        state.selected = vec![0, 1];
        let mut panel = PanelState::default();
        let _ = draw(&ctx, &mut state, &mut panel, vec![]);
        // The swatch row (Stroke) registers; the fill switch (Fill
        // section) must not.
        let sw = theme::palette::SWATCHES[1];
        assert!(
            ctx.read_response(egui::Id::new(("swatch", sw.r(), sw.g(), sw.b())))
                .is_some(),
            "the shared Stroke section renders"
        );
        assert!(
            ctx.read_response(egui::Id::new("fill-switch")).is_none(),
            "the Fill section stays hidden for a mixed rect+text set"
        );
        assert!(
            ctx.read_response(egui::Id::new("panel-delete")).is_some(),
            "the Selection section always renders"
        );
    }

    // --- Text section retarget (text-select-edit Task 006) ---

    /// Text-tool state with a selected text annotation at a
    /// distinctive 23 px / blue style — every read must come from it,
    /// not the 16 px / red session defaults.
    fn text_selected_panel_state() -> EditorState {
        let mut state = panel_state();
        state.tool = Tool::Text;
        state.anns.push(Annotation::text(
            flintshot_core::Point::new(4.0, 4.0),
            "hi",
            flintshot_core::text::TextStyle::simple(23.0, Color::rgb(0, 0, 255)),
        ));
        state.selected = vec![0];
        state
    }

    fn chip_center(ctx: &egui::Context, id: egui::Id) -> egui::Pos2 {
        ctx.read_response(id)
            .unwrap_or_else(|| panic!("{id:?} registered"))
            .rect
            .center()
    }

    /// A + click reads the selection's 23, writes 24 back onto it,
    /// syncs the session defaults, and records exactly one undo entry
    /// per discrete click.
    #[test]
    fn text_size_plus_click_restyles_selection_one_entry_per_click() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        widgets::set_animations(&ctx, false);
        let mut state = text_selected_panel_state();
        let mut panel = PanelState::default();
        let _ = draw(&ctx, &mut state, &mut panel, vec![]);
        let at = chip_center(&ctx, egui::Id::new("text-size-plus"));
        draw(&ctx, &mut state, &mut panel, click_events(at));
        match &state.anns[0] {
            Annotation::Text {
                font_size, color, ..
            } => {
                assert_eq!(*font_size, 24.0, "reads the selection's 23, writes 24");
                assert_eq!(
                    *color,
                    Color::rgb(0, 0, 255),
                    "untouched fields survive the write"
                );
            }
            other => panic!("expected Text, got {other:?}"),
        }
        assert_eq!(state.font_size, 24.0, "defaults synced");
        assert_eq!(state.undo.len(), 1, "one entry per discrete click");
        draw(&ctx, &mut state, &mut panel, click_events(at));
        match &state.anns[0] {
            Annotation::Text { font_size, .. } => assert_eq!(*font_size, 25.0),
            other => panic!("expected Text, got {other:?}"),
        }
        assert_eq!(state.undo.len(), 2, "the second click is a second gesture");
    }

    /// A `DragValue` drag coalesces: press → several move frames →
    /// release leaves one undo entry covering the whole drag.
    #[test]
    fn text_size_drag_coalesces_into_one_undo_entry() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        widgets::set_animations(&ctx, false);
        let mut state = text_selected_panel_state();
        let mut panel = PanelState::default();
        let _ = draw(&ctx, &mut state, &mut panel, vec![]);
        let minus = ctx
            .read_response(egui::Id::new("text-size-minus"))
            .expect("minus chip registered")
            .rect;
        // The DragValue occupies the 36 px slot right of the minus
        // chip; its centre is a safe drag anchor.
        let at = egui::pos2(minus.right() + 20.0, minus.center().y);
        let press = |pos| {
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        draw(&ctx, &mut state, &mut panel, press(at));
        draw(
            &ctx,
            &mut state,
            &mut panel,
            vec![egui::Event::PointerMoved(egui::pos2(at.x + 10.0, at.y))],
        );
        assert!(
            state.undo.is_empty(),
            "mid-drag the gesture is still open (no entry yet)"
        );
        draw(
            &ctx,
            &mut state,
            &mut panel,
            vec![egui::Event::PointerMoved(egui::pos2(at.x + 20.0, at.y))],
        );
        draw(
            &ctx,
            &mut state,
            &mut panel,
            vec![egui::Event::PointerButton {
                pos: egui::pos2(at.x + 20.0, at.y),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        match &state.anns[0] {
            Annotation::Text { font_size, .. } => {
                assert!(*font_size > 23.0, "the drag grew the size ({font_size})");
            }
            other => panic!("expected Text, got {other:?}"),
        }
        assert_eq!(state.undo.len(), 1, "the whole drag is one entry");
    }

    /// The typography popup's family chips route through the same
    /// write path: one click restyles the selection + syncs defaults
    /// with one entry. Driven at the popup-body level (the Aa popup
    /// hosts these exact widgets).
    #[test]
    fn text_popup_family_chip_restyles_selection() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        widgets::set_animations(&ctx, false);
        let mut state = text_selected_panel_state();
        let _ = ctx.run_ui(screen(1024.0, 768.0), |ui| {
            text_style_popup(ui, &mut state);
        });
        let at = chip_center(&ctx, egui::Id::new("text-family-mono"));
        let _ = ctx.run_ui(
            egui::RawInput {
                events: click_events(at),
                ..screen(1024.0, 768.0)
            },
            |ui| {
                text_style_popup(ui, &mut state);
            },
        );
        match &state.anns[0] {
            Annotation::Text {
                family,
                font_size,
                color,
                ..
            } => {
                assert_eq!(*family, FontFamily::Monospace, "family written");
                assert_eq!(*font_size, 23.0, "the read came from the selection");
                assert_eq!(*color, Color::rgb(0, 0, 255), "untouched fields survive");
            }
            other => panic!("expected Text, got {other:?}"),
        }
        assert_eq!(state.font_family, FontFamily::Monospace, "defaults synced");
        assert_eq!(state.undo.len(), 1);
    }

    /// Mid-session the surfaces read — and write — the lifted-out
    /// draft: a + click restyles `pending` and records nothing (the
    /// session's commit owns the single entry).
    #[test]
    fn text_size_plus_mid_session_targets_draft() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        widgets::set_animations(&ctx, false);
        let mut state = text_selected_panel_state();
        // Lift the annotation out (the DoubleClick path, driven
        // directly — the panel only cares that a session is live).
        state.dispatch_input(&flintshot_core::input::InputEvent::DoubleClick {
            pos: flintshot_core::Point::new(5.0, 5.0),
        });
        assert!(state.text_reedit.is_some(), "fixture: session live");
        let mut panel = PanelState::default();
        let _ = draw(&ctx, &mut state, &mut panel, vec![]);
        let at = chip_center(&ctx, egui::Id::new("text-size-plus"));
        draw(&ctx, &mut state, &mut panel, click_events(at));
        match &state.pending {
            Some(Annotation::Text { font_size, .. }) => {
                assert_eq!(*font_size, 24.0, "the draft carries the new size");
            }
            other => panic!("expected pending draft, got {other:?}"),
        }
        assert_eq!(state.font_size, 24.0, "defaults still sync");
        assert!(state.undo.is_empty(), "no independent entry mid-session");
    }

    /// No selection, no session: the row keeps today's byte-identical
    /// defaults path — direct field writes, no history.
    #[test]
    fn text_size_plus_without_selection_keeps_defaults_path() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        widgets::set_animations(&ctx, false);
        let mut state = panel_state();
        state.tool = Tool::Text;
        state.font_size = 16.0;
        let mut panel = PanelState::default();
        let _ = draw(&ctx, &mut state, &mut panel, vec![]);
        let at = chip_center(&ctx, egui::Id::new("text-size-plus"));
        draw(&ctx, &mut state, &mut panel, click_events(at));
        assert_eq!(state.font_size, 17.0, "defaults path still works");
        assert!(state.undo.is_empty(), "defaults writes record no history");
    }

    // --- Tab header (properties-history-tabs Task 003) ---

    /// The header registers both tab chips; the Properties tab is the
    /// default and its tool sections render beneath it.
    #[test]
    fn tab_header_registers_and_defaults_to_properties() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        widgets::set_animations(&ctx, false);
        let mut state = panel_state();
        state.tool = Tool::Rect;
        let mut panel = PanelState::default();
        assert_eq!(panel.tab, PanelTab::Properties, "default tab");
        let _ = draw(&ctx, &mut state, &mut panel, vec![]);
        for tab in [PanelTab::Properties, PanelTab::History] {
            assert!(
                ctx.read_response(egui::Id::new(("panel-tab", tab)))
                    .is_some(),
                "{tab:?} chip registers"
            );
        }
        let delete = ctx
            .read_response(egui::Id::new("panel-delete"))
            .expect("the Properties body renders the tool sections");
        let header = ctx
            .read_response(egui::Id::new(("panel-tab", PanelTab::Properties)))
            .expect("header chip")
            .rect;
        assert!(
            delete.rect.top() > header.bottom(),
            "section content renders beneath the header"
        );
    }

    /// Clicking HISTORY swaps the body (tool sections gone, the
    /// placeholder visible), the choice persists across frames, and
    /// clicking PROPERTIES restores the sections.
    #[test]
    fn clicking_history_swaps_body_and_persists() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        widgets::set_animations(&ctx, false);
        let mut state = panel_state();
        state.tool = Tool::Rect;
        let mut panel = PanelState::default();
        let _ = draw(&ctx, &mut state, &mut panel, vec![]);
        let history = chip_center(&ctx, egui::Id::new(("panel-tab", PanelTab::History)));
        let _ = draw(&ctx, &mut state, &mut panel, click_events(history));
        assert_eq!(panel.tab, PanelTab::History, "the click selects the tab");
        // Settle pass: read_response falls back to the previous
        // pass's widget map, so absence needs one clean redraw.
        let _ = draw(&ctx, &mut state, &mut panel, vec![]);
        assert_eq!(panel.tab, PanelTab::History);
        assert!(
            ctx.read_response(egui::Id::new("panel-delete")).is_none(),
            "the History body stays up and the tool sections are gone"
        );
        // Clicking PROPERTIES restores the sections.
        let props = chip_center(&ctx, egui::Id::new(("panel-tab", PanelTab::Properties)));
        let _ = draw(&ctx, &mut state, &mut panel, click_events(props));
        assert_eq!(panel.tab, PanelTab::Properties);
        assert!(
            ctx.read_response(egui::Id::new("panel-delete")).is_some(),
            "the Properties sections return"
        );
    }

    /// The inspector mode's contextual title also renders beneath the
    /// header (`N selected · Properties` path unchanged).
    #[test]
    fn inspector_body_still_renders_beneath_the_header() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        widgets::set_animations(&ctx, false);
        let mut state = inspector_state(2);
        let mut panel = PanelState::default();
        let _ = draw(&ctx, &mut state, &mut panel, vec![]);
        let header = ctx
            .read_response(egui::Id::new(("panel-tab", PanelTab::Properties)))
            .expect("header chip")
            .rect;
        let delete = ctx
            .read_response(egui::Id::new("panel-delete"))
            .expect("the inspector body renders")
            .rect;
        assert!(delete.top() > header.bottom());
    }

    // --- History timeline (properties-history-tabs Task 004) ---

    /// A scripted mixed session: rect ×2, number, move, delete, crop,
    /// property edit — seven undo entries.
    fn scripted_history_state() -> EditorState {
        use flintshot_core::input::InputEvent as CoreEvent;
        let mut state = panel_state();
        for _ in 0..2 {
            state.pending = Some(rect_ann_fixture());
            state.commit_annotation();
        }
        state.place_number(flintshot_core::Point::new(5.0, 5.0));
        state.tool = Tool::Select;
        state.selected = vec![0];
        state.dispatch_input(&CoreEvent::PointerDown {
            pos: flintshot_core::Point::new(20.0, 20.0),
            modifiers: flintshot_core::input::Modifiers::default(),
        });
        state.dispatch_input(&CoreEvent::PointerDrag {
            pos: flintshot_core::Point::new(24.0, 24.0),
        });
        state.dispatch_input(&CoreEvent::PointerUp);
        state.selected = vec![0];
        state.delete_selected();
        state.crop(CoreRect::from_xywh(0.0, 0.0, 30.0, 30.0));
        state.selected = vec![0];
        state.set_selected_color(Color::rgb(1, 2, 3));
        state
    }

    fn history_panel() -> PanelState {
        PanelState {
            tab: PanelTab::History,
            ..PanelState::default()
        }
    }

    /// One row per undo entry, oldest first, the current-state marker
    /// at the boundary, no future rows without redo.
    #[test]
    fn timeline_renders_one_row_per_entry_oldest_first() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        widgets::set_animations(&ctx, false);
        let mut state = scripted_history_state();
        assert_eq!(state.undo.len(), 7, "fixture: seven gestures");
        let mut panel = history_panel();
        let _ = draw(&ctx, &mut state, &mut panel, vec![]);
        let mut prev_top = 0.0;
        for i in 0..7usize {
            let rect = ctx
                .read_response(egui::Id::new(("history-row", i)))
                .unwrap_or_else(|| panic!("row {i} registers"))
                .rect;
            assert!(
                rect.top() > prev_top,
                "row {i} renders below row {} (oldest first)",
                i.saturating_sub(1)
            );
            prev_top = rect.top();
        }
        let marker = ctx
            .read_response(egui::Id::new("history-current"))
            .expect("the current-state marker registers")
            .rect;
        assert!(
            marker.top() > prev_top,
            "the marker sits below the last past row"
        );
        assert!(
            ctx.read_response(egui::Id::new(("history-future", 0)))
                .is_none(),
            "no future rows without redo"
        );
    }

    /// Clicking a past row restores that state in one action; a
    /// dimmed future row redoes forward; chord-stepping is reflected
    /// live on the next pass.
    #[test]
    fn clicking_rows_time_travels_and_chords_reflect_live() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        widgets::set_animations(&ctx, false);
        let mut state = panel_state();
        state.pending = Some(rect_ann_fixture());
        state.commit_annotation();
        state.place_number(flintshot_core::Point::new(5.0, 5.0));
        state.place_number(flintshot_core::Point::new(6.0, 6.0));
        let mut panel = history_panel();
        let _ = draw(&ctx, &mut state, &mut panel, vec![]);
        // Jump back to the very first state via row 0.
        let row0 = chip_center(&ctx, egui::Id::new(("history-row", 0)));
        let _ = draw(&ctx, &mut state, &mut panel, click_events(row0));
        assert_eq!(state.undo.len(), 1, "one click = one restore_to");
        assert_eq!(state.redo.len(), 2);
        // Settle: the future rows render now.
        let _ = draw(&ctx, &mut state, &mut panel, vec![]);
        let future0 = ctx
            .read_response(egui::Id::new(("history-future", 0)))
            .expect("nearest future row registers")
            .rect
            .center();
        let _ = draw(&ctx, &mut state, &mut panel, click_events(future0));
        assert_eq!(state.undo.len(), 2, "a future click redoes forward one");
        // A chord step (the floating cluster's Ctrl+Z path) shows on
        // the next pass: the nearest future row returns.
        state.undo();
        assert_eq!(state.undo.len(), 1);
        let _ = draw(&ctx, &mut state, &mut panel, vec![]);
        assert_eq!(state.undo.len(), 1, "redrawing is inert without clicks");
        let _ = draw(&ctx, &mut state, &mut panel, vec![]);
        assert!(
            ctx.read_response(egui::Id::new(("history-future", 0)))
                .is_some(),
            "the timeline reflects the chord step"
        );
        assert!(
            ctx.read_response(egui::Id::new(("history-row", 2)))
                .is_none(),
            "the undone past row is gone from the timeline"
        );
    }

    /// Empty stacks: the quiet hint, no rows, no marker.
    #[test]
    fn empty_history_renders_hint_only() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        widgets::set_animations(&ctx, false);
        let mut state = panel_state();
        let mut panel = history_panel();
        let _ = draw(&ctx, &mut state, &mut panel, vec![]);
        for id in [
            egui::Id::new(("history-row", 0)),
            egui::Id::new("history-current"),
            egui::Id::new(("history-future", 0)),
        ] {
            assert!(ctx.read_response(id).is_none(), "{id:?} must not render");
        }
    }

    /// Long histories scroll instead of overflowing: every row still
    /// registers and the content extends past the panel's bottom.
    #[test]
    fn long_history_scrolls_instead_of_overflowing() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        widgets::set_animations(&ctx, false);
        let mut state = panel_state();
        for _ in 0..50 {
            state.pending = Some(rect_ann_fixture());
            state.commit_annotation();
        }
        let mut panel = history_panel();
        let _ = draw(&ctx, &mut state, &mut panel, vec![]);
        let last = ctx
            .read_response(egui::Id::new(("history-row", 49)))
            .expect("the 50th row registers inside the scroll area")
            .rect;
        assert!(
            last.top() > 768.0,
            "content overflows the 768 px viewport (scrolls, not overflows)"
        );
    }
}
