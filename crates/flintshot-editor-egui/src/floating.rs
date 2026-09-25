//! Floating on-canvas controls — the editor's quick cluster.
//!
//! A small cutout-chip row pinned to the canvas's top-left corner:
//! undo / redo, the magnifier always-on toggle, and the canvas
//! view's zoom in / out (the shell applies the factor to the fit
//! transform). While the side panel is collapsed, a « expand chip
//! pins to the canvas's top-right corner — the panel's visible
//! reopen affordance. The chips float *over* the screenshot (they
//! are canvas chrome, not panel furniture), so [`show`] also
//! publishes the cluster's rect — the shell must treat presses
//! that begin there as non-canvas (see `crate::app`'s pointer
//! gating), while every pointer-miss passes through to annotation
//! gestures untouched.

use crate::panel;
use crate::rail;
use crate::theme;

/// Margin between the canvas's corner and the cluster, logical px
/// (two spacing-scale steps off the image edge).
const CLUSTER_MARGIN: f32 = 8.0;

/// Which floating chip fired this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FloatingAction {
    /// Undo — replays a Ctrl+Z dispatch.
    Undo,
    /// Redo — replays a Ctrl+Y dispatch.
    Redo,
    /// Flip the magnifier's always-on flag.
    ToggleMagnifier,
    /// Step the canvas view's zoom factor up one stop.
    ZoomIn,
    /// Step the canvas view's zoom factor down one stop.
    ZoomOut,
    /// Reopen the collapsed side panel (the « chip at the canvas's
    /// top-right corner; the shell owns the panel state).
    ExpandPanel,
}

/// Which canvas corner a cluster pins to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Corner {
    /// Inside the top-left edge.
    TopLeft,
    /// Inside the top-right edge.
    TopRight,
    /// Inside the bottom-right edge.
    BottomRight,
}

/// The pin position for a cluster of `size` in `canvas_rect` at
/// `corner` — the cluster's top-left, `CLUSTER_MARGIN` inside the
/// canvas edge. Pure so the mockup arrangement is unit-testable.
pub(crate) fn corner_anchor(
    canvas_rect: egui::Rect,
    size: egui::Vec2,
    corner: Corner,
) -> egui::Pos2 {
    match corner {
        Corner::TopLeft => canvas_rect.left_top() + egui::vec2(CLUSTER_MARGIN, CLUSTER_MARGIN),
        Corner::TopRight => egui::pos2(
            canvas_rect.right_top().x - CLUSTER_MARGIN - size.x,
            canvas_rect.right_top().y + CLUSTER_MARGIN,
        ),
        Corner::BottomRight => {
            canvas_rect.right_bottom() - egui::vec2(CLUSTER_MARGIN, CLUSTER_MARGIN) - size
        }
    }
}

/// A floating chip's outer size (the 28 px glyph square plus the
/// cream cutout's padding).
const CHIP_SIZE: f32 = 28.0;

fn run_cluster(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    chip_rects: &mut Vec<egui::Rect>,
    action: &mut Option<FloatingAction>,
    chips: Vec<(
        &'static str,
        &'static str,
        bool,
        rail::Glyph,
        Option<FloatingAction>,
    )>,
) {
    let mut island = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    panel::pod_island(&mut island, |ui| {
        for (id, tooltip, selected, glyph, on_click) in chips {
            let resp = cutout_chip(ui, egui::Id::new(id), tooltip, selected, glyph);
            chip_rects.push(resp.rect);
            if resp.clicked() {
                if let Some(a) = on_click {
                    *action = Some(a);
                }
            }
        }
    });
}

/// Draw the floating clusters over `canvas_rect`: undo / redo at
/// the top-left corner, the magnifier toggle + view zoom chips at
/// the bottom-right, and — while `panel_collapsed` — the side
/// panel's expand chip at the top-right (the reopen affordance;
/// the mockup's arrangement). Returns the fired action (if any)
/// and the chip rects for the shell's press gating — chip rects
/// only: a pointer-miss between chips passes through to the
/// canvas (the spec's passthrough contract). The zoom and expand
/// actions are the shell's to apply (the fit transform and the
/// panel state live there).
pub(crate) fn show(
    ctx: &egui::Context,
    canvas_rect: egui::Rect,
    magnifier_always_on: bool,
    panel_collapsed: bool,
) -> (Option<FloatingAction>, Vec<egui::Rect>) {
    let mut action = None;
    let mut chip_rects = Vec::new();

    // One full-canvas, click-transparent area hosts both clusters:
    // the history cluster at the top-left corner, the magnifier
    // cluster at the bottom-right (the mockup's arrangement). Child
    // UIs pin the islands to their corner rects; the area's
    // background takes no sense, so pointer misses pass through to
    // the canvas (the spec's passthrough contract). The corner
    // offsets are the same math as [`corner_anchor`], with the
    // island sizes derived from the fixed chip grid.
    let history_size = egui::vec2(
        2.0 * CHIP_SIZE + theme::SPACE_SCALE[1] + 2.0 * f32::from(theme::POD_PAD),
        CHIP_SIZE + 2.0 * f32::from(theme::POD_PAD),
    );
    let mag_size = egui::vec2(
        3.0 * CHIP_SIZE + 2.0 * theme::SPACE_SCALE[1] + 2.0 * f32::from(theme::POD_PAD),
        CHIP_SIZE + 2.0 * f32::from(theme::POD_PAD),
    );
    let expand_size = egui::vec2(
        CHIP_SIZE + 2.0 * f32::from(theme::POD_PAD),
        CHIP_SIZE + 2.0 * f32::from(theme::POD_PAD),
    );
    egui::Area::new(egui::Id::new("floating_controls"))
        .order(egui::Order::Middle)
        .constrain(true)
        .fixed_pos(canvas_rect.min)
        .show(ctx, |ui| {
            ui.set_max_size(canvas_rect.size());
            let history_rect = egui::Rect::from_min_size(
                corner_anchor(canvas_rect, history_size, Corner::TopLeft),
                history_size,
            );
            let mag_rect = egui::Rect::from_min_size(
                corner_anchor(canvas_rect, mag_size, Corner::BottomRight),
                mag_size,
            );
            run_cluster(
                ui,
                history_rect,
                &mut chip_rects,
                &mut action,
                vec![
                    (
                        "undo",
                        "Undo · Ctrl+Z",
                        false,
                        panel::g_undo,
                        Some(FloatingAction::Undo),
                    ),
                    (
                        "redo",
                        "Redo · Ctrl+Y",
                        false,
                        panel::g_redo,
                        Some(FloatingAction::Redo),
                    ),
                ],
            );
            run_cluster(
                ui,
                mag_rect,
                &mut chip_rects,
                &mut action,
                vec![
                    (
                        "magnifier-toggle",
                        "Magnifier always on",
                        magnifier_always_on,
                        g_magnifier,
                        Some(FloatingAction::ToggleMagnifier),
                    ),
                    (
                        "zoom-out",
                        "Zoom out · scroll wheel",
                        false,
                        g_minus,
                        Some(FloatingAction::ZoomOut),
                    ),
                    (
                        "zoom-in",
                        "Zoom in · scroll wheel",
                        false,
                        panel::g_plus,
                        Some(FloatingAction::ZoomIn),
                    ),
                ],
            );
            if panel_collapsed {
                let expand_rect = egui::Rect::from_min_size(
                    corner_anchor(canvas_rect, expand_size, Corner::TopRight),
                    expand_size,
                );
                run_cluster(
                    ui,
                    expand_rect,
                    &mut chip_rects,
                    &mut action,
                    vec![(
                        "expand-panel",
                        "Show panel · Ctrl+B",
                        false,
                        panel::g_panel_expand,
                        Some(FloatingAction::ExpandPanel),
                    )],
                );
            }
        });

    (action, chip_rects)
}

/// A cream cutout chip: white inner outline on a cream square, a
/// gold-tinted hover that pops (the rest-level shadow deepens), and
/// the forest-selected active state for toggles.
fn cutout_chip(
    ui: &mut egui::Ui,
    id: egui::Id,
    tooltip: &str,
    selected: bool,
    glyph: rail::Glyph,
) -> egui::Response {
    use crate::theme::palette as p;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(CHIP_SIZE, CHIP_SIZE), egui::Sense::hover());
    let resp = ui
        .interact(rect, id, egui::Sense::click())
        .on_hover_text(tooltip);
    let radius = egui::CornerRadius::same(crate::theme::POD_RADIUS);
    // Pop on hover: the resting shadow deepens to lift.
    ui.painter().add(if resp.hovered() || selected {
        theme::SHADOW_LIFT.as_shape(rect, radius)
    } else {
        theme::SHADOW_RESTING.as_shape(rect, radius)
    });
    ui.painter().rect(
        rect,
        radius,
        if selected {
            p::with_alpha(p::GOLD, 60)
        } else {
            p::PAPER_CREAM
        },
        egui::Stroke::NONE,
        egui::StrokeKind::Middle,
    );
    // The white inner outline, 2 px inset.
    let inner = rect.shrink(2.0);
    ui.painter().rect(
        inner,
        radius,
        p::PAPER_CREAM,
        egui::Stroke::new(1.5, p::PAPER),
        egui::StrokeKind::Inside,
    );
    let ink = if selected { p::FOREST_INK } else { p::INK };
    glyph(ui.painter(), rect, ink);
    resp
}

/// A loupe glyph: circle with a handle — the magnifier's own icon.
fn g_magnifier(p: &egui::Painter, r: egui::Rect, c: egui::Color32) {
    let centre = egui::pos2(r.center().x - 1.5, r.center().y - 1.5);
    p.circle_stroke(centre, 4.5, egui::Stroke::new(2.0, c));
    p.line_segment(
        [centre + egui::vec2(3.2, 3.2), pt(r, 6.0, 6.0)],
        egui::Stroke::new(2.0, c),
    );
}

/// Glyph-space point: `(0,0)` centre, `±7` edges, +Y down (the
/// toolbar glyph coordinate system).
fn pt(rect: egui::Rect, x: f32, y: f32) -> egui::Pos2 {
    egui::pos2(rect.center().x + x, rect.center().y + y)
}

/// A minus glyph (the zoom-out stop).
fn g_minus(p: &egui::Painter, r: egui::Rect, c: egui::Color32) {
    p.line_segment(
        [pt(r, -5.0, 0.0), pt(r, 5.0, 0.0)],
        egui::Stroke::new(2.0, c),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::magnifier::Magnifier;

    fn canvas(w: f32, h: f32) -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(w, h))
    }

    /// One frame of the cluster over a roomy canvas; returns the
    /// context for response reads.
    fn frame(events: Vec<egui::Event>, always_on: bool, panel_collapsed: bool) -> egui::Context {
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            events,
            screen_rect: Some(canvas(800.0, 600.0)),
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            let _ = show(ui.ctx(), canvas(800.0, 600.0), always_on, panel_collapsed);
        });
        ctx
    }

    /// Lay out once, then click the chip with `id`: the click
    /// needs the pointer's hover to have settled in a prior pass
    /// (Area-hosted widgets hit-test a pass behind their layout).
    /// All passes share one context — `frame` above constructs a
    /// fresh one per call, so settle passes run inline.
    fn action_after_click_on(
        id: &str,
        always_on: bool,
        panel_collapsed: bool,
    ) -> Option<FloatingAction> {
        let ctx = frame(vec![], always_on, panel_collapsed);
        // Settle passes first, THEN read the settled rect: Areas
        // place on the sizing pass and pin on the next.
        let raw = egui::RawInput {
            screen_rect: Some(canvas(800.0, 600.0)),
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            let _ = show(ui.ctx(), canvas(800.0, 600.0), always_on, panel_collapsed);
        });
        let chip = ctx
            .read_response(egui::Id::new(id))
            .unwrap_or_else(|| panic!("pass 1 registered {id}"));
        let at = chip.rect.center();
        let pass = |ctx: &egui::Context, events: Vec<egui::Event>| {
            let raw = egui::RawInput {
                events,
                screen_rect: Some(canvas(800.0, 600.0)),
                ..egui::RawInput::default()
            };
            let _ = ctx.run_ui(raw, |ui| {
                let _ = show(ui.ctx(), canvas(800.0, 600.0), always_on, panel_collapsed);
            });
        };
        // Settle passes: Areas place on the sizing pass and pin on
        // the next, so two passes precede the pointer read.
        pass(&ctx, vec![]);
        pass(&ctx, vec![]);
        // Hover-settle: pointer at the chip for one pass, then the
        // press + release clicks.
        pass(&ctx, vec![egui::Event::PointerMoved(at)]);
        let mut action = None;
        let raw = egui::RawInput {
            events: vec![
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
            screen_rect: Some(canvas(800.0, 600.0)),
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            let (a, _) = show(ui.ctx(), canvas(800.0, 600.0), always_on, panel_collapsed);
            action = a;
        });
        action
    }

    #[test]
    fn undo_chip_fires_undo() {
        assert_eq!(
            action_after_click_on("undo", false, false),
            Some(FloatingAction::Undo)
        );
    }

    #[test]
    fn redo_chip_fires_redo() {
        assert_eq!(
            action_after_click_on("redo", false, false),
            Some(FloatingAction::Redo)
        );
    }

    #[test]
    fn undo_chip_reports_hover() {
        let ctx = frame(vec![], false, false);
        // Settle passes: the Area places on the sizing pass and pins
        // on the next, so the chip rect read here is the final one.
        for _ in 0..2 {
            let _ = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(canvas(800.0, 600.0)),
                    ..egui::RawInput::default()
                },
                |ui| {
                    let _ = show(ui.ctx(), canvas(800.0, 600.0), false, false);
                },
            );
        }
        let chip = ctx.read_response(egui::Id::new("undo")).expect("undo");
        let at = chip.rect.center();
        let raw = egui::RawInput {
            events: vec![egui::Event::PointerMoved(at)],
            screen_rect: Some(canvas(800.0, 600.0)),
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            let _ = show(ui.ctx(), canvas(800.0, 600.0), false, false);
        });
        let after = ctx
            .read_response(egui::Id::new("undo"))
            .expect("undo chip persists");
        assert!(after.hovered(), "the pointer sits on the undo chip");
    }

    #[test]
    fn magnifier_chip_fires_the_toggle() {
        assert_eq!(
            action_after_click_on("magnifier-toggle", false, false),
            Some(FloatingAction::ToggleMagnifier)
        );
    }

    /// The zoom chips report view-zoom actions — the shell's fit
    /// transform applies them (the cluster itself owns no state).
    #[test]
    fn zoom_chips_report_view_zoom_actions() {
        assert_eq!(
            action_after_click_on("zoom-in", false, false),
            Some(FloatingAction::ZoomIn)
        );
        assert_eq!(
            action_after_click_on("zoom-out", false, false),
            Some(FloatingAction::ZoomOut)
        );
    }

    /// The loupe's own factor still clamps at both ends (the
    /// magnifier inset keeps its API; the view zoom replaced only
    /// the chips' role).
    #[test]
    fn loupe_zoom_clamps_at_both_ends() {
        let mut mag = Magnifier::new();
        for _ in 0..64 {
            mag.zoom_in();
        }
        let top = mag.zoom();
        for _ in 0..64 {
            mag.zoom_out();
        }
        assert!(top > mag.zoom(), "the factor must move down again");
        for _ in 0..64 {
            mag.zoom_out();
        }
        assert!(mag.zoom() >= 1.0, "never degenerate");
    }

    /// The expand chip fires while the panel is collapsed — the
    /// sidebar-collapse spec's deferred reopen affordance.
    #[test]
    fn expand_chip_fires_expand_panel_while_collapsed() {
        assert_eq!(
            action_after_click_on("expand-panel", false, true),
            Some(FloatingAction::ExpandPanel)
        );
    }

    /// The cluster publishes its chip rects — the shell's press
    /// gate consumes them (a chip press must never start a canvas
    /// gesture; misses between chips pass through).
    /// The corner-anchor math: top-left pins inside the canvas's
    /// top-left edge; bottom-right pins inside the bottom-right edge
    /// minus the cluster's own size.
    #[test]
    fn corner_anchor_pins_each_corner() {
        let canvas = canvas(800.0, 600.0);
        let size = egui::vec2(112.0, 40.0);
        let tl = corner_anchor(canvas, size, Corner::TopLeft);
        assert_eq!(tl, egui::pos2(8.0, 8.0), "top-left: margin inside");
        let br = corner_anchor(canvas, size, Corner::BottomRight);
        assert_eq!(
            br,
            egui::pos2(800.0 - 8.0 - 112.0, 600.0 - 8.0 - 40.0),
            "bottom-right: margin inside minus the cluster's size"
        );
        let tr = corner_anchor(canvas, size, Corner::TopRight);
        assert_eq!(
            tr,
            egui::pos2(800.0 - 8.0 - 112.0, 8.0),
            "top-right: margin inside, flush with the top band"
        );
    }

    #[test]
    fn show_publishes_chip_rects_inside_the_canvas() {
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            screen_rect: Some(canvas(800.0, 600.0)),
            ..egui::RawInput::default()
        };
        let mut chips = Vec::new();
        let _ = ctx.run_ui(raw, |ui| {
            let (_, rects) = show(ui.ctx(), canvas(800.0, 600.0), false, false);
            chips = rects;
        });
        assert_eq!(chips.len(), 5, "one rect per chip, no expand chip expanded");
        for chip in &chips {
            assert!(chip.width() > 0.0 && chip.height() > 0.0);
            assert!(
                canvas(800.0, 600.0).contains_rect(*chip),
                "every chip must stay inside the canvas rect"
            );
        }
    }

    /// Collapsed, the sixth chip joins the press-gate set — pinned
    /// to the canvas's top-right corner, clear of the top-left
    /// history cluster.
    #[test]
    fn collapsed_panel_adds_the_expand_chip_at_the_top_right() {
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            screen_rect: Some(canvas(800.0, 600.0)),
            ..egui::RawInput::default()
        };
        let mut chips = Vec::new();
        let _ = ctx.run_ui(raw, |ui| {
            let (_, rects) = show(ui.ctx(), canvas(800.0, 600.0), false, true);
            chips = rects;
        });
        assert_eq!(chips.len(), 6, "the expand chip joins the set");
        let expand = ctx
            .read_response(egui::Id::new("expand-panel"))
            .expect("the expand chip registers");
        assert!(
            canvas(800.0, 600.0).contains_rect(expand.rect),
            "the expand chip stays inside the canvas rect"
        );
        let undo = ctx
            .read_response(egui::Id::new("undo"))
            .expect("the undo chip registers");
        assert!(
            expand.rect.left() > undo.rect.right(),
            "top-right chip clears the top-left cluster: {:?} vs {:?}",
            expand.rect,
            undo.rect
        );
    }
}
