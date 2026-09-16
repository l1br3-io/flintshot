//! The pop-up book palette — every `Color32` literal in this crate
//! lives here and nowhere else (the final-sweep contract).
//!
//! Role succession from the retired graphite/ember skin: **forest**
//! takes over as the active/selected accent (active tool chip, Copy
//! hero fill, selection outlines, links), **gold** is the hover-lift
//! ring / focus highlight. Cards and pods are white cutouts on cream
//! paper; ink is the only text family.

use egui::Color32;

/// Panel / window base fill (cream paper).
pub(crate) const PAPER_CREAM: Color32 = Color32::from_rgb(0xFF, 0xF9, 0xF0);
/// Card / pod fill and the 8 px cutout border stroke color.
pub(crate) const CUTOUT_WHITE: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
/// The primary accent: active tool, selection, hero Copy fill, links.
pub(crate) const FOREST_DEEP: Color32 = Color32::from_rgb(0x1F, 0x52, 0x33);
/// Hover / secondary interactive and success green.
pub(crate) const FOREST_MID: Color32 = Color32::from_rgb(0x2D, 0x7A, 0x3E);
/// Warm highlight: the hover-lift ring, focus, recents hover.
pub(crate) const AUTUMN_GOLD: Color32 = Color32::from_rgb(0xFF, 0xB3, 0x47);
/// Annotation-swatch orange, mid emphasis.
pub(crate) const FOX_ORANGE: Color32 = Color32::from_rgb(0xE6, 0x7E, 0x22);
/// Destructive (delete annotation) and the leading annotation swatch.
pub(crate) const BERRY_RED: Color32 = Color32::from_rgb(0xC0, 0x39, 0x2B);
/// Primary text on cream.
pub(crate) const INK: Color32 = Color32::from_rgb(0x22, 0x30, 0x1F);
/// Secondary text / hints — ink one step lighter.
pub(crate) const INK_SOFT: Color32 = Color32::from_rgb(0x6A, 0x77, 0x67);
/// Sky annotation swatch (kept bright for visibility on any
/// screenshot).
pub(crate) const SKY: Color32 = Color32::from_rgb(0x4C, 0xC3, 0xFF);
/// Hairlines and inner-chip borders (cream-tinted neutral).
pub(crate) const HOLIDAY_HAIRLINE: Color32 = Color32::from_rgb(0xE8, 0xE0, 0xD3);
/// Faint surfaces: alternating rows, troughs, resting-hover tints.
pub(crate) const HOLIDAY_FAINT: Color32 = Color32::from_rgb(0xF3, 0xED, 0xE2);

/// Move-tool hover outline ink: white at reduced alpha, legible over
/// both dark and light screenshots.
pub(crate) const MOVE_HOVER_INK: Color32 =
    Color32::from_rgba_unmultiplied_const(0xFF, 0xFF, 0xFF, 170);
/// Painted-shadow tint at the resting level (ink, α ≈ 0.30).
pub(crate) const SHADOW_RESTING: Color32 =
    Color32::from_rgba_unmultiplied_const(0x22, 0x30, 0x1F, 77);
/// Painted-shadow tint at the lifted level (ink, α = 0.40).
pub(crate) const SHADOW_LIFTED: Color32 =
    Color32::from_rgba_unmultiplied_const(0x22, 0x30, 0x1F, 102);

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

/// Annotation color presets shown as swatches in the picker bay:
/// berry leads (the classic annotation default), then fox, gold,
/// forest, sky; white and ink earn their slots for dark / light
/// screenshots. Re-anchored on the pop-up-book palette — same
/// seven-slot shape as the retired graphite set.
pub(crate) const SWATCHES: [Color32; 7] = [
    BERRY_RED,
    FOX_ORANGE,
    AUTUMN_GOLD,
    FOREST_MID,
    SKY,
    CUTOUT_WHITE,
    INK,
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The accent must never collide with a neutral or text token,
    /// or the "forest marks the active thing" contract breaks.
    #[test]
    fn forest_accent_stays_distinct_from_neutrals() {
        for neutral in [
            PAPER_CREAM,
            CUTOUT_WHITE,
            HOLIDAY_HAIRLINE,
            HOLIDAY_FAINT,
            INK,
            INK_SOFT,
        ] {
            assert_ne!(FOREST_DEEP, neutral);
        }
    }

    /// Gold (hover-lift ring) and forest (active) must differ, or
    /// hover feedback would masquerade as selection.
    #[test]
    fn gold_hover_ring_differs_from_forest_active() {
        assert_ne!(AUTUMN_GOLD, FOREST_DEEP);
        assert_ne!(AUTUMN_GOLD, FOREST_MID);
    }

    /// The swatch set keeps its seven slots, all distinct, with
    /// berry leading as the default annotation color.
    #[test]
    fn swatches_are_distinct_and_berry_leads() {
        let mut sorted = SWATCHES;
        sorted.sort_by_key(|c| (c.r(), c.g(), c.b()));
        for pair in sorted.windows(2) {
            assert_ne!(pair[0], pair[1]);
        }
        assert_eq!(SWATCHES[0], BERRY_RED, "berry red is the default swatch");
        assert_eq!(SWATCHES.len(), 7);
    }

    /// Cards are white cutouts that must read as a step off the
    /// cream paper — the container edge the hairline + shadow trace.
    #[test]
    fn cutout_white_steps_off_paper_cream() {
        let lum = |c: Color32| u32::from(c.r()) + u32::from(c.g()) + u32::from(c.b());
        assert!(
            lum(PAPER_CREAM) < lum(CUTOUT_WHITE),
            "the cutout fill must lift off the paper fill"
        );
    }

    /// Text ink must keep contrast on both paper surfaces.
    #[test]
    fn ink_is_darker_than_both_paper_surfaces() {
        let lum = |c: Color32| u32::from(c.r()) + u32::from(c.g()) + u32::from(c.b());
        assert!(lum(INK) < lum(PAPER_CREAM));
        assert!(lum(INK) < lum(CUTOUT_WHITE));
        assert!(lum(INK_SOFT) < lum(PAPER_CREAM));
    }

    /// Shadow alphas pin the two-level spec: resting ≈ 0.30,
    /// lifted = 0.40, and lifted is the denser of the pair.
    #[test]
    fn shadow_tints_pin_the_two_level_spec() {
        assert_eq!(SHADOW_RESTING.a(), 77);
        assert_eq!(SHADOW_LIFTED.a(), 102);
        assert!(SHADOW_RESTING.a() < SHADOW_LIFTED.a());
    }
}
