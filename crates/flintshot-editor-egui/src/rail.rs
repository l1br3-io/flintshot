//! The left tool rail — zone 2 of the five-zone editor (mockup
//! `zone-map.html`): a 56 px forest-mid strip carrying the ten real
//! tools as 40×40 chips, grouped by the five semantic pods with
//! hairline separators.
//!
//! The active tool pops: gold fill, cream border, pop shadow, and a
//! spring bounce (through the animation gate). Gated-off tools stay
//! visible in lock-grey — hover-only, layout stable — so the
//! selection-first phase never hides the vocabulary. The tool
//! vocabulary ([`TOOL_PODS`], glyphs, [`ToolGate`]) moved here from
//! the retired side-panel rail; the overlay's floating row still
//! walks [`TOOL_PODS`].

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Stroke, StrokeKind, Vec2};
use flintshot_core::{EditorState, Tool};

use crate::theme;
use crate::widgets;

/// Rail width, logical px — the mockup's zone contract.
pub(crate) const RAIL_WIDTH: f32 = 56.0;
/// Rail tool chip edge, logical px (mockup: rail tool 40×40).
pub(crate) const RAIL_TOOL: f32 = 40.0;

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
/// redact. Select leads the selection pod; Hand and Crop follow
/// (V / H / C) so the three region-and-object jobs sit together.
///
/// The pod arrays below are the single source of truth; both the
/// editor rail and the overlay's floating tool row walk
/// [`TOOL_PODS`].
const SELECTION_POD: &[ToolSpec] = &[
    ToolSpec {
        tool: Tool::Select,
        name: "Select",
        key: "V",
        glyph: g_select,
    },
    ToolSpec {
        tool: Tool::Hand,
        name: "Hand",
        key: "H",
        glyph: g_hand,
    },
    ToolSpec {
        tool: Tool::Crop,
        name: "Crop",
        key: "C",
        glyph: g_crop,
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
    /// Only `Tool::Crop` is live; every other chip renders
    /// disabled with a lock clause in its tooltip.
    CropOnly,
}

impl ToolGate {
    /// Whether `tool`'s chip is clickable under this gate.
    #[must_use]
    pub(crate) fn contains(self, tool: Tool) -> bool {
        match self {
            ToolGate::All => true,
            ToolGate::CropOnly => tool == Tool::Crop,
        }
    }
}

/// Lock clause appended to a gated-off chip's tooltip. Phrased for
/// the editor's selection-first phase, the only gate user today.
pub(crate) const LOCK_CLAUSE: &str = "locked — Enter crops to unlock";

/// Glyph-space point: `(0,0)` centre, `±7` edges, +Y down.
pub(crate) fn pt(rect: egui::Rect, x: f32, y: f32) -> Pos2 {
    egui::pos2(rect.center().x + x, rect.center().y + y)
}

/// Glyph-space line segment from `a` to `b` (glyph coords, see
/// [`pt`]).
pub(crate) fn seg(
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
pub(crate) fn arrowhead(p: &egui::Painter, tip: Pos2, dir: Vec2, c: Color32) {
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

/// Stylized open hand — the pan idiom (palm + three fingers +
/// thumb). Simple strokes by design: it must read at 28 px next to
/// the arrow glyphs.
fn g_hand(p: &egui::Painter, r: egui::Rect, c: Color32) {
    // Palm.
    let palm = egui::Rect::from_min_size(pt(r, -4.0, -1.0), egui::vec2(8.0, 6.5));
    p.rect(
        palm,
        CornerRadius::same(2),
        Color32::TRANSPARENT,
        Stroke::new(2.0, c),
        StrokeKind::Middle,
    );
    // Fingers.
    seg(p, r, (-2.0, -5.5), (-2.0, -1.0), c, 2.0);
    seg(p, r, (0.5, -6.5), (0.5, -1.0), c, 2.0);
    seg(p, r, (3.0, -5.0), (3.0, -1.0), c, 2.0);
    // Thumb, angled off the palm's right edge.
    seg(p, r, (4.0, 2.5), (6.5, 1.0), c, 2.0);
}

/// Two overlapping right angles — the classic crop idiom. Reads as
/// "cut this region" and, unlike the old viewfinder brackets, can't
/// be mistaken for the dashed marquee at a glance.
fn g_crop(p: &egui::Painter, r: egui::Rect, c: Color32) {
    for (a, b) in [
        ((-3.0, -6.0), (-3.0, 3.0)),
        ((-3.0, 3.0), (6.0, 3.0)),
        ((3.0, 6.0), (3.0, -3.0)),
        ((3.0, -3.0), (-6.0, -3.0)),
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

// ------------------------------------------------------------------
// Rail
// ------------------------------------------------------------------

/// Rail state that persists across frames: the (lazily created)
/// grain texture handle.
#[derive(Default)]
pub(crate) struct RailState {
    /// The grain tile, created on first use.
    grain: Option<egui::TextureHandle>,
}

/// A rail chip's tooltip: `name · key`, plus the lock clause when
/// the gate holds the tool back. Pure for tests.
pub(crate) fn tooltip_text(spec: &ToolSpec, gate: ToolGate) -> String {
    if gate.contains(spec.tool) {
        format!("{} · {}", spec.name, spec.key)
    } else {
        format!("{} · {} · {}", spec.name, spec.key, LOCK_CLAUSE)
    }
}

/// Draw the rail as the window's left panel. Call it after the
/// top / bottom panels and before the right panel (draw order:
/// top → bottom → left → right).
///
/// Returns whether the **Ask the shot** spark chip fired (an
/// action, not a tool — `state.tool` never changes).
pub(crate) fn show(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    gate: ToolGate,
    rail: &mut RailState,
) -> bool {
    let mut ai_pressed = false;
    egui::Panel::left("tool_rail")
        .resizable(false)
        .exact_size(RAIL_WIDTH)
        .frame(
            egui::Frame::new()
                .fill(theme::palette::FOREST_MID)
                .inner_margin(egui::Margin::same(6)),
        )
        .show(ui, |ui| {
            stamp_grain(ui, rail);
            ui.vertical(|ui| {
                let pod_count = TOOL_PODS.len();
                for (i, pod) in TOOL_PODS.iter().enumerate() {
                    for spec in pod.tools {
                        tool_chip(ui, state, gate, spec);
                    }
                    if i + 1 < pod_count {
                        ui.add_space(4.0);
                        let w = ui.available_width();
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(w, 1.0), egui::Sense::hover());
                        ui.painter().line_segment(
                            [rect.left_center(), rect.right_center()],
                            Stroke::new(
                                1.0,
                                theme::palette::with_alpha(theme::palette::CREAM_TEXT, 46),
                            ),
                        );
                        ui.add_space(4.0);
                    }
                }
                // The Ask-the-shot pod: below a hairline, after the
                // redact pod — an action chip in its own island.
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ai_pressed = spark_chip(ui);
                    ui.add_space(4.0);
                    let w = ui.available_width();
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(w, 1.0), egui::Sense::hover());
                    ui.painter().line_segment(
                        [rect.left_center(), rect.right_center()],
                        Stroke::new(
                            1.0,
                            theme::palette::with_alpha(theme::palette::CREAM_TEXT, 46),
                        ),
                    );
                    ui.add_space(4.0);
                });
            });
        });
    ai_pressed
}

/// The Ask-the-shot action chip: a four-point gold spark, hover-lift
/// only. Deliberately **not** a tool chip — no pressed/sticky state,
/// no gate interaction (never lock-grey); pressing opens the
/// transform dialog (`crate::ai_dialog`).
fn spark_chip(ui: &mut egui::Ui) -> bool {
    let id = egui::Id::new("rail-ai");
    let (rect, _) = ui.allocate_exact_size(egui::vec2(RAIL_TOOL, RAIL_TOOL), egui::Sense::hover());
    let resp = ui
        .interact(rect, id, egui::Sense::click())
        .on_hover_text("Ask the shot (I)");

    // Hover-lift shadow only — the panel's hover-lift language; no
    // selection styling because there is nothing to select.
    let chip_rect = if resp.hovered() {
        let lifted = egui::Rect::from_center_size(rect.center(), rect.size() * 1.06);
        ui.painter()
            .add(theme::SHADOW_LIFT.as_shape(lifted, egui::CornerRadius::same(theme::POD_RADIUS)));
        lifted
    } else {
        rect
    };
    ui.painter().rect(
        chip_rect,
        egui::CornerRadius::same(theme::POD_RADIUS),
        if resp.hovered() {
            theme::palette::with_alpha(theme::palette::CREAM_TEXT, 31)
        } else {
            Color32::TRANSPARENT
        },
        egui::Stroke::NONE,
        egui::StrokeKind::Middle,
    );
    g_spark(ui.painter(), chip_rect, theme::palette::GOLD);
    resp.clicked()
}

/// Four-point spark — the Ask-the-shot mark: a diamond star of four
/// tapered points, gold on the forest rail.
fn g_spark(p: &egui::Painter, r: egui::Rect, c: Color32) {
    // Centre cross of four lines out to the tips, plus a smaller
    // rotated square for the star's body — reads at 28 px.
    let ct = r.center();
    let long = 7.0;
    let short = 2.2;
    p.line_segment(
        [
            egui::pos2(ct.x - long, ct.y),
            egui::pos2(ct.x - short, ct.y),
        ],
        Stroke::new(1.8, c),
    );
    p.line_segment(
        [
            egui::pos2(ct.x + short, ct.y),
            egui::pos2(ct.x + long, ct.y),
        ],
        Stroke::new(1.8, c),
    );
    p.line_segment(
        [
            egui::pos2(ct.x, ct.y - long),
            egui::pos2(ct.x, ct.y - short),
        ],
        Stroke::new(1.8, c),
    );
    p.line_segment(
        [
            egui::pos2(ct.x, ct.y + short),
            egui::pos2(ct.x, ct.y + long),
        ],
        Stroke::new(1.8, c),
    );
    let body = short * std::f32::consts::FRAC_1_SQRT_2;
    p.line_segment(
        [egui::pos2(ct.x - body, ct.y), egui::pos2(ct.x, ct.y - body)],
        Stroke::new(1.8, c),
    );
    p.line_segment(
        [egui::pos2(ct.x, ct.y - body), egui::pos2(ct.x + body, ct.y)],
        Stroke::new(1.8, c),
    );
    p.line_segment(
        [egui::pos2(ct.x + body, ct.y), egui::pos2(ct.x, ct.y + body)],
        Stroke::new(1.8, c),
    );
    p.line_segment(
        [egui::pos2(ct.x, ct.y + body), egui::pos2(ct.x - body, ct.y)],
        Stroke::new(1.8, c),
    );
}

/// Stamp the grain tile under the chips — first widget call in the
/// panel body so it lands beneath them.
fn stamp_grain(ui: &mut egui::Ui, rail: &mut RailState) {
    if rail.grain.is_none() {
        let image = egui::ColorImage::new([64, 64], theme::grain::tile(0x5EED_C0DE, 64));
        let options = egui::TextureOptions {
            wrap_mode: egui::TextureWrapMode::Repeat,
            ..Default::default()
        };
        rail.grain = Some(ui.ctx().load_texture("rail-grain", image, options));
    }
    if let Some(texture) = rail.grain.as_ref() {
        theme::grain::paint_grain(ui.painter(), ui.max_rect(), texture);
    }
}

/// One 40×40 rail chip. Active: gold fill, cream border, pop
/// shadow, and a spring bounce on switch (through the animation
/// gate). Gated-off: lock-grey, hover-only.
fn tool_chip(ui: &mut egui::Ui, state: &mut EditorState, gate: ToolGate, spec: &ToolSpec) {
    use crate::theme::palette as p;
    let enabled = gate.contains(spec.tool);
    let selected = state.tool == spec.tool;
    let id = egui::Id::new(("rail-tool", spec.key));
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(RAIL_TOOL, RAIL_TOOL), egui::Sense::hover());
    let resp = ui
        .interact(rect, id, sense)
        .on_hover_text(tooltip_text(spec, gate));

    // The spring pop: the freshly-selected chip paints smaller and
    // overshoots to rest. Deselected tools reset their progress so
    // reselecting bounces again.
    let eased = if selected {
        bounce(ui.ctx(), id)
    } else {
        ui.ctx().data_mut(|d| d.insert_temp(id, 0.0_f32));
        1.0
    };
    let scale = if enabled { 0.85 + 0.15 * eased } else { 1.0 };
    let chip_rect = egui::Rect::from_center_size(rect.center(), rect.size() * scale);

    let (fill, glyph_color, stroke, shadow) = if selected {
        (
            p::GOLD,
            p::FOREST_INK,
            Stroke::new(2.0, p::PAPER_CREAM),
            Some(theme::SHADOW_POP),
        )
    } else if !enabled {
        (
            p::with_alpha(p::LOCK_GREY, 60),
            p::LOCK_GREY,
            Stroke::NONE,
            None,
        )
    } else if resp.hovered() {
        (
            p::with_alpha(p::CREAM_TEXT, 31),
            p::CREAM_TEXT,
            Stroke::NONE,
            None,
        )
    } else {
        (Color32::TRANSPARENT, p::CREAM_TEXT, Stroke::NONE, None)
    };
    if let Some(shadow) = shadow {
        ui.painter()
            .add(shadow.as_shape(chip_rect, egui::CornerRadius::same(theme::POD_RADIUS)));
    }
    ui.painter().rect(
        chip_rect,
        egui::CornerRadius::same(theme::POD_RADIUS),
        fill,
        stroke,
        egui::StrokeKind::Inside,
    );
    (spec.glyph)(ui.painter(), chip_rect, glyph_color);

    if resp.clicked() {
        state.set_tool(spec.tool);
    }
}

/// The active chip's spring progress toward rest (0 freshly
/// selected → 1 settled), eased through the shared spring. The
/// animation gate jumps it straight to 1.
fn bounce(ctx: &egui::Context, id: egui::Id) -> f32 {
    if !widgets::animate(ctx) {
        return 1.0;
    }
    let dt = ctx.input(|i| i.stable_dt);
    let t = ctx.data_mut(|d| {
        let current = d.get_temp::<f32>(id).unwrap_or(0.0);
        let next = theme::ease::approach(current, 1.0, dt, 0.25);
        d.insert_temp(id, next);
        next
    });
    theme::ease::spring(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat() -> Vec<&'static ToolSpec> {
        TOOL_PODS.iter().flat_map(|pod| pod.tools.iter()).collect()
    }

    fn blank_state() -> EditorState {
        let img = image::RgbaImage::from_raw(64, 48, vec![255; 64 * 48 * 4])
            .expect("raw buffer matches 64*48*4");
        EditorState::new(img)
    }

    fn rail_frame(
        ctx: &egui::Context,
        state: &mut EditorState,
        gate: ToolGate,
        rail: &mut RailState,
        events: Vec<egui::Event>,
    ) -> bool {
        let raw = egui::RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(640.0, 480.0),
            )),
            ..egui::RawInput::default()
        };
        let mut fired = false;
        let _ = ctx.run_ui(raw, |ui| {
            fired = show(ui, state, gate, rail);
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

    // --- Vocabulary (moved with the code from the panel rail) ---

    #[test]
    fn tool_pods_partition_every_variant_in_spec_order() {
        let expected: &[(&str, &[Tool])] = &[
            ("selection", &[Tool::Select, Tool::Hand, Tool::Crop]),
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
            Tool::Hand,
            Tool::Crop,
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
        // Select leads the selection pod, Hand seconds, Crop thirds.
        let flat: Vec<Tool> = TOOL_PODS
            .iter()
            .flat_map(|p| p.tools.iter().map(|t| t.tool))
            .collect();
        assert_eq!(flat.first(), Some(&Tool::Select));
        assert_eq!(flat.get(1), Some(&Tool::Hand));
        assert_eq!(flat.get(2), Some(&Tool::Crop));
    }

    #[test]
    fn select_leads_the_selection_pod() {
        let flat = flat();
        assert!(
            flat.first().is_some_and(|t| t.tool == Tool::Select),
            "Select must be the first rail chip"
        );
        assert_eq!(flat[0].name, "Select", "the label reads Select");
        let keys: Vec<&str> = flat.iter().map(|t| t.key).collect();
        assert_eq!(
            &keys[..3],
            &["V", "H", "C"],
            "the selection pod maps to V / H / C"
        );
    }

    #[test]
    fn every_tool_glyph_fn_is_unique() {
        let mut fns: Vec<&'static str> = Vec::new();
        for (i, spec) in flat().iter().enumerate() {
            // Distinct fns: compare fn pointers pairwise.
            for (j, other) in flat().iter().enumerate() {
                if i != j {
                    assert_ne!(
                        spec.glyph as usize, other.glyph as usize,
                        "tools {i} and {j} share a glyph fn"
                    );
                }
            }
            fns.push(spec.key);
        }
        assert_eq!(fns.len(), 12, "the full vocabulary rides the rail");
    }

    #[test]
    fn tooltip_text_carries_name_key_and_lock_clause() {
        let pen = &TOOL_PODS[2].tools[0]; // Pencil
        assert_eq!(
            tooltip_text(pen, ToolGate::All),
            format!("{} · {}", pen.name, pen.key)
        );
        assert!(
            tooltip_text(pen, ToolGate::CropOnly).contains(LOCK_CLAUSE),
            "gated-off tools expose the lock clause"
        );
    }

    // --- Gate clicks (moved with the code from the panel rail) ---

    fn tool_after_pen_click(gate: ToolGate) -> Tool {
        let mut state = blank_state();
        state.tool = Tool::Crop;
        let ctx = egui::Context::default();
        let mut rail = RailState::default();
        rail_frame(&ctx, &mut state, gate, &mut rail, vec![]);
        let pen = ctx
            .read_response(egui::Id::new(("rail-tool", "P")))
            .expect("pass 1 registered the Pencil chip");
        rail_frame(
            &ctx,
            &mut state,
            gate,
            &mut rail,
            click_events(pen.rect.center()),
        );
        state.tool
    }

    #[test]
    fn crop_only_click_cannot_switch_tool() {
        assert_eq!(
            tool_after_pen_click(ToolGate::CropOnly),
            Tool::Crop,
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

    // --- Rail geometry + styling ---

    /// The rail is a left strip: the central panel starts at its
    /// 56 px edge, full height.
    #[test]
    fn rail_is_a_56px_left_strip() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut state = blank_state();
        let mut rail = RailState::default();
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(640.0, 480.0),
            )),
            ..egui::RawInput::default()
        };
        let central_left = std::cell::Cell::new(0.0_f32);
        let _ = ctx.run_ui(raw, |ui| {
            show(ui, &mut state, ToolGate::All, &mut rail);
            let central = egui::CentralPanel::default().show(ui, |ui| {
                ui.allocate_space(egui::vec2(1.0, 1.0));
            });
            central_left.set(central.response.rect.left());
        });
        assert!(
            (central_left.get() - RAIL_WIDTH).abs() < 1.0,
            "the rail strips {} px off the left, central left {}",
            RAIL_WIDTH,
            central_left.get()
        );
    }

    /// Rail metrics pin the density bump (spec table: width 56,
    /// chip 40).
    #[test]
    fn rail_metrics_pin_the_density_bump() {
        assert_eq!(RAIL_WIDTH.to_bits(), 56.0_f32.to_bits());
        assert_eq!(RAIL_TOOL.to_bits(), 40.0_f32.to_bits());
    }

    /// All ten tools render as chips and a click on each switches
    /// to it (under the open gate).
    #[test]
    fn every_tool_chip_renders_and_clicks() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut state = blank_state();
        let mut rail = RailState::default();
        rail_frame(&ctx, &mut state, ToolGate::All, &mut rail, vec![]);
        for spec in flat() {
            let chip = ctx
                .read_response(egui::Id::new(("rail-tool", spec.key)))
                .unwrap_or_else(|| panic!("{} chip renders", spec.key));
            let before = state.tool;
            rail_frame(
                &ctx,
                &mut state,
                ToolGate::All,
                &mut rail,
                click_events(chip.rect.center()),
            );
            assert_eq!(state.tool, spec.tool, "{} chip switches", spec.key);
            if spec.tool != before {
                break;
            }
        }
    }

    /// The active tool paints a gold chip with a cream border and a
    /// pop shadow behind it.
    #[test]
    fn active_tool_paints_gold_with_cream_border() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut state = blank_state();
        state.set_tool(Tool::Pencil);
        let mut rail = RailState::default();
        // Run a few frames so the bounce settles at scale 1.
        for i in 0..40 {
            let raw = egui::RawInput {
                time: Some(f64::from(i) * 0.016),
                predicted_dt: 1.0 / 60.0,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(640.0, 480.0),
                )),
                ..egui::RawInput::default()
            };
            let _ = ctx.run_ui(raw, |ui| {
                show(ui, &mut state, ToolGate::All, &mut rail);
            });
        }
        let shapes = ctx
            .run_ui(
                egui::RawInput {
                    time: Some(2.4),
                    predicted_dt: 1.0 / 60.0,
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(640.0, 480.0),
                    )),
                    ..egui::RawInput::default()
                },
                |ui| {
                    show(ui, &mut state, ToolGate::All, &mut rail);
                },
            )
            .shapes;
        let gold = shapes.iter().any(|cs| {
            matches!(&cs.shape, egui::Shape::Rect(r)
                if r.fill == theme::palette::GOLD
                    && r.stroke.color == theme::palette::PAPER_CREAM)
        });
        assert!(gold, "the active chip paints gold with a cream border");
    }

    /// Gated-off chips render (stable layout) but never switch.
    #[test]
    fn gated_tools_stay_visible_but_inert() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut state = blank_state();
        let mut rail = RailState::default();
        rail_frame(&ctx, &mut state, ToolGate::CropOnly, &mut rail, vec![]);
        for spec in flat() {
            assert!(
                ctx.read_response(egui::Id::new(("rail-tool", spec.key)))
                    .is_some(),
                "{} stays visible under the gate",
                spec.key
            );
        }
        let pen = ctx
            .read_response(egui::Id::new(("rail-tool", "P")))
            .expect("pencil chip");
        rail_frame(
            &ctx,
            &mut state,
            ToolGate::CropOnly,
            &mut rail,
            click_events(pen.rect.center()),
        );
        assert_ne!(state.tool, Tool::Pencil, "gated click cannot switch");
    }

    /// The Ask-the-shot spark chip is an action, not a tool: it
    /// fires (show returns `true`) without touching `state.tool`,
    /// under any gate (never lock-grey — it is not a `Tool`).
    #[test]
    fn spark_chip_fires_without_touching_the_tool() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        widgets::set_animations(&ctx, false);
        let mut state = blank_state();
        state.set_tool(Tool::Rect);
        let mut rail = RailState::default();
        // The rail is tall — the spark pod sits below the tool pods,
        // so the frame needs headroom beyond the 640×480 default.
        let mut frame = |events: Vec<egui::Event>| {
            let raw = egui::RawInput {
                events,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(640.0, 800.0),
                )),
                ..egui::RawInput::default()
            };
            let mut fired = false;
            let _ = ctx.run_ui(raw, |ui| {
                fired = show(ui, &mut state, ToolGate::CropOnly, &mut rail);
            });
            fired
        };
        assert!(!frame(vec![]), "no click, no fire");
        let spark = ctx
            .read_response(egui::Id::new("rail-ai"))
            .expect("the spark chip registers");
        assert!(
            spark.rect.bottom() < 800.0 && spark.rect.top() > 0.0,
            "the spark sits on-screen: {:?}",
            spark.rect
        );
        let fired = frame(click_events(spark.rect.center()));
        assert!(fired, "the spark chip fires even under CropOnly");
        assert_eq!(state.tool, Tool::Rect, "the tool never changes");
    }

    /// Switching tools bounces the active chip through the spring;
    /// with the animation gate off it lands at rest immediately.
    #[test]
    fn tool_pop_bounces_and_the_gate_jumps_it() {
        // Gate on: the selected chip starts small and grows.
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut state = blank_state();
        state.set_tool(Tool::Pencil);
        let mut rail = RailState::default();
        let mut sizes = Vec::new();
        for i in 0..30 {
            let raw = egui::RawInput {
                time: Some(f64::from(i) * 0.016),
                predicted_dt: 1.0 / 60.0,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(640.0, 480.0),
                )),
                ..egui::RawInput::default()
            };
            let shapes = ctx
                .run_ui(raw, |ui| {
                    show(ui, &mut state, ToolGate::All, &mut rail);
                })
                .shapes;
            let size = shapes.iter().find_map(|cs| match &cs.shape {
                egui::Shape::Rect(r) if r.fill == theme::palette::GOLD => Some(r.rect.width()),
                _ => None,
            });
            if let Some(s) = size {
                sizes.push(s);
            }
        }
        assert!(sizes.len() > 3, "the gold chip paints across frames");
        assert!(
            sizes[0] < sizes[sizes.len() - 1],
            "the chip bounces: first {} last {}",
            sizes[0],
            sizes[sizes.len() - 1]
        );

        // Gate off: full size on the first frame.
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        widgets::set_animations(&ctx, false);
        let mut state = blank_state();
        state.set_tool(Tool::Pencil);
        let mut rail = RailState::default();
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(640.0, 480.0),
            )),
            ..egui::RawInput::default()
        };
        let shapes = ctx
            .run_ui(raw, |ui| {
                show(ui, &mut state, ToolGate::All, &mut rail);
            })
            .shapes;
        let size = shapes.iter().find_map(|cs| match &cs.shape {
            egui::Shape::Rect(r) if r.fill == theme::palette::GOLD => Some(r.rect.width()),
            _ => None,
        });
        assert!(
            size.is_some_and(|s| (s - RAIL_TOOL).abs() < 1.0),
            "gate off: the chip paints at rest size, got {size:?}"
        );
    }
}
