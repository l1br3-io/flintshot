//! The statusbar — the 34 px forest footer under the editor's five
//! zones (worker editor windows and routed editor pages alike; home
//! and settings have none, per the mockups).
//!
//! Left: `Image: {w}×{h} px · {size}` in cream-bold, the size an
//! RGBA heuristic estimate. Center: the selected tool's hint in
//! gold (from the engine's `tool_hint`). Right: the real kbd chips
//! (undo / redo / save / delete) via [`crate::widgets::kbd_chip`].

use crate::theme;
use crate::widgets;
use flintshot_core::Tool;

/// Fixed statusbar height, logical px — the mockup's zone contract,
/// tall enough for the `TYPE_SM` readouts the density bump carries.
pub(crate) const STATUSBAR_HEIGHT: f32 = 34.0;

// The size estimate lives in `flintshot_core::estimate_size_bytes`
// (heuristic `w × h × 4` or an exact PNG encode), selected by the
// `ui.statusbar-size-mode` setting; this module formats it.
/// Humanized byte count: whole B below a KiB, one-decimal KiB below
/// a MiB, one-decimal MiB above. Pure for tests.
pub(crate) fn humanize(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    #[expect(
        clippy::cast_precision_loss,
        reason = "byte counts ≤ 2^53 are exact in f64; display rounding only"
    )]
    let b = bytes as f64;
    if b < KIB {
        format!("{bytes} B")
    } else if b < MIB {
        format!("{:.1} KiB", b / KIB)
    } else {
        format!("{:.1} MiB", b / MIB)
    }
}

/// The `Image: {w}×{h} px · {size}` readout. Pure for tests.
pub(crate) fn dims_label(dims: (u32, u32), size_bytes: u64) -> String {
    format!("Image: {}×{} px · {}", dims.0, dims.1, humanize(size_bytes))
}

/// The statusbar's kbd chips, in the binding table's order: undo,
/// redo, save, delete, and the panel toggle (the keyboard reopen
/// path while collapsed — the floating « chip is the pointer one).
/// The order doubles as the responsive discard order: when the row
/// cannot fit, the tail yields first (see [`fitting_chips_prefix`]).
const KEYS: [&str; 5] = ["Ctrl+Z", "Ctrl+Y", "Ctrl+S", "Del", "Ctrl+B"];

/// How many of `widths`' leading chips fit `available` px at `gap`
/// spacing — the statusbar's progressive discard. The tail yields
/// first (the head is the binding table's importance order); a row
/// that cannot fit even the first chip keeps zero, never a
/// squeezed chip. Pure; unit-tested.
fn fitting_chips_prefix(widths: &[f32], available: f32, gap: f32) -> usize {
    let mut used = 0.0_f32;
    for (i, &w) in widths.iter().enumerate() {
        let next = if i == 0 { w } else { used + gap + w };
        if next > available + 0.5 {
            return i;
        }
        used = next;
    }
    widths.len()
}

/// Air the hint needs inside its span before it counts as fitting —
/// a galley butted against the chips reads as collision, not a
/// centre.
const HINT_SPAN_MARGIN: f32 = 16.0;

/// Draw the statusbar across the bottom of `ui`. Call it before the
/// side panels and the central panel (draw order: top → bottom →
/// left → right).
pub(crate) fn show(ui: &mut egui::Ui, dims: (u32, u32), tool: Tool, size_bytes: u64) {
    use crate::theme::palette as p;
    egui::Panel::bottom("flintshot_statusbar")
        .resizable(false)
        .exact_size(STATUSBAR_HEIGHT)
        .frame(
            egui::Frame::new()
                .fill(p::FOREST_DEEP)
                .inner_margin(egui::Margin::symmetric(14, 0)),
        )
        .show(ui, |ui| {
            ui.set_min_height(STATUSBAR_HEIGHT - 2.0);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                // Dims + estimated size, cream-bold.
                let galley = ui.painter().layout_no_wrap(
                    dims_label(dims, size_bytes),
                    egui::FontId::proportional(theme::TYPE_SM),
                    p::CREAM_TEXT,
                );
                let (rect, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
                ui.painter().galley(rect.min, galley, p::CREAM_TEXT);
                // The quiet dot separator.
                ui.painter().text(
                    ui.cursor().min,
                    egui::Align2::LEFT_CENTER,
                    "•",
                    egui::FontId::proportional(theme::TYPE_SM),
                    p::with_alpha(p::CREAM_TEXT, 115),
                );
                ui.add_space(12.0);
                // The chips' real footprint, measured before any of
                // them allocate (the fit path below decides how many
                // survive; the painted chips hit the cache).
                let gap = ui.spacing().item_spacing.x;
                let chip_galleys: Vec<_> = KEYS
                    .iter()
                    .map(|key| {
                        ui.painter().layout_no_wrap(
                            (*key).to_owned(),
                            egui::FontId::proportional(theme::TYPE_XS),
                            p::INK_SOFT,
                        )
                    })
                    .collect();
                let chip_widths: Vec<f32> = chip_galleys
                    .iter()
                    .map(|g| widgets::kbd_chip_width(g.size().x))
                    .collect();
                // The row's whole-chip footprint: chip widths plus
                // the gaps between them (the same accumulation
                // [`fitting_chips_prefix`] performs).
                let keys_width: f32 = chip_widths.iter().enumerate().fold(0.0, |acc, (i, w)| {
                    if i == 0 {
                        *w
                    } else {
                        acc + gap + w
                    }
                });
                // The per-tool hint, gold and centred in the leftover
                // span between the dims and the keys — but only when
                // the span can actually hold it: the galley is
                // no-wrap by design (one gold line), so painting it
                // into a smaller span would run it off the bar's
                // right edge (responsive-ui).
                let hint_galley = ui.painter().layout_no_wrap(
                    crate::panel::tool_hint(tool).to_owned(),
                    egui::FontId::proportional(theme::TYPE_SM),
                    p::GOLD,
                );
                // One boundary gap rides between the span and the
                // chip row (the layout transition consumes it), so
                // the chips' region lands at exactly `keys_width`.
                let span = (ui.available_width() - keys_width - gap).max(0.0);
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(span, STATUSBAR_HEIGHT - 4.0),
                    egui::Sense::hover(),
                );
                if hint_galley.size().x + HINT_SPAN_MARGIN <= span {
                    let x = rect.center().x - hint_galley.size().x / 2.0;
                    let y = rect.center().y - hint_galley.size().y / 2.0;
                    ui.painter().galley(egui::pos2(x, y), hint_galley, p::GOLD);
                }
                // The real binding chips, right-aligned — the
                // fitting prefix only (the tail yields first when
                // the row cannot afford all five).
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let keep = fitting_chips_prefix(&chip_widths, ui.available_width(), gap);
                    for key in KEYS.iter().take(keep) {
                        widgets::kbd_chip(ui, key);
                    }
                });
            });
            // The quiet top hairline, cream over forest.
            let rect = ui.max_rect();
            ui.painter().line_segment(
                [rect.left_top(), rect.right_top()],
                egui::Stroke::new(2.0, p::with_alpha(p::CREAM_TEXT, 36)),
            );
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::palette as p;

    /// The bar pins the density bump: 34 px tall, and the dims
    /// readout paints at `TYPE_SM` (the bigger readouts the taller
    /// bar exists to carry).
    #[test]
    fn statusbar_pins_the_density_bump() {
        assert_eq!(STATUSBAR_HEIGHT.to_bits(), 34.0_f32.to_bits());
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(640.0, 480.0),
            )),
            ..egui::RawInput::default()
        };
        let out = ctx.run_ui(raw, |ui| {
            show(ui, (1920, 1080), Tool::Pencil, 1920 * 1080 * 4);
        });
        let label = dims_label((1920, 1080), 1920 * 1080 * 4);
        let readout_size = out.shapes.iter().find_map(|cs| match &cs.shape {
            egui::Shape::Text(t) if t.galley.job.text == label => {
                t.galley.job.sections.first().map(|s| s.format.font_id.size)
            }
            _ => None,
        });
        assert_eq!(
            readout_size,
            Some(theme::TYPE_SM),
            "the dims readout paints at TYPE_SM"
        );
    }

    #[test]
    fn humanize_spans_the_b_kib_mib_boundaries() {
        assert_eq!(humanize(0), "0 B");
        assert_eq!(humanize(999), "999 B");
        assert_eq!(humanize(1023), "1023 B");
        assert_eq!(humanize(1024), "1.0 KiB");
        assert_eq!(humanize(15_725), "15.4 KiB");
        assert_eq!(humanize(1024 * 1024 - 1), "1024.0 KiB");
        assert_eq!(humanize(1024 * 1024), "1.0 MiB");
        assert_eq!(humanize(2_500_000), "2.4 MiB");
    }

    #[test]
    fn statusbar_estimate_matches_the_core_heuristic() {
        assert_eq!(
            flintshot_core::estimate_size_bytes((0, 0), flintshot_core::SizeMode::Heuristic, None),
            0
        );
        assert_eq!(
            flintshot_core::estimate_size_bytes(
                (1920, 1080),
                flintshot_core::SizeMode::Heuristic,
                None
            ),
            1920 * 1080 * 4
        );
    }

    #[test]
    fn dims_label_carries_dims_and_humanized_size() {
        assert_eq!(dims_label((100, 50), 20_000), "Image: 100×50 px · 19.5 KiB");
    }

    /// The statusbar spans the full window width and strips exactly
    /// its height zone off the bottom: the central panel above it
    /// ends at `screen_h − STATUSBAR_HEIGHT`.
    #[test]
    fn statusbar_is_a_full_width_bottom_strip() {
        const W: f32 = 640.0;
        const H: f32 = 480.0;
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(W, H),
            )),
            ..egui::RawInput::default()
        };
        let out = ctx.run_ui(raw, |ui| {
            show(ui, (1920, 1080), Tool::Pencil, 1920 * 1080 * 4);
            let central = egui::CentralPanel::default().show(ui, |ui| {
                ui.allocate_space(egui::vec2(1.0, 1.0));
            });
            let r = central.response.rect;
            assert!(
                r.top().abs() < 1.0,
                "the central panel spans from the top, top {}",
                r.top()
            );
            assert!(
                (r.bottom() - (H - STATUSBAR_HEIGHT)).abs() < 1.0,
                "the statusbar strips its zone off the bottom, bottom {}",
                r.bottom()
            );
        });
        // The painted hint is the engine's per-tool hint, in gold.
        let hint = crate::panel::tool_hint(Tool::Pencil);
        let gold_hints = out.shapes.iter().filter(|cs| {
            matches!(
                &cs.shape,
                egui::Shape::Text(t)
                    if t.galley.job.text == hint
                        && t.galley.job.sections.first().is_some_and(
                            |s| s.format.color == p::GOLD
                        )
            )
        });
        assert!(gold_hints.count() > 0, "the tool hint paints gold: {hint}");
    }

    /// The hint text follows the selected tool: two tools paint two
    /// different hints.
    #[test]
    fn hint_switches_with_the_selected_tool() {
        let painted = |tool: Tool| {
            let ctx = egui::Context::default();
            theme::apply(&ctx);
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1032.0, 648.0),
                )),
                ..egui::RawInput::default()
            };
            let out = ctx.run_ui(raw, |ui| {
                show(ui, (100, 100), tool, 100 * 100 * 4);
            });
            let want = crate::panel::tool_hint(tool);
            out.shapes
                .iter()
                .any(|cs| matches!(&cs.shape, egui::Shape::Text(t) if t.galley.job.text == want))
        };
        assert!(painted(Tool::Pencil), "pencil hint paints");
        assert!(painted(Tool::Text), "text hint paints");
        assert_ne!(
            crate::panel::tool_hint(Tool::Pencil),
            crate::panel::tool_hint(Tool::Text),
            "the two tools actually have distinct hints"
        );
    }

    /// The binding chips render: the real binding table's
    /// Ctrl+Z / Ctrl+Y / Ctrl+S / Del / Ctrl+B all paint.
    #[test]
    fn kbd_chips_carry_the_binding_table() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1032.0, 648.0),
            )),
            ..egui::RawInput::default()
        };
        let out = ctx.run_ui(raw, |ui| {
            show(ui, (100, 100), Tool::Pencil, 100 * 100 * 4);
        });
        for key in KEYS {
            let painted = out
                .shapes
                .iter()
                .any(|cs| matches!(&cs.shape, egui::Shape::Text(t) if t.galley.job.text == key));
            assert!(painted, "kbd chip {key} paints");
        }
    }

    /// The statusbar names the panel toggle: a `Ctrl+B` kbd chip
    /// paints, so the keyboard reopen path is discoverable before
    /// the first collapse (the floating « chip covers the pointer
    /// path once collapsed).
    #[test]
    fn statusbar_names_the_ctrl_b_panel_toggle() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1032.0, 648.0),
            )),
            ..egui::RawInput::default()
        };
        let out = ctx.run_ui(raw, |ui| {
            show(ui, (100, 100), Tool::Pencil, 100 * 100 * 4);
        });
        let painted = out
            .shapes
            .iter()
            .any(|cs| matches!(&cs.shape, egui::Shape::Text(t) if t.galley.job.text == "Ctrl+B"));
        assert!(painted, "the Ctrl+B chip paints on the statusbar");
    }

    // --- Responsive fit (responsive-ui) -------------------------------
    //
    // The bar's centre hint and right chip row yield before they
    // collide with the dims readout or run off the window: the hint
    // is skipped when its span cannot hold it, and the chips drop
    // from the tail (the binding table's order is importance order)
    // when they cannot fit.

    fn bar_frame(screen: egui::Vec2, tool: Tool) -> egui::FullOutput {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), screen)),
            ..egui::RawInput::default()
        };
        ctx.run_ui(raw, |ui| show(ui, (1920, 1080), tool, 1920 * 1080 * 4))
    }

    fn paints_text(out: &egui::FullOutput, needle: &str) -> bool {
        out.shapes.iter().any(|cs| match &cs.shape {
            egui::Shape::Text(t) => t.galley.job.text.contains(needle),
            _ => false,
        })
    }

    #[test]
    fn the_hint_is_skipped_when_the_span_cannot_hold_it() {
        let wide = bar_frame(egui::vec2(1032.0, 648.0), Tool::Pencil);
        assert!(
            paints_text(&wide, crate::panel::tool_hint(Tool::Pencil)),
            "a wide bar centres the hint"
        );
        let narrow = bar_frame(egui::vec2(420.0, 300.0), Tool::Pencil);
        assert!(
            !paints_text(&narrow, crate::panel::tool_hint(Tool::Pencil)),
            "a narrow bar skips the hint rather than truncating it"
        );
        assert!(
            paints_text(&narrow, "Image:"),
            "the dims readout stays on a narrow bar"
        );
    }

    #[test]
    fn the_chips_drop_the_tail_before_colliding_at_narrow_widths() {
        let narrow = bar_frame(egui::vec2(340.0, 300.0), Tool::Pencil);
        let escape = crate::test_layout::worst_text_clip_escape(&narrow);
        assert!(
            escape <= 0.5,
            "no statusbar text may truncate (worst escape {escape:.1} px)"
        );
        assert!(paints_text(&narrow, "Image:"), "the dims readout stays");
        let chip_count = narrow
            .shapes
            .iter()
            .filter(|cs| {
                matches!(&cs.shape, egui::Shape::Text(t) if KEYS.contains(&t.galley.job.text.as_str()))
            })
            .count();
        assert!(
            chip_count >= 1 && chip_count < KEYS.len(),
            "a 340 px bar keeps some but not all chips (kept {chip_count})"
        );
    }

    #[test]
    fn the_wide_bar_keeps_the_whole_chip_row() {
        let wide = bar_frame(egui::vec2(1032.0, 648.0), Tool::Pencil);
        let chip_count = wide
            .shapes
            .iter()
            .filter(|cs| {
                matches!(&cs.shape, egui::Shape::Text(t) if KEYS.contains(&t.galley.job.text.as_str()))
            })
            .count();
        assert_eq!(chip_count, KEYS.len(), "every chip paints on a wide bar");
    }

    /// The progressive discard's pure seam: how many leading chips
    /// fit `available` px at `gap` spacing — the tail yields first.
    #[test]
    fn fitting_chips_prefix_counts_the_fitting_head() {
        // 50 + 6 + 50 = 106 ≤ 140; a third chip would need 162.
        assert_eq!(fitting_chips_prefix(&[50.0, 50.0, 50.0], 140.0, 6.0), 2);
        // Everything fits.
        assert_eq!(fitting_chips_prefix(&[50.0, 50.0], 200.0, 6.0), 2);
        // Nothing fits: the readout keeps its row (never negative,
        // never a squeezed chip).
        assert_eq!(fitting_chips_prefix(&[50.0, 50.0], 10.0, 6.0), 0);
    }
}
