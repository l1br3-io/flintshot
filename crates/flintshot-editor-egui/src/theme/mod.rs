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
pub(crate) const SPACE_SCALE: [f32; 5] = [4.0, 8.0, 12.0, 16.0, 24.0];

/// Corner radius of a pod / card / popover, logical px.
pub(crate) const POD_RADIUS: u8 = 10;
/// Corner radius of an inner chip, logical px.
pub(crate) const CHIP_RADIUS: u8 = 7;

/// The cutout border: 8 px of white around every card and pod.
// Consumed by the docked side panel and welcome cards (tasks 005+);
// the toolbar pods sit on cream and use the hairline instead.
#[allow(dead_code)]
pub(crate) const CUTOUT_STROKE: Stroke = Stroke {
    width: 8.0,
    color: palette::CUTOUT_WHITE,
};

/// The 1 px cream-tinted hairline for inner chips and separators.
pub(crate) const HAIRLINE_STROKE: Stroke = Stroke {
    width: 1.0,
    color: palette::HOLIDAY_HAIRLINE,
};

/// Resting painted shadow: 10 px blur at α ≈ 0.30.
pub(crate) const SHADOW_RESTING: egui::Shadow = egui::Shadow {
    offset: [0, 2],
    blur: 10,
    spread: 0,
    color: palette::SHADOW_RESTING,
};

/// Lifted painted shadow (hover / pop-in): 30 px blur at α = 0.40.
pub(crate) const SHADOW_LIFTED: egui::Shadow = egui::Shadow {
    offset: [0, 4],
    blur: 30,
    spread: 0,
    color: palette::SHADOW_LIFTED,
};

/// Type ramp, logical px. Plus Jakarta Sans (Regular / Medium /
/// `ExtraBold`) lands with the fonts task; the sizes are the contract.
// Display size: consumed by the welcome hero card (task 010).
#[allow(dead_code)]
pub(crate) const TYPE_DISPLAY: f32 = 26.0;
/// Title / heading size, logical px.
pub(crate) const TYPE_TITLE: f32 = 16.0;
/// Body and control-label size, logical px.
pub(crate) const TYPE_BODY: f32 = 13.0;
/// Caption / hint size, logical px.
pub(crate) const TYPE_CAPTION: f32 = 11.0;

/// Inner padding between a pod island's edge and its chips, logical
/// px (i8 to match egui's `Margin`).
pub(crate) const POD_PAD: i8 = 6;

/// Extra gap between adjacent pod islands, logical px — on the
/// spacing scale, on top of the ambient item spacing, so pod
/// boundaries dominate the intra-pod chip spacing.
pub(crate) const POD_GAP: f32 = 8.0;

/// Widget button padding — visibly airier than the compact
/// instrument-panel density it replaces.
pub(crate) const BUTTON_PADDING: egui::Vec2 = egui::Vec2::new(10.0, 6.0);

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
        style.spacing.interact_size = egui::vec2(26.0, 26.0);
        style.spacing.slider_width = 120.0;
        style.spacing.slider_rail_height = 6.0;
    });
}

/// Build the cream-paper `Visuals`: white chips with hairline
/// borders on cream, a gold ring on hover, forest fills on
/// active/selected with white glyphs.
fn visuals() -> egui::Visuals {
    let mut v = egui::Visuals::light();
    v.panel_fill = palette::PAPER_CREAM;
    v.window_fill = palette::CUTOUT_WHITE;
    v.faint_bg_color = palette::HOLIDAY_FAINT;
    v.extreme_bg_color = palette::HOLIDAY_FAINT;
    v.hyperlink_color = palette::FOREST_DEEP;
    v.window_shadow = SHADOW_RESTING;
    v.popup_shadow = SHADOW_LIFTED;

    // Non-interactive: labels, separators, window outline.
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette::INK);
    v.widgets.noninteractive.bg_stroke = HAIRLINE_STROKE;
    v.widgets.noninteractive.bg_fill = palette::PAPER_CREAM;
    v.widgets.noninteractive.corner_radius = CornerRadius::same(CHIP_RADIUS);

    // Resting widgets: white cutout chips with hairline borders.
    v.widgets.inactive.weak_bg_fill = palette::CUTOUT_WHITE;
    v.widgets.inactive.bg_fill = palette::CUTOUT_WHITE;
    v.widgets.inactive.bg_stroke = HAIRLINE_STROKE;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, palette::INK);
    v.widgets.inactive.corner_radius = CornerRadius::same(CHIP_RADIUS);

    // Hover: the gold lift ring on a faint-cream fill.
    v.widgets.hovered.weak_bg_fill = palette::HOLIDAY_FAINT;
    v.widgets.hovered.bg_fill = palette::HOLIDAY_FAINT;
    v.widgets.hovered.bg_stroke = Stroke::new(1.25, palette::AUTUMN_GOLD);
    v.widgets.hovered.fg_stroke = Stroke::new(1.2, palette::INK);
    v.widgets.hovered.corner_radius = CornerRadius::same(CHIP_RADIUS);

    // Pressed / open / selected: forest-filled with white glyphs —
    // the chip reads as cut out of the panel.
    v.widgets.active.weak_bg_fill = palette::FOREST_DEEP;
    v.widgets.active.bg_fill = palette::FOREST_DEEP;
    v.widgets.active.bg_stroke = Stroke::new(1.0, palette::FOREST_DEEP);
    v.widgets.active.fg_stroke = Stroke::new(1.2, palette::CUTOUT_WHITE);
    v.widgets.active.corner_radius = CornerRadius::same(CHIP_RADIUS);
    v.widgets.open = v.widgets.hovered;

    // Selection (text spans + selected segmented options): forest
    // background with white foreground.
    v.selection.bg_fill = palette::FOREST_MID;
    v.selection.stroke = Stroke::new(1.2, palette::CUTOUT_WHITE);

    // Popup / window dressing.
    v.window_stroke = HAIRLINE_STROKE;
    v.window_corner_radius = CornerRadius::same(POD_RADIUS);
    v.menu_corner_radius = CornerRadius::same(POD_RADIUS);
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
        let want = [4.0_f32, 8.0, 12.0, 16.0, 24.0];
        for (got, want) in SPACE_SCALE.iter().zip(want) {
            assert_eq!(got.to_bits(), want.to_bits(), "step must be exact");
        }
        assert!(
            SPACE_SCALE.windows(2).all(|w| w[0] < w[1]),
            "the scale must be strictly increasing"
        );
    }

    /// A pod must round wider than its chips so the container /
    /// chip hierarchy stays readable.
    #[test]
    fn pod_radius_rounds_wider_than_chips() {
        assert!(POD_RADIUS > CHIP_RADIUS);
    }

    /// The two-level shadow spec: resting at 10 px, lifted at up to
    /// 30 px, lifted strictly softer and denser.
    #[test]
    fn shadows_pin_the_two_levels() {
        assert_eq!(SHADOW_RESTING.blur, 10);
        assert_eq!(SHADOW_LIFTED.blur, 30);
        assert!(SHADOW_RESTING.blur < SHADOW_LIFTED.blur);
        assert_ne!(SHADOW_RESTING, SHADOW_LIFTED);
    }

    /// The cutout stroke is the spec's 8 px white border.
    #[test]
    fn cutout_stroke_is_eight_px_white() {
        assert_eq!(CUTOUT_STROKE.width.to_bits(), 8.0_f32.to_bits());
        assert_eq!(CUTOUT_STROKE.color, palette::CUTOUT_WHITE);
    }

    /// The hairline is 1 px of cream-tinted neutral.
    #[test]
    fn hairline_is_one_px_neutral() {
        assert_eq!(HAIRLINE_STROKE.width.to_bits(), 1.0_f32.to_bits());
        assert_eq!(HAIRLINE_STROKE.color, palette::HOLIDAY_HAIRLINE);
    }

    /// The type ramp orders strictly downward from display to
    /// caption.
    #[test]
    fn type_ramp_orders_strictly() {
        assert!(TYPE_DISPLAY > TYPE_TITLE);
        assert!(TYPE_TITLE > TYPE_BODY);
        assert!(TYPE_BODY > TYPE_CAPTION);
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

    /// `apply` installs the cream paper as the panel fill and the
    /// forest accent as the selection fill.
    #[test]
    fn apply_installs_paper_and_forest() {
        let ctx = Context::default();
        apply(&ctx);
        let style = ctx.style_of(egui::Theme::Light);
        assert_eq!(style.visuals.panel_fill, palette::PAPER_CREAM);
        assert_eq!(style.visuals.selection.bg_fill, palette::FOREST_MID);
        assert_eq!(
            style.visuals.widgets.inactive.bg_fill,
            palette::CUTOUT_WHITE
        );
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
