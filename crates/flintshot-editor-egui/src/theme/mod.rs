//! The pop-up book design-token system — the single style authority
//! for the home window, editor, and overlay.
//!
//! Cream paper surfaces, white cutout cards with thick borders and
//! painted shadows, forest accents, springy motion (see `ease`,
//! landed with the motion primitives task). [`palette`] holds every
//! color; this module holds the non-color tokens (spacing scale,
//! rounding, strokes, shadows, type ramp) and [`apply`], which pins
//! the light theme — the dark "deep forest" variant is a planned
//! follow-up effort, not a runtime choice.

// The motion + texture primitives ship ahead of their first
// consumers (hover-lift and pop-in land with the side panel /
// floating controls; grain with the welcome surfaces). Drop these
// allows when those land.
#[allow(dead_code)]
pub(crate) mod ease;
#[allow(dead_code)]
pub(crate) mod grain;
pub(crate) mod palette;

use egui::{Context, CornerRadius, Stroke};

/// The spacing scale, logical px — the only distances layout code
/// may use between pods, sections, and controls.
pub(crate) const SPACE_SCALE: [f32; 5] = [6.0, 10.0, 14.0, 18.0, 28.0];

/// Corner radius of a cutout — cards, bars, sections, popovers —
/// logical px (spec: cutout 10).
pub(crate) const CUTOUT_RADIUS: u8 = 10;
/// Corner radius of a pod — chips and buttons — logical px
/// (spec: pod 12, rounding wider than its container's cutout).
pub(crate) const POD_RADIUS: u8 = 12;
/// Corner radius of a pill — switches — logical px. Painter-side
/// clamping keeps any oversized value a true pill, so this is
/// effectively "as round as it gets".
// Consumed with the switch widget (task 002).
#[allow(dead_code)]
pub(crate) const PILL_RADIUS: u8 = 100;

/// The cutout border: 8 px of cream around every card and pod.
// Consumed by the docked side panel and welcome cards (tasks 005+);
// the toolbar pods sit on cream and use the hairline instead.
#[allow(dead_code)]
pub(crate) const CUTOUT_STROKE: Stroke = Stroke {
    width: 8.0,
    color: palette::PAPER_CREAM,
};

/// The 1 px paper-dim hairline for inner chips and separators.
pub(crate) const HAIRLINE_STROKE: Stroke = Stroke {
    width: 1.0,
    color: palette::PAPER_DIM,
};

/// Resting painted shadow (chips / sections): the soft layer of the
/// mockup's `--sh-resting`.
pub(crate) const SHADOW_RESTING: egui::Shadow = egui::Shadow {
    offset: [0, 3],
    blur: 8,
    spread: 0,
    color: palette::SHADOW_RESTING,
};

/// Lift painted shadow (hover): the soft layer of `--sh-lift`.
pub(crate) const SHADOW_LIFT: egui::Shadow = egui::Shadow {
    offset: [0, 8],
    blur: 18,
    spread: 0,
    color: palette::SHADOW_LIFT,
};

/// Pop painted shadow (active tool, menus): the soft layer of
/// `--sh-pop`.
pub(crate) const SHADOW_POP: egui::Shadow = egui::Shadow {
    offset: [0, 12],
    blur: 26,
    spread: 0,
    color: palette::SHADOW_POP,
};

/// Paper painted shadow (window / canvas paper, hero card): the
/// soft layer of `--sh-paper`.
pub(crate) const SHADOW_PAPER: egui::Shadow = egui::Shadow {
    offset: [0, 22],
    blur: 44,
    spread: 0,
    color: palette::SHADOW_PAPER,
};

/// The six-step type ramp, logical px (spec: xs 12 · sm 13.5 ·
/// md 15 · lg 18 · xl 24 · hero 34). Plus Jakarta Sans; the
/// display weight for hero / title lives in [`FAMILY_DISPLAY`].
pub(crate) const TYPE_XS: f32 = 12.0;
/// Small text: captions, hints, kbd chips.
pub(crate) const TYPE_SM: f32 = 13.5;
/// Body and control-label size.
pub(crate) const TYPE_MD: f32 = 15.0;
/// Title / heading size.
pub(crate) const TYPE_LG: f32 = 18.0;
/// Extra-large display text.
pub(crate) const TYPE_XL: f32 = 24.0;
/// Hero display text.
#[allow(dead_code)]
pub(crate) const TYPE_HERO: f32 = 34.0;

/// Inner padding between a pod island's edge and its chips, logical
/// px (i8 to match egui's `Margin`).
pub(crate) const POD_PAD: i8 = 6;

/// Extra gap between adjacent pod islands, logical px — on the
/// spacing scale, on top of the ambient item spacing, so pod
/// boundaries dominate the intra-pod chip spacing.
pub(crate) const POD_GAP: f32 = 8.0;

/// Widget button padding — visibly airier than the compact
/// instrument-panel density it replaces.
pub(crate) const BUTTON_PADDING: egui::Vec2 = egui::Vec2::new(12.0, 8.0);

/// Painted control height — segmented-well rows, list rows, pills:
/// the click-target baseline every surface's controls share.
// Consumed by the chrome / widget sweep (ai-ux-refresh tasks 2–4).
#[allow(dead_code)]
pub(crate) const CONTROL_H: f32 = 30.0;

/// Hero control height — the topbar's hero buttons, taller than
/// [`CONTROL_H`] to carry their glyph + label pair.
// Consumed by the chrome sweep (ai-ux-refresh task 2).
#[allow(dead_code)]
pub(crate) const HERO_H: f32 = 34.0;

/// Standard modal dialog width — the AI transform dialog and any
/// future cutout dialogs.
// Consumed by the dialogs sweep (ai-ux-refresh task 4).
#[allow(dead_code)]
pub(crate) const DIALOG_W: f32 = 400.0;

/// Move-tool hover outline (see `crate::chrome`): 1 px dashed white —
/// the "grab me" invitation, legible over any screenshot.
pub(crate) const MOVE_HOVER_STROKE: Stroke = Stroke {
    width: 1.0,
    color: palette::MOVE_HOVER_INK,
};
/// Move-tool grab outline: 2 px solid forest — the active-object
/// emphasis while a move drag is live.
pub(crate) const MOVE_GRAB_STROKE: Stroke = Stroke {
    width: 2.0,
    color: palette::FOREST_DEEP,
};
/// Persistent-selection outline (see `crate::chrome`): 1 px dashed
/// forest on the move chrome's dash / pad rhythm — the "this one is
/// selected" affordance the Del key acts on.
pub(crate) const SELECTED_STROKE: Stroke = Stroke {
    width: 1.0,
    color: palette::FOREST_DEEP,
};
/// Dash / gap rhythm of the hover outline, logical px.
pub(crate) const MOVE_DASH: f32 = 4.0;
pub(crate) const MOVE_GAP: f32 = 3.0;
/// Padding between an annotation's bbox and its move chrome, logical
/// px — keeps the outline off the annotation's own ink.
pub(crate) const MOVE_CHROME_PAD: f32 = 3.0;

/// Stroke-width presets behind the S / M / L dots.
pub(crate) const STROKE_PRESETS: [f32; 3] = [2.0, 4.0, 8.0];

/// Named font family for display text: Plus Jakarta Sans
/// `ExtraBold`. egui's `FontId` has no weight axis, so the display
/// weight lives in the family — see `font_definitions`.
pub(crate) const FAMILY_DISPLAY: &str = "jakarta-display";

/// Named font family for text annotations: the pre-Jakarta
/// Ubuntu-led chain, pinned so the text tool's on-canvas output is
/// unchanged by the UI re-skin (the annotation font is not the UI
/// font).
pub(crate) const FAMILY_ANNOTATION: &str = "annotation-sans";

/// Build the app's [`egui::FontDefinitions`]: Plus Jakarta Sans
/// (Regular / Medium / `ExtraBold`, OFL — see `assets/fonts/`)
/// embedded via `include_bytes!`, with Jakarta Regular as the
/// primary proportional face and the egui emoji fallbacks retained
/// behind it. GOTCHA: the monospace family (Hack) is left at the
/// egui default — dims readouts and Mono annotations rely on it.
fn font_definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();

    for (name, bytes) in [
        (
            "PlusJakartaSans-Regular",
            include_bytes!("../../assets/fonts/PlusJakartaSans-Regular.ttf") as &[u8],
        ),
        (
            "PlusJakartaSans-Medium",
            include_bytes!("../../assets/fonts/PlusJakartaSans-Medium.ttf"),
        ),
        (
            "PlusJakartaSans-ExtraBold",
            include_bytes!("../../assets/fonts/PlusJakartaSans-ExtraBold.ttf"),
        ),
    ] {
        fonts.font_data.insert(
            name.to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(bytes)),
        );
    }

    // UI text: Jakarta leads the chain; the default emoji fallbacks
    // trail for glyph coverage.
    if let Some(chain) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        chain.insert(0, "PlusJakartaSans-Regular".to_owned());
    }

    // The old pre-Jakarta proportional chain, re-homed under a
    // dedicated name so annotations render exactly as before.
    fonts.families.insert(
        egui::FontFamily::Name(FAMILY_ANNOTATION.into()),
        vec![
            "Ubuntu-Light".to_owned(),
            "NotoEmoji-Regular".to_owned(),
            "emoji-icon-font".to_owned(),
        ],
    );

    // Display weight: ExtraBold as its own family.
    fonts.families.insert(
        egui::FontFamily::Name(FAMILY_DISPLAY.into()),
        vec!["PlusJakartaSans-ExtraBold".to_owned()],
    );

    fonts
}

/// Apply the pop-up book theme to a [`Context`]. Called once per
/// window at app construction; light-only (dark is a follow-up
/// effort). Selected widgets and selected text inherit the forest
/// accent for free via the widget / selection visuals.
pub(crate) fn apply(ctx: &Context) {
    ctx.set_theme(egui::ThemePreference::Light);
    ctx.set_visuals(visuals());
    ctx.set_fonts(font_definitions());
    ctx.style_mut_of(egui::Theme::Light, |style| {
        style.spacing.button_padding = BUTTON_PADDING;
        style.spacing.item_spacing = egui::vec2(SPACE_SCALE[1], SPACE_SCALE[1]);
        style.spacing.interact_size = egui::vec2(CONTROL_H, CONTROL_H);
        style.spacing.slider_width = 120.0;
        style.spacing.slider_rail_height = 6.0;
        // The native type ladder joins the ramp: body / button text
        // at TYPE_MD, headings at TYPE_LG, small at TYPE_SM — so
        // text fields, DragValues, markdown, and toast match the
        // painted surfaces without per-site fonts.
        for (style_key, size) in [
            (egui::TextStyle::Body, TYPE_MD),
            (egui::TextStyle::Button, TYPE_MD),
            (egui::TextStyle::Heading, TYPE_LG),
            (egui::TextStyle::Small, TYPE_SM),
        ] {
            if let Some(font) = style.text_styles.get_mut(&style_key) {
                font.size = size;
            }
        }
    });
}

/// Build the paper `Visuals`: cream chips with hairline borders on
/// the paper desk, a forest border on hover, forest fills on
/// active/selected with cream glyphs.
fn visuals() -> egui::Visuals {
    let mut v = egui::Visuals::light();
    v.panel_fill = palette::PAPER;
    v.window_fill = palette::PAPER_CREAM;
    v.faint_bg_color = palette::PAPER_DIM;
    v.extreme_bg_color = palette::PAPER_DIM;
    v.hyperlink_color = palette::FOREST_DEEP;
    v.window_shadow = SHADOW_PAPER;
    v.popup_shadow = SHADOW_POP;

    // Non-interactive: labels, separators, window outline.
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette::INK);
    v.widgets.noninteractive.bg_stroke = HAIRLINE_STROKE;
    v.widgets.noninteractive.bg_fill = palette::PAPER_CREAM;
    v.widgets.noninteractive.corner_radius = CornerRadius::same(POD_RADIUS);

    // Resting widgets: cream cutout chips with hairline borders.
    v.widgets.inactive.weak_bg_fill = palette::PAPER_CREAM;
    v.widgets.inactive.bg_fill = palette::PAPER_CREAM;
    v.widgets.inactive.bg_stroke = HAIRLINE_STROKE;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, palette::INK);
    v.widgets.inactive.corner_radius = CornerRadius::same(POD_RADIUS);

    // Hover: the forest border on a dim-well fill.
    v.widgets.hovered.weak_bg_fill = palette::PAPER_DIM;
    v.widgets.hovered.bg_fill = palette::PAPER_DIM;
    v.widgets.hovered.bg_stroke = Stroke::new(1.25, palette::FOREST);
    v.widgets.hovered.fg_stroke = Stroke::new(1.2, palette::INK);
    v.widgets.hovered.corner_radius = CornerRadius::same(POD_RADIUS);

    // Pressed / open / selected: forest-filled with cream glyphs —
    // the chip reads as cut out of the panel.
    v.widgets.active.weak_bg_fill = palette::FOREST_DEEP;
    v.widgets.active.bg_fill = palette::FOREST_DEEP;
    v.widgets.active.bg_stroke = Stroke::new(1.0, palette::FOREST_DEEP);
    v.widgets.active.fg_stroke = Stroke::new(1.2, palette::CREAM_TEXT);
    v.widgets.active.corner_radius = CornerRadius::same(POD_RADIUS);
    v.widgets.open = v.widgets.hovered;

    // Selection (text spans + selected segmented options): forest
    // background with cream foreground.
    v.selection.bg_fill = palette::FOREST_MID;
    v.selection.stroke = Stroke::new(1.2, palette::CREAM_TEXT);

    // Popup / window dressing.
    v.window_stroke = HAIRLINE_STROKE;
    v.window_corner_radius = CornerRadius::same(CUTOUT_RADIUS);
    v.menu_corner_radius = CornerRadius::same(CUTOUT_RADIUS);
    v
}

#[cfg(test)]
mod tests {
    // Token tests pin constants by design; they exist to fail on
    // drift. Const-evaluable asserts are the point, not an accident.
    #![expect(
        clippy::assertions_on_constants,
        reason = "the constants ARE the pinned contract; the tests exist to fail on drift"
    )]

    use super::*;

    /// The spacing scale is exactly the five spec steps — no
    /// off-scale distances may sneak in as constants.
    #[test]
    fn spacing_scale_is_exactly_the_five_steps() {
        let want = [6.0_f32, 10.0, 14.0, 18.0, 28.0];
        for (got, want) in SPACE_SCALE.iter().zip(want) {
            assert_eq!(got.to_bits(), want.to_bits(), "step must be exact");
        }
        assert!(
            SPACE_SCALE.windows(2).all(|w| w[0] < w[1]),
            "the scale must be strictly increasing"
        );
    }

    /// A pod (chip / button) rounds wider than its cutout container,
    /// per the mockup's radius pair.
    #[test]
    fn pod_radius_rounds_wider_than_cutout() {
        assert_eq!(POD_RADIUS, 12);
        assert_eq!(CUTOUT_RADIUS, 10);
        assert!(POD_RADIUS > CUTOUT_RADIUS);
    }

    /// The four-level shadow spec: resting / lift / pop / paper,
    /// strictly softer by blur across resting → lift → pop → paper,
    /// and all distinct.
    #[test]
    fn shadows_pin_the_four_levels() {
        assert_eq!(SHADOW_RESTING.blur, 8);
        assert_eq!(SHADOW_LIFT.blur, 18);
        assert_eq!(SHADOW_POP.blur, 26);
        assert_eq!(SHADOW_PAPER.blur, 44);
        assert!(SHADOW_RESTING.blur < SHADOW_LIFT.blur);
        assert!(SHADOW_LIFT.blur < SHADOW_POP.blur);
        assert!(SHADOW_POP.blur < SHADOW_PAPER.blur);
        assert_ne!(SHADOW_RESTING, SHADOW_LIFT);
        assert_ne!(SHADOW_POP, SHADOW_PAPER);
    }

    /// The cutout stroke is the spec's 8 px cream border.
    #[test]
    fn cutout_stroke_is_eight_px_cream() {
        assert_eq!(CUTOUT_STROKE.width.to_bits(), 8.0_f32.to_bits());
        assert_eq!(CUTOUT_STROKE.color, palette::PAPER_CREAM);
    }

    /// The hairline is 1 px of the paper-dim neutral.
    #[test]
    fn hairline_is_one_px_neutral() {
        assert_eq!(HAIRLINE_STROKE.width.to_bits(), 1.0_f32.to_bits());
        assert_eq!(HAIRLINE_STROKE.color, palette::PAPER_DIM);
    }

    /// The type ramp is exactly the six spec sizes, strictly
    /// increasing, and nothing on the ramp drops below the 12 px
    /// legibility floor.
    #[test]
    fn type_ramp_is_exactly_the_six_spec_sizes() {
        let ramp = [TYPE_XS, TYPE_SM, TYPE_MD, TYPE_LG, TYPE_XL, TYPE_HERO];
        let want = [12.0_f32, 13.5, 15.0, 18.0, 24.0, 34.0];
        for (got, want) in ramp.iter().zip(want) {
            assert_eq!(got.to_bits(), want.to_bits(), "ramp step must be exact");
        }
        assert!(ramp.windows(2).all(|w| w[0] < w[1]));
        assert!(
            ramp.iter().all(|&s| s >= 12.0),
            "no UI text below the 12 px floor"
        );
    }

    /// Control-metric tokens pin the density bump: control/pill
    /// height, hero control height, dialog width, and the exact
    /// button padding. Painted controls read these, never literals.
    #[test]
    fn control_metric_tokens_pin_the_density_bump() {
        assert_eq!(CONTROL_H.to_bits(), 30.0_f32.to_bits());
        assert_eq!(HERO_H.to_bits(), 34.0_f32.to_bits());
        assert_eq!(DIALOG_W.to_bits(), 400.0_f32.to_bits());
        assert_eq!(BUTTON_PADDING, egui::Vec2::new(12.0, 8.0));
    }

    /// `apply` installs the control-height interact size so
    /// egui-native widgets land on the same click target as the
    /// painted ones.
    #[test]
    fn apply_installs_the_denser_interact_size() {
        let ctx = Context::default();
        apply(&ctx);
        assert_eq!(
            ctx.style_of(egui::Theme::Light).spacing.interact_size,
            egui::vec2(CONTROL_H, CONTROL_H)
        );
    }

    /// `apply` installs the native type ladder on the ramp: body /
    /// button text at `TYPE_MD`, headings at `TYPE_LG`, small text
    /// at `TYPE_SM` — native widgets (text fields, markdown, toast)
    /// match the painted surfaces without per-site fonts.
    #[test]
    fn apply_installs_the_native_type_ladder() {
        let ctx = Context::default();
        apply(&ctx);
        let styles = &ctx.style_of(egui::Theme::Light).text_styles;
        let size = |style: egui::TextStyle| {
            styles
                .get(&style)
                .map(|font| font.size)
                .unwrap_or_default()
                .to_bits()
        };
        assert_eq!(size(egui::TextStyle::Body), TYPE_MD.to_bits());
        assert_eq!(size(egui::TextStyle::Button), TYPE_MD.to_bits());
        assert_eq!(size(egui::TextStyle::Heading), TYPE_LG.to_bits());
        assert_eq!(size(egui::TextStyle::Small), TYPE_SM.to_bits());
    }

    /// Stroke presets stay strictly increasing and in slider range.
    #[test]
    fn stroke_presets_are_ordered_and_in_slider_range() {
        let mut prev = 0.0;
        for w in STROKE_PRESETS {
            assert!(w > prev, "presets must be strictly increasing");
            assert!((1.0..=20.0).contains(&w));
            prev = w;
        }
    }

    /// `apply` pins the light theme — the dark variant is a
    /// follow-up effort, never a runtime state.
    #[test]
    fn apply_pins_light() {
        let ctx = Context::default();
        apply(&ctx);
        assert_eq!(ctx.theme(), egui::Theme::Light);
    }

    /// `apply` installs the paper desk as the panel fill, cream as
    /// the resting chip fill, and the forest accent as the selection
    /// fill with cream glyphs.
    #[test]
    fn apply_installs_paper_and_forest() {
        let ctx = Context::default();
        apply(&ctx);
        let style = ctx.style_of(egui::Theme::Light);
        let v = &style.visuals;
        assert_eq!(v.panel_fill, palette::PAPER);
        assert_eq!(v.window_fill, palette::PAPER_CREAM);
        assert_eq!(v.selection.bg_fill, palette::FOREST_MID);
        assert_eq!(v.widgets.inactive.bg_fill, palette::PAPER_CREAM);
        assert_eq!(v.widgets.hovered.bg_stroke.color, palette::FOREST);
        assert_eq!(v.widgets.active.fg_stroke.color, palette::CREAM_TEXT);
    }

    /// The window / popup shadows sit at the paper and pop levels —
    /// windows are paper, menus pop.
    #[test]
    fn window_and_popup_shadows_use_paper_and_pop() {
        let ctx = Context::default();
        apply(&ctx);
        let v = &ctx.style_of(egui::Theme::Light).visuals;
        assert_eq!(v.window_shadow, SHADOW_PAPER);
        assert_eq!(v.popup_shadow, SHADOW_POP);
    }

    /// Spec success criterion 1, machine-checked: `Color32`
    /// construction (`from_*`) appears only in `theme/palette.rs`
    /// — the token-only contract, enforced forever.
    #[test]
    fn color32_construction_lives_only_in_the_palette() {
        fn scan(dir: &std::path::Path, hits: &mut Vec<std::path::PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    scan(&path, hits);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let Ok(body) = std::fs::read_to_string(&path) else {
                        continue;
                    };
                    // The pattern is assembled at runtime so this
                    // test's own source doesn't trip the scan.
                    let needle = concat!("Color32", "::from_");
                    if body.contains(needle) {
                        hits.push(path);
                    }
                }
            }
        }
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut hits = Vec::new();
        scan(&src, &mut hits);
        let palette = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("theme")
            .join("palette.rs");
        for hit in &hits {
            assert_eq!(
                hit.canonicalize().ok(),
                palette.canonicalize().ok(),
                "Color32 construction outside theme/palette.rs: {hit:?}"
            );
        }
    }

    /// Button padding is measurably larger than the retired compact
    /// 7×3 instrument-panel density (spec: ~30–50% growth).
    #[test]
    fn button_padding_grew_past_the_instrument_panel() {
        assert!(BUTTON_PADDING.x > 7.0 * 1.3);
        assert!(BUTTON_PADDING.y > 3.0 * 1.5);
    }

    /// Pod padding grew past the retired 3 px island pad.
    #[test]
    fn pod_pad_grew_past_the_old_island() {
        assert!(POD_PAD > 3);
    }

    /// Jakarta Regular leads the proportional chain with the egui
    /// emoji fallbacks intact behind it (no tofu on emoji).
    #[test]
    fn jakarta_leads_proportional_with_emoji_fallback() {
        let fonts = font_definitions();
        let chain = &fonts.families[&egui::FontFamily::Proportional];
        assert_eq!(
            chain.first().map(String::as_str),
            Some("PlusJakartaSans-Regular"),
            "Jakarta must be the primary UI face"
        );
        assert!(
            chain.iter().any(|f| f.contains("emoji")),
            "the default emoji fallbacks must survive behind Jakarta"
        );
    }

    /// All three Jakarta weights are embedded — the spec's one
    /// family, three weights.
    #[test]
    fn all_three_jakarta_weights_are_embedded() {
        let fonts = font_definitions();
        for name in [
            "PlusJakartaSans-Regular",
            "PlusJakartaSans-Medium",
            "PlusJakartaSans-ExtraBold",
        ] {
            assert!(fonts.font_data.contains_key(name), "{name} missing");
        }
    }

    /// Display text maps to its own `ExtraBold` family — egui has no
    /// weight axis on `FontId`, so the weight lives in the family.
    #[test]
    fn display_family_maps_to_extra_bold() {
        let fonts = font_definitions();
        assert_eq!(
            fonts.families[&egui::FontFamily::Name(FAMILY_DISPLAY.into())],
            ["PlusJakartaSans-ExtraBold".to_owned()]
        );
    }

    /// Annotations keep the pre-Jakarta face: their named family
    /// pins the old Ubuntu-led chain so the text tool's output is
    /// unchanged by the UI re-skin (the annotation font is not the
    /// UI font).
    #[test]
    fn annotation_family_pins_the_pre_jakarta_face() {
        let fonts = font_definitions();
        let chain = &fonts.families[&egui::FontFamily::Name(FAMILY_ANNOTATION.into())];
        assert_eq!(chain.first().map(String::as_str), Some("Ubuntu-Light"));
        assert!(
            chain.iter().any(|f| f.contains("emoji")),
            "annotations keep emoji coverage"
        );
        assert!(
            !chain.iter().any(|f| f.starts_with("PlusJakartaSans")),
            "annotations must not silently become Jakarta"
        );
    }

    /// The monospace family (Hack) stays untouched — dims readouts,
    /// code-ish text, and Mono annotations keep their face.
    #[test]
    fn monospace_family_is_untouched() {
        let fonts = font_definitions();
        assert_eq!(
            fonts.families[&egui::FontFamily::Monospace]
                .first()
                .map(String::as_str),
            Some("Hack")
        );
    }
}
