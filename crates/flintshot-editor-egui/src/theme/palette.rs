//! The pop-up book palette of record (spec §Color tokens) — every
//! `Color32` literal in this crate lives here and nowhere else (the
//! final-sweep contract).
//!
//! Surfaces: a warm paper desk with cream cards and dim wells. The
//! forest family carries structure (topbar, rail, hovers, borders);
//! gold is the active-tool / accent metal; ink is the only text on
//! paper, cream text the only text on forest. The graphite/ember
//! skin and the v1 popup-book accent values are retired — see the
//! retired-hex test at the bottom.

use egui::Color32;

/// Topbar + statusbar fills, home hero card.
pub(crate) const FOREST_DEEP: Color32 = Color32::from_rgb(0x1F, 0x52, 0x33);
/// Left rail fill, well-selected fill.
pub(crate) const FOREST_MID: Color32 = Color32::from_rgb(0x2C, 0x6B, 0x45);
/// Hovers, borders.
pub(crate) const FOREST: Color32 = Color32::from_rgb(0x3A, 0x80, 0x58);
/// Deepest shade, text on gold.
// Lands as a consumer-facing token with the widgets (switch glyph)
// and rail tasks; the shadow tints below bake its rgb directly.
#[allow(dead_code)]
pub(crate) const FOREST_INK: Color32 = Color32::from_rgb(0x16, 0x38, 0x1F);
/// Active tool, brand badge, statusbar hint, switches.
// Consumed from the widgets / rail tasks on; visuals() still hovers
// with the forest border until then.
#[allow(dead_code)]
pub(crate) const GOLD: Color32 = Color32::from_rgb(0xE4, 0xB5, 0x4A);
/// Gold border / pressed, annotation accents.
// Consumed with the switch widget (task 002).
#[allow(dead_code)]
pub(crate) const GOLD_DEEP: Color32 = Color32::from_rgb(0xC9, 0x9A, 0x2E);
/// Canvas desk, panel base.
pub(crate) const PAPER: Color32 = Color32::from_rgb(0xF6, 0xEF, 0xDE);
/// Cards, controls, chips.
pub(crate) const PAPER_CREAM: Color32 = Color32::from_rgb(0xFB, 0xF6, 0xEA);
/// Wells, insets, disabled — also the hairline neutral.
pub(crate) const PAPER_DIM: Color32 = Color32::from_rgb(0xED, 0xE4, 0xCE);
/// Primary text on paper.
pub(crate) const INK: Color32 = Color32::from_rgb(0x2A, 0x26, 0x20);
/// Secondary text, labels.
pub(crate) const INK_SOFT: Color32 = Color32::from_rgb(0x6E, 0x67, 0x54);
/// Text on forest fills.
pub(crate) const CREAM_TEXT: Color32 = Color32::from_rgb(0xF3, 0xED, 0xDD);
/// Muted text on forest fills.
// Consumed with the topbar (task 005).
#[allow(dead_code)]
pub(crate) const CREAM_DIM: Color32 = Color32::from_rgb(0xC9, 0xC2, 0xAC);
/// Gated tools, disabled.
// Consumed with the rail's locked tools (task 004).
#[allow(dead_code)]
pub(crate) const LOCK_GREY: Color32 = Color32::from_rgb(0x9A, 0x94, 0x7F);
/// Destructive accents (selection delete tint).
pub(crate) const BERRY_RED: Color32 = Color32::from_rgb(0xC0, 0x39, 0x2B);
/// Sky annotation swatch (kept bright for visibility on any
/// screenshot).
pub(crate) const SKY: Color32 = Color32::from_rgb(0x4C, 0xC3, 0xFF);

/// Move-tool hover outline ink: white at reduced alpha, legible over
/// both dark and light screenshots.
pub(crate) const MOVE_HOVER_INK: Color32 =
    Color32::from_rgba_unmultiplied_const(0xFF, 0xFF, 0xFF, 170);

/// Painted-shadow tint at the resting level (forest ink, α ≈ 0.28).
pub(crate) const SHADOW_RESTING: Color32 =
    Color32::from_rgba_unmultiplied_const(0x16, 0x38, 0x1F, 71);
/// Painted-shadow tint at the lift level (forest ink, α ≈ 0.32).
pub(crate) const SHADOW_LIFT: Color32 = Color32::from_rgba_unmultiplied_const(0x16, 0x38, 0x1F, 82);
/// Painted-shadow tint at the pop level (forest ink, α ≈ 0.38).
pub(crate) const SHADOW_POP: Color32 = Color32::from_rgba_unmultiplied_const(0x16, 0x38, 0x1F, 97);
/// Painted-shadow tint at the paper level (forest ink, α ≈ 0.28).
pub(crate) const SHADOW_PAPER: Color32 =
    Color32::from_rgba_unmultiplied_const(0x16, 0x38, 0x1F, 71);

/// Convert a core [`flintshot_core::geom::Color`] (unpremultiplied)
/// into egui's premultiplied space. The converter lives here so
/// `Color32` construction has exactly one home (the token-only
/// scan's contract); values are runtime, not tokens.
pub(crate) fn egui_color(c: flintshot_core::geom::Color) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

/// A core color forced opaque (swatch rings compare by rgb).
pub(crate) fn opaque(c: flintshot_core::geom::Color) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, 255)
}

/// An opaque token color at a new alpha — the cream-tinted hovers
/// over forest fills, the dimmed borders. Opaque sources only: the
/// premultiplied round-trip is lossless at α 255 and lossy below.
pub(crate) fn with_alpha(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

/// Premultiplied egui color from raw components (thumbnail pixels,
/// popover swatches) — runtime values, not tokens.
pub(crate) fn premultiplied(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// Functional black at `alpha` — readability chrome that must sit
/// legibly over arbitrary screenshots (the overlay freeze mask,
/// badge backgrounds, the loupe's border). Not a skin color; the
/// only non-token color constructors the crate allows.
pub(crate) fn readability_black(alpha: u8) -> Color32 {
    Color32::from_black_alpha(alpha)
}

/// Annotation color presets shown as swatches in the picker bay,
/// the mockup's refreshed six: berry leads (the classic annotation
/// default), then gold, forest, sky; ink and cream-white earn their
/// slots for dark / light screenshots.
pub(crate) const SWATCHES: [Color32; 6] = [BERRY_RED, GOLD, FOREST_MID, SKY, INK, PAPER_CREAM];

#[cfg(test)]
mod tests {
    use super::*;

    /// The accent family must never collide with a neutral or text
    /// token, or the "forest marks the active thing" contract breaks.
    #[test]
    fn forest_family_stays_distinct_from_neutrals() {
        for forest in [FOREST_DEEP, FOREST_MID, FOREST] {
            for neutral in [
                PAPER,
                PAPER_CREAM,
                PAPER_DIM,
                LOCK_GREY,
                INK,
                INK_SOFT,
                CREAM_TEXT,
                CREAM_DIM,
            ] {
                assert_ne!(forest, neutral);
            }
        }
    }

    /// Gold (active tool) must differ from every forest step, or
    /// active-tool metal would masquerade as structure.
    #[test]
    fn gold_differs_from_the_forest_family() {
        for forest in [FOREST_DEEP, FOREST_MID, FOREST, FOREST_INK] {
            assert_ne!(GOLD, forest);
            assert_ne!(GOLD_DEEP, forest);
        }
    }

    /// The swatch set keeps the mockup's six slots, all distinct,
    /// with berry leading as the default annotation color.
    #[test]
    fn swatches_are_the_six_mockup_slots_with_berry_leading() {
        let mut sorted = SWATCHES;
        sorted.sort_by_key(|c| (c.r(), c.g(), c.b()));
        for pair in sorted.windows(2) {
            assert_ne!(pair[0], pair[1]);
        }
        assert_eq!(SWATCHES[0], BERRY_RED, "berry red is the default swatch");
        assert_eq!(SWATCHES.len(), 6);
    }

    /// Cards (cream) must read as a step off the panel base (paper)
    /// — the desk / card hierarchy the hairline + shadow trace.
    #[test]
    fn paper_cream_cards_lift_off_paper() {
        let lum = |c: Color32| u32::from(c.r()) + u32::from(c.g()) + u32::from(c.b());
        assert!(lum(PAPER) < lum(PAPER_CREAM));
        assert!(lum(PAPER_DIM) < lum(PAPER_CREAM));
        assert!(lum(PAPER_DIM) < lum(PAPER));
    }

    /// Text ink must keep contrast on both paper surfaces, and cream
    /// text must stay lighter than every forest fill.
    #[test]
    fn text_keeps_contrast_on_its_surfaces() {
        let lum = |c: Color32| u32::from(c.r()) + u32::from(c.g()) + u32::from(c.b());
        assert!(lum(INK) < lum(PAPER));
        assert!(lum(INK) < lum(PAPER_CREAM));
        assert!(lum(INK_SOFT) < lum(PAPER));
        for forest in [FOREST_DEEP, FOREST_MID, FOREST] {
            assert!(lum(CREAM_TEXT) > lum(forest), "cream text on {forest:?}");
        }
    }

    /// Shadow alphas pin the four-level spec: resting < lift < pop,
    /// paper at the soft resting alpha, all forest-ink hued.
    #[test]
    fn shadow_tints_pin_the_four_level_spec() {
        assert_eq!(SHADOW_RESTING.a(), 71);
        assert_eq!(SHADOW_LIFT.a(), 82);
        assert_eq!(SHADOW_POP.a(), 97);
        assert_eq!(SHADOW_PAPER.a(), 71);
        assert!(SHADOW_RESTING.a() < SHADOW_LIFT.a());
        assert!(SHADOW_LIFT.a() < SHADOW_POP.a());
        for tint in [SHADOW_RESTING, SHADOW_LIFT, SHADOW_POP, SHADOW_PAPER] {
            let expected = Color32::from_rgba_unmultiplied(
                FOREST_INK.r(),
                FOREST_INK.g(),
                FOREST_INK.b(),
                tint.a(),
            );
            assert_eq!(tint, expected, "shadows are forest-ink shadows");
        }
    }

    /// The retired v1 / graphite-era hex values must never return:
    /// scan this file's own source for the old literals. The hexes
    /// are assembled from parts at runtime so this test's own
    /// source doesn't contain them.
    #[test]
    fn retired_palette_values_stay_retired() {
        let body = include_str!("palette.rs");
        for parts in [
            ["FF", "B3", "47"],
            ["2D", "7A", "3E"],
            ["FF", "F9", "F0"],
            ["E6", "7E", "22"],
            ["E8", "E0", "D3"],
            ["F3", "ED", "E2"],
        ] {
            let hex = parts.concat();
            assert!(
                !body.contains(&hex),
                "retired palette value {hex} reappeared in palette.rs"
            );
        }
    }
}
