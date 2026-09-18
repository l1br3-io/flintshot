//! Shared cutout widgets — the reusable controls the five-zone UI
//! composes everywhere: the [`ghost_button`] for actions on forest
//! fills, [`well_row`] segmented value picks, the [`switch`] pill,
//! collapsible [`section`] cards, and the [`kbd_chip`] key hint.
//!
//! Every widget consumes the Task-1 tokens only (no `Color32`
//! literals here) and eases its microinteractions through
//! [`theme::ease`] behind the single [`animate`] gate — Task 11's
//! `ui.animations` setting flips that off in one place.

use crate::theme;

/// The animation gate's slot in the context's per-frame data — one
/// well-known id, set by the settings route, read by every widget.
const ANIMATIONS_KEY: &str = "flintshot_widgets_animations";

/// Whether spring microinteractions run. Defaults to `true`; the
/// `ui.animations` setting (Task 11) flips it through
/// [`set_animations`], and every widget's transitions no-op at once.
pub(crate) fn animate(ctx: &egui::Context) -> bool {
    ctx.data_mut(|d| d.get_temp::<bool>(egui::Id::new(ANIMATIONS_KEY)))
        .unwrap_or(true)
}

/// Set the [`animate`] gate for this context.
pub(crate) fn set_animations(ctx: &egui::Context, on: bool) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(ANIMATIONS_KEY), on));
}

/// Advance a per-id 0→1 progress toward `target` one frame's worth
/// and spring-ease it. With the gate off the value jumps straight to
/// the target — no transitions anywhere.
fn eased(ctx: &egui::Context, id: egui::Id, target: f32, duration: f32) -> f32 {
    if !animate(ctx) {
        return target;
    }
    let dt = ctx.input(|i| i.stable_dt);
    let t = ctx.data_mut(|d| {
        let current = d.get_temp::<f32>(id).unwrap_or(0.0);
        let next = theme::ease::approach(current, target, dt, duration);
        d.insert_temp(id, next);
        next
    });
    theme::ease::spring(t)
}

/// A quiet action button for forest fills: painted glyph + label,
/// transparent at rest with a cream-alpha hairline, a cream wash +
/// gold border on hover. `hero_key` arms the gold border at rest
/// (the button that owns the surface's hero shortcut). Disabled
/// buttons dim to cream-dim and swallow input. `copied` swaps the
/// glyph slot to the painted check (`panel::g_check`) and the
/// foreground to gold: the Copy hero's ~2 s "Copied" state. Icons
/// are painted vector glyphs, never text — the font chain's exotic
/// codepoints are tofu (ux-redesign-regressions cmap scan).
/// The painted glyph slot edge, logical px — `rail`'s ±7 glyph
/// space plus padding.
const GLYPH_EDGE: f32 = 14.0;

/// One hero ghost button; registers under `id` and reports clicks.
pub(crate) fn ghost_button(
    ui: &mut egui::Ui,
    id: egui::Id,
    glyph: crate::rail::Glyph,
    label: &str,
    hero_key: bool,
    copied: bool,
) -> egui::Response {
    use crate::theme::palette as p;
    let fg = if !ui.is_enabled() {
        p::CREAM_DIM
    } else if copied {
        p::GOLD
    } else {
        p::CREAM_TEXT
    };
    let font = egui::FontId::proportional(theme::TYPE_MD);
    let label_galley = ui.painter().layout_no_wrap(label.to_owned(), font, fg);
    let pad_x = 12.0;
    let gap = 6.0;
    let size = egui::vec2(
        pad_x + GLYPH_EDGE + gap + label_galley.size().x + pad_x,
        30.0,
    );
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let sense = if ui.is_enabled() {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let resp = ui.interact(rect, id, sense);
    let (fill, border) = if !ui.is_enabled() {
        (egui::Color32::TRANSPARENT, p::with_alpha(p::CREAM_TEXT, 31))
    } else if resp.hovered() {
        let border = if hero_key { p::GOLD_DEEP } else { p::GOLD };
        (p::with_alpha(p::CREAM_TEXT, 36), border)
    } else if hero_key {
        (egui::Color32::TRANSPARENT, p::GOLD)
    } else {
        (egui::Color32::TRANSPARENT, p::with_alpha(p::CREAM_TEXT, 64))
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(theme::POD_RADIUS),
        fill,
        egui::Stroke::new(1.0, border),
        egui::StrokeKind::Inside,
    );
    let text_w = GLYPH_EDGE + gap + label_galley.size().x;
    let x0 = rect.center().x - text_w / 2.0;
    let glyph_rect = egui::Rect::from_center_size(
        egui::pos2(x0 + GLYPH_EDGE / 2.0, rect.center().y),
        egui::vec2(GLYPH_EDGE, GLYPH_EDGE),
    );
    if copied {
        crate::panel::g_check(ui.painter(), glyph_rect, fg);
    } else {
        glyph(ui.painter(), glyph_rect, fg);
    }
    let label_pos = egui::pos2(
        x0 + GLYPH_EDGE + gap,
        rect.center().y - label_galley.size().y / 2.0,
    );
    ui.painter().galley(label_pos, label_galley, fg);
    resp
}

/// A well (paper-dim inset) holding segmented value picks: the
/// selected segment fills forest-mid with cream text, the rest sit
/// as cream chips with ink-soft labels. Returns the clicked
/// segment's index, if any.
pub(crate) fn well_row(
    ui: &mut egui::Ui,
    id: egui::Id,
    options: &[&str],
    selected: usize,
) -> Option<usize> {
    use crate::theme::palette as p;
    let seg_h = 26.0;
    let seg_pad = 10.0;
    let gap = 4.0;
    let well_pad = 3.0;
    let font = egui::FontId::proportional(theme::TYPE_SM);
    let galleys: Vec<_> = options
        .iter()
        .map(|o| {
            ui.painter()
                .layout_no_wrap((*o).to_owned(), font.clone(), p::INK)
        })
        .collect();
    let inner_w: f32 = galleys
        .iter()
        .map(|g| seg_pad * 2.0 + g.size().x)
        .sum::<f32>()
        + gap * f32::from(u16::try_from(options.len().saturating_sub(1)).unwrap_or(0));
    let well_size = egui::vec2(inner_w + well_pad * 2.0, seg_h + well_pad * 2.0);
    let (rect, _) = ui.allocate_exact_size(well_size, egui::Sense::hover());
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(theme::POD_RADIUS),
        p::PAPER_DIM,
        egui::Stroke::NONE,
        egui::StrokeKind::Middle,
    );
    let mut clicked = None;
    let mut x = rect.left() + well_pad;
    for (i, galley) in galleys.iter().enumerate() {
        let seg_rect = egui::Rect::from_min_size(
            egui::pos2(x, rect.top() + well_pad),
            egui::vec2(seg_pad * 2.0 + galley.size().x, seg_h),
        );
        let is_selected = i == selected;
        let (fill, ink, stroke) = if is_selected {
            (p::FOREST_MID, p::CREAM_TEXT, egui::Stroke::NONE)
        } else {
            (p::PAPER_CREAM, p::INK_SOFT, theme::HAIRLINE_STROKE)
        };
        ui.painter().rect(
            seg_rect,
            egui::CornerRadius::same(theme::POD_RADIUS),
            fill,
            stroke,
            egui::StrokeKind::Middle,
        );
        let pos = egui::pos2(
            seg_rect.left() + seg_pad,
            seg_rect.center().y - galley.size().y / 2.0,
        );
        ui.painter().galley(pos, galley.clone(), ink);
        if ui
            .interact(seg_rect, id.with(i), egui::Sense::click())
            .clicked()
        {
            clicked = Some(i);
        }
        x = seg_rect.right() + gap;
    }
    clicked
}

/// What a [`switch`] reports for this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SwitchResponse {
    /// The switch's state after this frame's interaction.
    pub on: bool,
}

/// The pill toggle: on = gold track with the knob translated right,
/// off = paper-dim track, knob springing between. Locked switches
/// render at 55% opacity and ignore input.
pub(crate) fn switch(ui: &mut egui::Ui, id: egui::Id, on: bool, locked: bool) -> SwitchResponse {
    use crate::theme::palette as p;
    const TRACK: egui::Vec2 = egui::vec2(34.0, 20.0);
    let knob_d = 14.0;
    let knob_pad = 3.0;
    let (rect, _) = ui.allocate_exact_size(TRACK, egui::Sense::hover());
    let resp = ui.interact(
        rect,
        id,
        if locked {
            egui::Sense::hover()
        } else {
            egui::Sense::click()
        },
    );
    let now_on = if resp.clicked() { !on } else { on };
    let eased = eased(
        ui.ctx(),
        id.with("knob"),
        if now_on { 1.0 } else { 0.0 },
        0.16,
    );
    let travel = TRACK.x - knob_d - knob_pad * 2.0;
    let knob_x = rect.left() + knob_pad + travel * eased;
    let (mut track_fill, mut knob_fill, mut stroke) = if now_on {
        (p::GOLD, p::CREAM_TEXT, egui::Stroke::NONE)
    } else {
        (p::PAPER_DIM, p::PAPER_CREAM, theme::HAIRLINE_STROKE)
    };
    if locked {
        track_fill = track_fill.gamma_multiply(0.55);
        knob_fill = knob_fill.gamma_multiply(0.55);
        stroke.color = stroke.color.gamma_multiply(0.55);
    }
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(theme::PILL_RADIUS),
        track_fill,
        stroke,
        egui::StrokeKind::Middle,
    );
    let knob = egui::Rect::from_center_size(
        egui::pos2(knob_x + knob_d / 2.0, rect.center().y),
        egui::vec2(knob_d, knob_d),
    );
    ui.painter().rect(
        knob,
        egui::CornerRadius::same(theme::PILL_RADIUS),
        knob_fill,
        egui::Stroke::NONE,
        egui::StrokeKind::Middle,
    );
    SwitchResponse { on: now_on }
}

/// What a [`section`] reports for this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SectionResponse {
    /// The section's open state after this frame's interaction.
    pub open: bool,
    /// The body's visible height this frame (0 while closed) — the
    /// spring-animated allocation.
    pub body_height: f32,
}

/// A collapsible card: a chevron + title header row, and a body that
/// springs open and shut. The open flag persists across frames
/// under the caller's `id`; the body is measured fresh each frame
/// offscreen, so content changes resize without churn.
pub(crate) fn section(
    ui: &mut egui::Ui,
    id: egui::Id,
    title: &str,
    default_open: bool,
    mut body: impl FnMut(&mut egui::Ui),
) -> SectionResponse {
    use crate::theme::palette as p;
    let mut open = ui
        .ctx()
        .data_mut(|d| d.get_temp::<bool>(id))
        .unwrap_or(default_open);

    let header_h = 26.0;
    let (header_rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), header_h),
        egui::Sense::hover(),
    );
    if ui.interact(header_rect, id, egui::Sense::click()).clicked() {
        open = !open;
    }

    let target = if open { 1.0 } else { 0.0 };
    let eased = eased(ui.ctx(), id.with("unfold"), target, 0.22);

    // Chevron: apex pointing RIGHT when closed, rotating to point
    // DOWN (flat side up) with the spring as the section unfolds.
    let c = egui::pos2(header_rect.left() + 8.0, header_rect.center().y);
    let angle = eased * std::f32::consts::FRAC_PI_2;
    let (sin, cos) = angle.sin_cos();
    let pt = |dx: f32, dy: f32| egui::pos2(c.x + dx * cos - dy * sin, c.y + dx * sin + dy * cos);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![pt(-4.0, -4.0), pt(-4.0, 4.0), pt(5.0, 0.0)],
        p::INK_SOFT,
        egui::Stroke::NONE,
    ));
    let galley = ui.painter().layout_no_wrap(
        title.to_owned(),
        egui::FontId::proportional(theme::TYPE_MD),
        p::INK,
    );
    ui.painter().galley(
        egui::pos2(
            header_rect.left() + 20.0,
            header_rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        p::INK,
    );

    let mut body_height = 0.0;
    if open || eased > 0.0 {
        // Measure the body offscreen and invisible; the spring then
        // unfolds the visible allocation toward that height.
        let avail_w = ui.available_width();
        let measure_rect = egui::Rect::from_min_size(
            egui::pos2(ui.max_rect().left(), ui.max_rect().bottom() + 10_000.0),
            egui::vec2(avail_w, 0.0),
        );
        let mut probe = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(measure_rect)
                .invisible()
                .layout(egui::Layout::top_down_justified(egui::Align::LEFT)),
        );
        body(&mut probe);
        let measured = probe.min_rect().height().max(1.0);
        body_height = measured * eased;
        ui.allocate_ui(egui::vec2(avail_w, body_height), |child| {
            child.set_clip_rect(child.max_rect());
            body(child);
        });
    }

    ui.ctx().data_mut(|d| d.insert_temp(id, open));
    SectionResponse { open, body_height }
}

/// A keyboard-hint chip: the keys in the xs ramp on a cream chip
/// with a hairline border.
pub(crate) fn kbd_chip(ui: &mut egui::Ui, keys: &str) {
    use crate::theme::palette as p;
    let galley = ui.painter().layout_no_wrap(
        keys.to_owned(),
        egui::FontId::proportional(theme::TYPE_XS),
        p::INK_SOFT,
    );
    let pad_x = 6.0;
    let size = egui::vec2(galley.size().x + pad_x * 2.0, galley.size().y + 4.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(4),
        p::PAPER_CREAM,
        theme::HAIRLINE_STROKE,
        egui::StrokeKind::Middle,
    );
    ui.painter().galley(
        egui::pos2(rect.left() + pad_x, rect.center().y - galley.size().y / 2.0),
        galley,
        p::INK_SOFT,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Pos2;

    const DT: f32 = 1.0 / 60.0;
    const SCREEN: egui::Vec2 = egui::vec2(400.0, 300.0);

    fn base_input(time: f32, events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            time: Some(f64::from(time)),
            predicted_dt: DT,
            events,
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, SCREEN)),
            ..egui::RawInput::default()
        }
    }

    /// One frame of `draw` with `events` on a 400×300 surface and a
    /// stable 60 fps delta so the springs have a clock.
    fn run(
        ctx: &egui::Context,
        time: f32,
        events: Vec<egui::Event>,
        draw: impl FnMut(&mut egui::Ui),
    ) {
        let raw = base_input(time, events);
        let _ = ctx.run_ui(raw, draw);
    }

    /// The repo's one-frame click: move, press, release in order.
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

    /// One event-free frame of `draw`; the frame's painted shapes
    /// come back for color scans.
    fn shapes_of(
        ctx: &egui::Context,
        time: f32,
        draw: impl FnMut(&mut egui::Ui),
    ) -> Vec<egui::epaint::ClippedShape> {
        let raw = base_input(time, vec![]);
        ctx.run_ui(raw, draw).shapes
    }

    #[test]
    fn animate_gate_defaults_on_and_round_trips() {
        let ctx = egui::Context::default();
        assert!(animate(&ctx), "animations default on");
        set_animations(&ctx, false);
        assert!(!animate(&ctx), "the setter must flip the gate");
        set_animations(&ctx, true);
        assert!(animate(&ctx));
    }

    #[test]
    fn ghost_button_renders_in_both_variants() {
        for hero in [false, true] {
            let ctx = egui::Context::default();
            theme::apply(&ctx);
            run(&ctx, 0.0, vec![], |ui| {
                ghost_button(
                    ui,
                    egui::Id::new("gb"),
                    crate::panel::g_copy,
                    "Copy",
                    hero,
                    false,
                );
            });
            assert!(
                ctx.read_response(egui::Id::new("gb")).is_some(),
                "hero={hero}: the button renders"
            );
        }
    }

    #[test]
    fn ghost_button_clicks_when_clicked() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        run(&ctx, 0.0, vec![], |ui| {
            ghost_button(
                ui,
                egui::Id::new("gb"),
                crate::panel::g_copy,
                "Copy",
                false,
                false,
            );
        });
        let rect = ctx
            .read_response(egui::Id::new("gb"))
            .expect("pass 1 laid the button out")
            .rect;
        let mut clicked = false;
        run(&ctx, DT, click_events(rect.center()), |ui| {
            clicked = ghost_button(
                ui,
                egui::Id::new("gb"),
                crate::panel::g_copy,
                "Copy",
                false,
                false,
            )
            .clicked();
        });
        assert!(clicked, "a pointer press+release on the button clicks");
    }

    #[test]
    fn ghost_button_disabled_is_not_interactive() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        run(&ctx, 0.0, vec![], |ui| {
            ghost_button(
                ui,
                egui::Id::new("gb"),
                crate::panel::g_copy,
                "Copy",
                false,
                false,
            );
        });
        let rect = ctx
            .read_response(egui::Id::new("gb"))
            .expect("pass 1 laid the button out")
            .rect;
        run(&ctx, DT, click_events(rect.center()), |ui| {
            ui.disable();
            let resp = ghost_button(
                ui,
                egui::Id::new("gb"),
                crate::panel::g_copy,
                "Copy",
                false,
                false,
            );
            assert!(!resp.clicked(), "a disabled button swallows clicks");
        });
    }

    #[test]
    fn well_row_reports_clicked_segment_and_shows_selection() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let shapes = shapes_of(&ctx, 0.0, |ui| {
            well_row(ui, egui::Id::new("well"), &["Auto", "Full", "Custom"], 1);
        });
        // The selected segment (index 1) paints a forest-mid fill.
        let selected_fill = shapes.iter().any(
            |cs| matches!(&cs.shape, egui::Shape::Rect(r) if r.fill == theme::palette::FOREST_MID),
        );
        assert!(selected_fill, "the selected segment paints forest-mid");

        // Click the third segment; its index comes back.
        let well = ctx
            .read_response(egui::Id::new("well").with(2_usize))
            .expect("segment 2 renders as its own response");
        let picked = std::cell::Cell::new(None);
        run(&ctx, DT, click_events(well.rect.center()), |ui| {
            picked.set(well_row(
                ui,
                egui::Id::new("well"),
                &["Auto", "Full", "Custom"],
                1,
            ));
        });
        assert_eq!(
            picked.get(),
            Some(2),
            "the clicked segment index comes back"
        );
    }

    #[test]
    fn switch_toggles_and_paints_gold_when_on() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        run(&ctx, 0.0, vec![], |ui| {
            switch(ui, egui::Id::new("sw"), false, false);
        });
        let rect = ctx
            .read_response(egui::Id::new("sw"))
            .expect("pass 1 laid the switch out")
            .rect;

        // Off → click reports on.
        let now_on = std::cell::Cell::new(true);
        run(&ctx, DT, click_events(rect.center()), |ui| {
            now_on.set(switch(ui, egui::Id::new("sw"), false, false).on);
        });
        assert!(now_on.get(), "a click on an off switch turns it on");

        // On: the track paints full-alpha gold.
        let shapes = shapes_of(&ctx, 3.0 * DT, |ui| {
            switch(ui, egui::Id::new("sw"), true, false);
        });
        let gold_track = shapes.iter().any(|cs| {
            matches!(&cs.shape, egui::Shape::Rect(r)
                if r.fill == theme::palette::GOLD && r.fill.a() == u8::MAX)
        });
        assert!(gold_track, "the on-track paints full-alpha gold");

        // On → click reports off.
        let now_off = std::cell::Cell::new(true);
        run(&ctx, 2.0 * DT, click_events(rect.center()), |ui| {
            now_off.set(switch(ui, egui::Id::new("sw"), true, false).on);
        });
        assert!(!now_off.get(), "a click on an on switch turns it off");
    }

    #[test]
    fn switch_locked_is_non_interactive_and_dimmed() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        run(&ctx, 0.0, vec![], |ui| {
            switch(ui, egui::Id::new("sw"), true, true);
        });
        let rect = ctx
            .read_response(egui::Id::new("sw"))
            .expect("locked still renders its row")
            .rect;
        run(&ctx, DT, click_events(rect.center()), |ui| {
            let resp = switch(ui, egui::Id::new("sw"), true, true);
            assert!(resp.on, "locked keeps its state under a click");
        });
        let shapes = shapes_of(&ctx, 4.0 * DT, |ui| {
            switch(ui, egui::Id::new("sw"), true, true);
        });
        let full_alpha_gold = shapes.iter().any(|cs| {
            matches!(&cs.shape, egui::Shape::Rect(r)
                if r.fill == theme::palette::GOLD && r.fill.a() == u8::MAX)
        });
        assert!(
            !full_alpha_gold,
            "locked paints the track dimmed, not full gold"
        );
    }

    #[test]
    fn section_toggles_persists_and_settles_open() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let id = egui::Id::new("sec");
        // Frame 1: closed — click the header to open.
        run(&ctx, 0.0, vec![], |ui| {
            let resp = section(ui, id, "Stroke", false, |_| {});
            assert!(!resp.open, "starts closed");
        });
        let header = ctx
            .read_response(id)
            .expect("the header registers under the caller's id");
        run(&ctx, DT, click_events(header.rect.center()), |ui| {
            let resp = section(ui, id, "Stroke", false, |_| {});
            assert!(resp.open, "the header click opens the section");
        });
        // Frames 3..N: open persists; the body height grows with the
        // spring toward its settled height.
        let mut heights = Vec::new();
        for i in 2u16..=30 {
            let resp = std::cell::Cell::new(0.0_f32);
            run(&ctx, f32::from(i) * DT, vec![], |ui| {
                let r = section(ui, id, "Stroke", false, |body| {
                    body.label("stroke width controls");
                    body.add_space(8.0);
                    body.label("a second line");
                });
                assert!(r.open, "open persists across frames");
                resp.set(r.body_height);
            });
            heights.push(resp.get());
        }
        assert!(
            heights[0] < heights[heights.len() - 1],
            "the body height animates open: first {:?}, last {:?}",
            heights.first(),
            heights.last()
        );
        let settled = heights[heights.len() - 1];
        assert!(
            settled > 20.0,
            "the settled body shows the content, height {settled}"
        );
    }

    #[test]
    fn section_without_animations_jumps_to_its_target() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        set_animations(&ctx, false);
        let id = egui::Id::new("sec");
        run(&ctx, 0.0, vec![], |ui| {
            section(ui, id, "Stroke", false, |_| {});
        });
        let header = ctx.read_response(id).expect("the header renders");
        run(&ctx, DT, click_events(header.rect.center()), |ui| {
            section(ui, id, "Stroke", false, |_| {});
        });
        let height = std::cell::Cell::new(0.0_f32);
        run(&ctx, 2.0 * DT, vec![], |ui| {
            let resp = section(ui, id, "Stroke", false, |body| {
                body.label("stroke width controls");
                body.add_space(8.0);
                body.label("a second line");
            });
            height.set(resp.body_height);
        });
        assert!(
            height.get() > 20.0,
            "gate off: the body is fully open one frame after the click, got {}",
            height.get()
        );
    }

    /// The collapse chevron's direction: apex RIGHT when closed (the
    /// "expand me" affordance), apex DOWN when open — never left/up
    /// (the first build pointed left and read as broken).
    #[test]
    fn section_chevron_apex_points_right_closed_and_down_open() {
        let chevron_points = |open: bool| {
            let ctx = egui::Context::default();
            theme::apply(&ctx);
            set_animations(&ctx, false); // eased jumps to target
            let id = egui::Id::new("sec");
            if open {
                // One click opens; the next frame paints the settled
                // chevron.
                run(&ctx, 0.0, vec![], |ui| {
                    let _ = section(ui, id, "Stroke", false, |_| {});
                });
                let header = ctx.read_response(id).expect("the header registers");
                run(&ctx, DT, click_events(header.rect.center()), |ui| {
                    let _ = section(ui, id, "Stroke", false, |_| {});
                });
            }
            let shapes = shapes_of(&ctx, 2.0 * DT, |ui| {
                let _ = section(ui, id, "Stroke", false, |_| {});
            });
            shapes
                .iter()
                .filter_map(|cs| match &cs.shape {
                    // `convex_polygon` paints as a closed PathShape.
                    egui::Shape::Path(p) if p.closed && p.points.len() == 3 => {
                        Some([p.points[0], p.points[1], p.points[2]])
                    }
                    _ => None,
                })
                .next_back()
                .unwrap_or_else(|| panic!("open={open}: the chevron paints a triangle"))
        };
        let assert_direction = |pts: [Pos2; 3], open: bool| {
            let axis: Vec<f32> = if open {
                pts.iter().map(|p| p.y).collect() // apex down = max y
            } else {
                pts.iter().map(|p| p.x).collect() // apex right = max x
            };
            let max = axis.iter().copied().fold(f32::MIN, f32::max);
            let min = axis.iter().copied().fold(f32::MAX, f32::min);
            let at_max = axis.iter().filter(|v| (**v - max).abs() < 0.5).count();
            let at_min = axis.iter().filter(|v| (**v - min).abs() < 0.5).count();
            assert_eq!(
                (at_max, at_min),
                (1, 2),
                "open={open}: one apex + a flat base side, points {pts:?}"
            );
        };
        assert_direction(chevron_points(false), false);
        assert_direction(chevron_points(true), true);
    }

    #[test]
    fn kbd_chip_renders_its_keys() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        run(&ctx, 0.0, vec![], |ui| {
            kbd_chip(ui, "Ctrl+C");
        });
    }
}
