//! Floating on-canvas controls — the editor's quick cluster.
//!
//! A small cutout-chip row pinned to the canvas's top-left corner:
//! undo / redo, the magnifier always-on toggle, and the loupe's
//! zoom in / out. The chips float *over* the screenshot (they are
//! canvas chrome, not panel furniture), so [`show`] also publishes
//! the cluster's rect — the shell must treat presses that begin
//! there as non-canvas (see `crate::app`'s pointer gating), while
//! every pointer-miss passes through to annotation gestures
//! untouched.

use crate::magnifier::Magnifier;
use crate::panel;
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
    /// Step the loupe's zoom factor up one stop.
    LoupeZoomIn,
    /// Step the loupe's zoom factor down one stop.
    LoupeZoomOut,
}

/// Draw the cluster over `canvas_rect`. Returns the fired action
/// (if any) and the chip rects for the shell's press gating — chip
/// rects only: a pointer-miss between chips passes through to the
/// canvas (the spec's passthrough contract).
pub(crate) fn show(
    ctx: &egui::Context,
    canvas_rect: egui::Rect,
    magnifier_always_on: bool,
    magnifier: &mut Magnifier,
) -> (Option<FloatingAction>, Vec<egui::Rect>) {
    let mut action = None;
    let mut chip_rects = Vec::new();
    let anchor = canvas_rect.left_top() + egui::vec2(CLUSTER_MARGIN, CLUSTER_MARGIN);
    egui::Area::new(egui::Id::new("floating_controls"))
        .order(egui::Order::Middle)
        .fixed_pos(anchor)
        .show(ctx, |ui| {
            panel::pod_island(ui, |ui| {
                let mut chip = |id: &'static str,
                                tooltip: &str,
                                selected: bool,
                                glyph: panel::Glyph,
                                on_click: Option<FloatingAction>| {
                    let resp = panel::chip(ui, egui::Id::new(id), tooltip, selected, glyph, true);
                    chip_rects.push(resp.rect);
                    if resp.clicked() {
                        if let Some(a) = on_click {
                            action = Some(a);
                        }
                    }
                };
                chip(
                    "undo",
                    "Undo · Ctrl+Z",
                    false,
                    panel::g_undo,
                    Some(FloatingAction::Undo),
                );
                chip(
                    "redo",
                    "Redo · Ctrl+Y",
                    false,
                    panel::g_redo,
                    Some(FloatingAction::Redo),
                );
                chip(
                    "magnifier-toggle",
                    "Magnifier always on",
                    magnifier_always_on,
                    g_magnifier,
                    Some(FloatingAction::ToggleMagnifier),
                );
                chip(
                    "zoom-out",
                    "Loupe zoom out",
                    false,
                    g_minus,
                    Some(FloatingAction::LoupeZoomOut),
                );
                chip(
                    "zoom-in",
                    "Loupe zoom in",
                    false,
                    panel::g_plus,
                    Some(FloatingAction::LoupeZoomIn),
                );
                // The zoom clicks step the factor right after the
                // generic bookkeeping above.
                if action == Some(FloatingAction::LoupeZoomIn) {
                    magnifier.zoom_in();
                }
                if action == Some(FloatingAction::LoupeZoomOut) {
                    magnifier.zoom_out();
                }
            });
            ui.min_rect()
        });
    (action, chip_rects)
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

// Keep the theme import for the (currently token-only) dressing
// shared with the panel's chips.
const _: () = {
    let _ = theme::POD_GAP;
};

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas(w: f32, h: f32) -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(w, h))
    }

    /// One frame of the cluster over a roomy canvas; returns the
    /// context for response reads.
    fn frame(events: Vec<egui::Event>, always_on: bool, mag: &mut Magnifier) -> egui::Context {
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            events,
            screen_rect: Some(canvas(800.0, 600.0)),
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            let _ = show(ui.ctx(), canvas(800.0, 600.0), always_on, mag);
        });
        ctx
    }

    /// Lay out once, then click the chip with `id`: the click
    /// needs the pointer's hover to have settled in a prior pass
    /// (Area-hosted widgets hit-test a pass behind their layout).
    /// All passes share one context — `frame` above constructs a
    /// fresh one per call, so settle passes run inline.
    fn action_after_click_on(id: &str, always_on: bool) -> (Option<FloatingAction>, Magnifier) {
        let mut mag = Magnifier::new();
        let ctx = frame(vec![], always_on, &mut mag);
        let chip = ctx
            .read_response(egui::Id::new(id))
            .unwrap_or_else(|| panic!("pass 1 registered {id}"));
        let at = chip.rect.center();
        let pass = |ctx: &egui::Context, events: Vec<egui::Event>, mag: &mut Magnifier| {
            let raw = egui::RawInput {
                events,
                screen_rect: Some(canvas(800.0, 600.0)),
                ..egui::RawInput::default()
            };
            let _ = ctx.run_ui(raw, |ui| {
                let _ = show(ui.ctx(), canvas(800.0, 600.0), always_on, mag);
            });
        };
        // Hover-settle: pointer at the chip for one pass, then the
        // press + release clicks.
        pass(&ctx, vec![egui::Event::PointerMoved(at)], &mut mag);
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
            let (a, _) = show(ui.ctx(), canvas(800.0, 600.0), always_on, &mut mag);
            action = a;
        });
        (action, mag)
    }

    #[test]
    fn undo_chip_fires_undo() {
        assert_eq!(
            action_after_click_on("undo", false).0,
            Some(FloatingAction::Undo)
        );
    }

    #[test]
    fn redo_chip_fires_redo() {
        assert_eq!(
            action_after_click_on("redo", false).0,
            Some(FloatingAction::Redo)
        );
    }

    #[test]
    fn magnifier_chip_fires_the_toggle() {
        assert_eq!(
            action_after_click_on("magnifier-toggle", false).0,
            Some(FloatingAction::ToggleMagnifier)
        );
    }

    #[test]
    fn zoom_chips_step_the_loupe_factor() {
        let base = Magnifier::new().zoom();
        let (action, mag) = action_after_click_on("zoom-in", false);
        assert_eq!(action, Some(FloatingAction::LoupeZoomIn));
        assert!(mag.zoom() > base, "one click steps the loupe up");
        let (action, mag) = action_after_click_on("zoom-out", false);
        assert_eq!(action, Some(FloatingAction::LoupeZoomOut));
        assert!(
            mag.zoom() < base,
            "one click steps the loupe down from the base factor"
        );
        assert!(mag.zoom() >= 2.0, "never below the usable range");
    }

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

    /// The cluster publishes its chip rects — the shell's press
    /// gate consumes them (a chip press must never start a canvas
    /// gesture; misses between chips pass through).
    #[test]
    fn show_publishes_chip_rects_inside_the_canvas() {
        let mut mag = Magnifier::new();
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            screen_rect: Some(canvas(800.0, 600.0)),
            ..egui::RawInput::default()
        };
        let mut chips = Vec::new();
        let _ = ctx.run_ui(raw, |ui| {
            let (_, rects) = show(ui.ctx(), canvas(800.0, 600.0), false, &mut mag);
            chips = rects;
        });
        assert_eq!(chips.len(), 5, "one rect per chip");
        for chip in &chips {
            assert!(chip.width() > 0.0 && chip.height() > 0.0);
            assert!(
                canvas(800.0, 600.0).contains_rect(*chip),
                "every chip must stay inside the canvas rect"
            );
        }
    }
}
