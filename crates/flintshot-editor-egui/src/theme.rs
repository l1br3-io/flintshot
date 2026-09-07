//! The flintshot editor theme: a graphite instrument panel with a
//! single ember accent ("a flint strikes a spark").
//!
//! The editor window is an instrument, not a page — the toolbar
//! recedes behind the screenshot it annotates. Everything is
//! graphite; ember appears in exactly four places: the active tool
//! chip, the Copy primary action, the grabbed-annotation outline
//! under the Move tool, and the selected-annotation outline. Nothing
//! else gets color.

use egui::{Color32, Context, CornerRadius, Stroke};

/// Panel background (graphite-900).
pub(crate) const GRAPHITE_900: Color32 = Color32::from_rgb(0x15, 0x17, 0x1B);
/// Widget fill (graphite-700).
pub(crate) const GRAPHITE_700: Color32 = Color32::from_rgb(0x23, 0x26, 0x2C);
/// Hairlines, borders, hover fill (graphite-500).
pub(crate) const GRAPHITE_500: Color32 = Color32::from_rgb(0x34, 0x39, 0x41);
/// Primary text (chalk).
pub(crate) const CHALK: Color32 = Color32::from_rgb(0xE8, 0xEA, 0xEE);
/// Secondary text (slate) — hints, dims readouts.
pub(crate) const SLATE: Color32 = Color32::from_rgb(0x9A, 0xA1, 0xAC);
/// The ember accent: active tool chip, Copy button fill, and the
/// grabbed- / selected-annotation outlines under Move — the "this is
/// the active thing" surfaces.
pub(crate) const EMBER: Color32 = Color32::from_rgb(0xFF, 0x8C, 0x42);
/// Glyph color on ember fills (matches the panel graphite so
/// selected chips read as "cut out" of the toolbar).
pub(crate) const INK: Color32 = GRAPHITE_900;

/// Move-tool hover outline (see `crate::chrome`): 1 px dashed chalk —
/// the "grab me" invitation, deliberately subordinate to the solid
/// white selection chrome.
pub(crate) const MOVE_HOVER_STROKE: Stroke = Stroke {
    width: 1.0,
    color: Color32::from_rgba_unmultiplied_const(0xE8, 0xEA, 0xEE, 170),
};
/// Move-tool grab outline: 2 px solid ember — the active-object
/// emphasis while a move drag is live.
pub(crate) const MOVE_GRAB_STROKE: Stroke = Stroke {
    width: 2.0,
    color: EMBER,
};
/// Persistent-selection outline (see `crate::chrome`): 1 px dashed
/// ember on the move chrome's dash / pad rhythm — the "this one is
/// selected" affordance the Del key acts on.
pub(crate) const SELECTED_STROKE: Stroke = Stroke {
    width: 1.0,
    color: EMBER,
};
/// Dash / gap rhythm of the hover outline, logical px.
pub(crate) const MOVE_DASH: f32 = 4.0;
pub(crate) const MOVE_GAP: f32 = 3.0;
/// Padding between an annotation's bbox and its move chrome, logical
/// px — keeps the outline off the annotation's own ink.
pub(crate) const MOVE_CHROME_PAD: f32 = 3.0;

/// Annotation color presets shown as swatches in the toolbar bay.
/// Signal red leads (the classic annotation default); ink earns its
/// slot for dark-UI screenshots.
pub(crate) const SWATCHES: [Color32; 7] = [
    Color32::from_rgb(0xFF, 0x52, 0x52), // signal
    Color32::from_rgb(0xFF, 0x8C, 0x42), // ember
    Color32::from_rgb(0xFF, 0xD6, 0x0A), // lemon
    Color32::from_rgb(0x3D, 0xDC, 0x97), // mint
    Color32::from_rgb(0x4C, 0xC3, 0xFF), // sky
    Color32::from_rgb(0xFF, 0xFF, 0xFF), // white
    Color32::from_rgb(0x1A, 0x1D, 0x23), // ink
];

/// Stroke-width presets behind the S / M / L dots.
pub(crate) const STROKE_PRESETS: [f32; 3] = [2.0, 4.0, 8.0];

/// Apply the flintshot theme to the editor's [`Context`]. Called
/// once at app construction; the styling persists for the window's
/// lifetime (both toolbar rows, popups, and text-editing caret).
pub(crate) fn apply(ctx: &Context) {
    // egui 0.35 keeps one style per theme; pin the dark one and
    // override both its visuals and its density.
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.set_visuals(visuals());
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        // Compact instrument-panel density.
        style.spacing.button_padding = egui::vec2(7.0, 3.0);
        style.spacing.item_spacing = egui::vec2(5.0, 4.0);
        style.spacing.interact_size = egui::vec2(20.0, 20.0);
        style.spacing.slider_width = 90.0;
        style.spacing.slider_rail_height = 4.0;
    });
}

/// Build the graphite `Visuals`. Selected widgets (and selected text)
/// invert to ember-on-ink so any egui `selectable_value` / selection
/// highlight inherits the accent for free.
fn visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = GRAPHITE_900;
    v.window_fill = GRAPHITE_700;
    v.faint_bg_color = GRAPHITE_700;
    v.extreme_bg_color = GRAPHITE_900;
    v.hyperlink_color = EMBER;

    // Non-interactive: labels, separators, window outline.
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, CHALK);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, GRAPHITE_500);
    v.widgets.noninteractive.bg_fill = GRAPHITE_700;
    v.widgets.noninteractive.corner_radius = CornerRadius::same(5);

    // Resting widgets: graphite chips with hairline borders.
    v.widgets.inactive.weak_bg_fill = GRAPHITE_700;
    v.widgets.inactive.bg_fill = GRAPHITE_700;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, GRAPHITE_500);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, CHALK);
    v.widgets.inactive.corner_radius = CornerRadius::same(5);

    // Hover: lift to the lighter graphite.
    v.widgets.hovered.weak_bg_fill = GRAPHITE_500;
    v.widgets.hovered.bg_fill = GRAPHITE_500;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, GRAPHITE_500);
    v.widgets.hovered.fg_stroke = Stroke::new(1.2, CHALK);
    v.widgets.hovered.corner_radius = CornerRadius::same(5);

    // Pressed / open: same fill, ember text.
    v.widgets.active.weak_bg_fill = GRAPHITE_500;
    v.widgets.active.bg_fill = GRAPHITE_500;
    v.widgets.active.bg_stroke = Stroke::new(1.0, GRAPHITE_500);
    v.widgets.active.fg_stroke = Stroke::new(1.2, EMBER);
    v.widgets.active.corner_radius = CornerRadius::same(5);
    v.widgets.open = v.widgets.hovered;

    // Selection (text spans + selected segmented options): ember
    // background with ink foreground.
    v.selection.bg_fill = EMBER;
    v.selection.stroke = Stroke::new(1.2, INK);

    // Popup / window dressing.
    v.window_stroke = Stroke::new(1.0, GRAPHITE_500);
    v.window_corner_radius = CornerRadius::same(7);
    v.menu_corner_radius = CornerRadius::same(7);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swatches_are_distinct() {
        let mut sorted = SWATCHES;
        sorted.sort_by_key(|c| (c.r(), c.g(), c.b()));
        for pair in sorted.windows(2) {
            assert_ne!(pair[0], pair[1]);
        }
    }

    #[test]
    fn stroke_presets_are_ordered_and_in_slider_range() {
        let mut prev = 0.0;
        for w in STROKE_PRESETS {
            assert!(w > prev, "presets must be strictly increasing");
            assert!((1.0..=20.0).contains(&w));
            prev = w;
        }
    }

    #[test]
    fn accent_and_fills_stay_distinct() {
        // Ember must never collide with a graphite or text token, or
        // the "only color on the rail" contract breaks.
        for grey in [GRAPHITE_900, GRAPHITE_700, GRAPHITE_500, CHALK, SLATE, INK] {
            assert_ne!(EMBER, grey);
        }
    }
}
