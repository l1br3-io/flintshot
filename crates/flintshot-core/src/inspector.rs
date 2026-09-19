//! Selection-inspector policy: capabilities + property mutators.
//!
//! The right panel's selection mode (multi-select-properties spec)
//! is a **selection inspector**: it shows the intersection of
//! properties shared by every selected annotation, and edits apply
//! live to the selected annotations, undoably. This module owns that
//! policy — the pure capabilities query (the spec's property ×
//! annotation-type matrix) and one mutator per property section —
//! so both shells inherit identical semantics.
//!
//! Mutators:
//! - write only the fields every selected annotation actually
//!   carries (non-supporting members are skipped),
//! - re-apply [`MARKER_ALPHA`](crate::freehand::MARKER_ALPHA) on
//!   marker color edits (the translucent-highlight contract,
//!   ADR-0022),
//! - sync the corresponding session tool defaults so the next draw
//!   matches what the inspector just set,
//! - push **one** undo snapshot per edit burst via kind-based
//!   coalescing (see [`EditorState::end_property_edit`]),
//! - never touch `pending` or the selection set, and are no-ops on
//!   an empty selection.

use crate::editor::{Annotation, BlurKind, EditorState, Snapshot};
use crate::freehand::MARKER_ALPHA;
use crate::geom::Color;
use crate::text::TextStyle;
use crate::{NUMBER_MAX_DIAMETER, NUMBER_MIN_DIAMETER};

/// Which property sections the current selection supports.
///
/// A section is `true` iff **every** selected annotation supports it
/// (the spec's intersection rule — an empty intersection renders a
/// count + delete-only panel, never a control that would silently
/// no-op on part of the set). All fields are `false` on an empty
/// selection.
// Same reasoning as `input::Modifiers`: these are independent
// capability bits (fill and text_style can be true or false in any
// combination), not option-like toggles a state machine would model
// better.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent capability bits, not option-like states"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct SelectionCapabilities {
    /// Stroke colour section (text members contribute their text
    /// colour; blur has none).
    pub stroke_color: bool,
    /// Stroke width section (strokes and freehand widths only).
    pub stroke_width: bool,
    /// Closed-shape interior fill section (rect / ellipse only).
    pub fill: bool,
    /// Text style section (the popover's fields, text only).
    pub text_style: bool,
    /// Effects section (blur kind + strength, blur only).
    pub effects: bool,
    /// Number size section (bubble diameter, number only).
    pub number_size: bool,
}

/// The coalescing identity of a property edit: consecutive mutator
/// calls of the same kind share one undo snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PropertyEditKind {
    /// Stroke / text / bubble colour section.
    Color,
    /// Stroke width section.
    StrokeWidth,
    /// Fill switch section.
    Fill,
    /// Text style (popover) section.
    TextStyle,
    /// Blur effects section.
    Blur,
    /// Number diameter section.
    NumberSize,
}

impl EditorState {
    /// The intersection of property sections the selection supports.
    ///
    /// Pure query over `selected` + `anns`; never mutates. See
    /// [`SelectionCapabilities`] for the intersection rule.
    #[must_use]
    pub fn selected_capabilities(&self) -> SelectionCapabilities {
        SelectionCapabilities {
            stroke_color: self.all_selected_support(supports_stroke_color),
            stroke_width: self.all_selected_support(supports_stroke_width),
            fill: self.all_selected_support(supports_fill),
            text_style: self.all_selected_support(supports_text_style),
            effects: self.all_selected_support(supports_effects),
            number_size: self.all_selected_support(supports_number_size),
        }
    }

    /// `true` iff the selection is non-empty and every member
    /// satisfies `support`. Stale indices can't occur (every
    /// list-mutating path clears the set) but spoil the section
    /// rather than granting it, so corrupted state fails closed.
    fn all_selected_support(&self, support: impl Fn(&Annotation) -> bool) -> bool {
        !self.selected.is_empty()
            && self
                .selected
                .iter()
                .all(|&index| self.anns.get(index).is_some_and(&support))
    }

    /// Open (or continue) a property-edit burst of `kind`, pushing
    /// the pre-edit `anns` snapshot only when the burst is new.
    fn begin_property_edit(&mut self, kind: PropertyEditKind) {
        if self.last_edit_kind != Some(kind) {
            self.undo
                .push(Snapshot::anns_only(self.anns.clone(), self.next_number));
            self.redo.clear();
            self.last_edit_kind = Some(kind);
        }
    }

    /// Set the stroke colour of every colour-capable selected
    /// annotation (text members get their text colour, number
    /// members their bubble colour; markers re-apply
    /// [`MARKER_ALPHA`]). Syncs the `color` default (and
    /// `text_color` when a text member was written).
    ///
    /// No-op on an empty selection.
    pub fn set_selected_color(&mut self, color: Color) {
        if self.selected.is_empty() {
            return;
        }
        self.begin_property_edit(PropertyEditKind::Color);
        let mut wrote_text = false;
        for index in self.selected.clone() {
            let Some(ann) = self.anns.get_mut(index) else {
                continue;
            };
            match ann {
                Annotation::Rect { stroke, .. }
                | Annotation::Ellipse { stroke, .. }
                | Annotation::Line { stroke, .. }
                | Annotation::Arrow { stroke, .. } => stroke.color = color,
                Annotation::Marker { color: c, .. } => *c = color.with_alpha(MARKER_ALPHA),
                Annotation::Pencil { color: c, .. } | Annotation::Number { color: c, .. } => {
                    *c = color;
                }
                Annotation::Text { color: c, .. } => {
                    *c = color;
                    wrote_text = true;
                }
                Annotation::Blur { .. } => {}
            }
        }
        self.color = color;
        if wrote_text {
            self.text_color = color;
        }
    }

    /// Set the stroke / freehand width of every width-capable
    /// selected annotation. Syncs the `stroke_width` default.
    ///
    /// No-op on an empty selection.
    pub fn set_selected_stroke_width(&mut self, width: f32) {
        if self.selected.is_empty() {
            return;
        }
        self.begin_property_edit(PropertyEditKind::StrokeWidth);
        for index in self.selected.clone() {
            let Some(ann) = self.anns.get_mut(index) else {
                continue;
            };
            match ann {
                Annotation::Rect { stroke, .. }
                | Annotation::Ellipse { stroke, .. }
                | Annotation::Line { stroke, .. }
                | Annotation::Arrow { stroke, .. } => stroke.width = width,
                Annotation::Marker { width: w, .. } | Annotation::Pencil { width: w, .. } => {
                    *w = width;
                }
                Annotation::Text { .. } | Annotation::Number { .. } | Annotation::Blur { .. } => {}
            }
        }
        self.stroke_width = width;
    }

    /// Toggle the interior fill of every fill-capable selected
    /// annotation (fill colour = the annotation's own stroke
    /// colour, matching `begin_annotation`). Syncs the
    /// `fill_shapes` default.
    ///
    /// No-op on an empty selection.
    pub fn set_selected_fill(&mut self, on: bool) {
        if self.selected.is_empty() {
            return;
        }
        self.begin_property_edit(PropertyEditKind::Fill);
        for index in self.selected.clone() {
            let Some(ann) = self.anns.get_mut(index) else {
                continue;
            };
            match ann {
                Annotation::Rect { fill, stroke, .. }
                | Annotation::Ellipse { fill, stroke, .. } => {
                    *fill = on.then_some(stroke.color);
                }
                _ => {}
            }
        }
        self.fill_shapes = on;
    }

    /// Stamp a complete [`TextStyle`] into every text-capable
    /// selected annotation, and sync every text session default
    /// from it. No-op on an empty selection.
    pub fn set_selected_text_style(&mut self, style: TextStyle) {
        if self.selected.is_empty() {
            return;
        }
        self.begin_property_edit(PropertyEditKind::TextStyle);
        for index in self.selected.clone() {
            let Some(ann) = self.anns.get_mut(index) else {
                continue;
            };
            if let Annotation::Text {
                font_size,
                color,
                family,
                weight,
                style: font_style,
                align,
                line_spacing,
                outline,
                background,
                ..
            } = ann
            {
                *font_size = style.size;
                *color = style.color;
                *family = style.family;
                *weight = style.weight;
                *font_style = style.style;
                *align = style.align;
                *line_spacing = style.line_spacing;
                *outline = style.outline;
                *background = style.background;
            }
        }
        self.text_color = style.color;
        self.font_size = style.size;
        self.font_family = style.family;
        self.font_weight = style.weight;
        self.font_style = style.style;
        self.text_align = style.align;
        self.line_spacing = style.line_spacing;
        self.text_outline = style.outline;
        self.text_background = style.background;
    }

    /// Set the blur kind and strength of every effects-capable
    /// selected annotation. Syncs the `blur_kind` default (strength
    /// has no session default).
    ///
    /// No-op on an empty selection.
    pub fn set_selected_blur(&mut self, kind: BlurKind, strength: u8) {
        if self.selected.is_empty() {
            return;
        }
        self.begin_property_edit(PropertyEditKind::Blur);
        for index in self.selected.clone() {
            let Some(ann) = self.anns.get_mut(index) else {
                continue;
            };
            if let Annotation::Blur {
                kind: k,
                strength: s,
                ..
            } = ann
            {
                *k = kind;
                *s = strength;
            }
        }
        self.blur_kind = kind;
    }

    /// Set the bubble diameter (clamped to
    /// [`NUMBER_MIN_DIAMETER`]..=[`NUMBER_MAX_DIAMETER`]) of every
    /// number-capable selected annotation. Syncs the
    /// `number_diameter` default.
    ///
    /// No-op on an empty selection.
    pub fn set_selected_number_diameter(&mut self, diameter: f32) {
        if self.selected.is_empty() {
            return;
        }
        let clamped = diameter.clamp(NUMBER_MIN_DIAMETER, NUMBER_MAX_DIAMETER);
        self.begin_property_edit(PropertyEditKind::NumberSize);
        for index in self.selected.clone() {
            let Some(ann) = self.anns.get_mut(index) else {
                continue;
            };
            if let Annotation::Number { diameter, .. } = ann {
                *diameter = clamped;
            }
        }
        self.number_diameter = clamped;
    }

    /// Close the current property-edit burst.
    ///
    /// Undo coalescing is kind-based: a mutator pushes a snapshot
    /// only when its kind differs from the previous mutator call's,
    /// so a continuous widget (`DragValue`, slider, colour editor)
    /// dragged across many frames costs one undo entry. Shells call
    /// this when the interaction *ends* (egui `drag_stopped` /
    /// `lost_focus`) or after a discrete click, so the next gesture
    /// starts a fresh entry.
    ///
    /// # Worked example
    ///
    /// ```text
    /// set_selected_color(a)   // snapshot S1 (pre-edit anns), kind=Color
    /// set_selected_color(b)   // same kind → no snapshot
    /// set_selected_color(c)   // same kind → no snapshot
    /// undo                    // restores S1: one entry for a→b→c
    /// ```
    ///
    /// Any non-property mutation (commit, move, delete, crop, undo,
    /// redo, number placement) also breaks the burst.
    pub fn end_property_edit(&mut self) {
        self.last_edit_kind = None;
    }
}

/// Stroke-colour support: every annotation except blur (text
/// contributes its text colour, number its bubble colour).
fn supports_stroke_color(ann: &Annotation) -> bool {
    !matches!(ann, Annotation::Blur { .. })
}

/// Stroke-width support: stroked shapes + freehand widths.
fn supports_stroke_width(ann: &Annotation) -> bool {
    matches!(
        ann,
        Annotation::Rect { .. }
            | Annotation::Ellipse { .. }
            | Annotation::Line { .. }
            | Annotation::Arrow { .. }
            | Annotation::Marker { .. }
            | Annotation::Pencil { .. }
    )
}

/// Interior-fill support: the closed shapes only.
fn supports_fill(ann: &Annotation) -> bool {
    matches!(ann, Annotation::Rect { .. } | Annotation::Ellipse { .. })
}

/// Text-style support: text annotations only.
fn supports_text_style(ann: &Annotation) -> bool {
    matches!(ann, Annotation::Text { .. })
}

/// Effects support: blur annotations only.
fn supports_effects(ann: &Annotation) -> bool {
    matches!(ann, Annotation::Blur { .. })
}

/// Number-size support: number bubbles only.
fn supports_number_size(ann: &Annotation) -> bool {
    matches!(ann, Annotation::Number { .. })
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::editor::Annotation::{
        Arrow, Blur, Ellipse, Line, Marker, Number, Pencil, Rect, Text,
    };
    use crate::editor::Tool;
    use crate::geom::{Point, Rect as GeomRect, Stroke};
    use crate::text::{
        FontFamily, FontStyle, FontWeight, TextAlign, TextOutline, DEFAULT_LINE_SPACING,
    };

    fn blank() -> EditorState {
        EditorState::new(image::RgbaImage::new(2, 2))
    }

    fn state_with(anns: Vec<Annotation>) -> EditorState {
        let mut s = blank();
        s.tool = Tool::Hand;
        let n = anns.len();
        s.anns = anns;
        s.selected = (0..n).collect();
        s
    }

    fn rect_ann() -> Annotation {
        Rect {
            rect: GeomRect::from_xywh(0.0, 0.0, 10.0, 10.0),
            stroke: Stroke::new(Color::RED, 3.0),
            fill: None,
        }
    }

    fn ellipse_ann() -> Annotation {
        Ellipse {
            rect: GeomRect::from_xywh(0.0, 0.0, 10.0, 10.0),
            stroke: Stroke::new(Color::RED, 3.0),
            fill: None,
        }
    }

    fn line_ann() -> Annotation {
        Line {
            a: Point::new(0.0, 0.0),
            b: Point::new(10.0, 10.0),
            stroke: Stroke::new(Color::RED, 3.0),
        }
    }

    fn arrow_ann() -> Annotation {
        Arrow {
            a: Point::new(0.0, 0.0),
            b: Point::new(10.0, 10.0),
            stroke: Stroke::new(Color::RED, 3.0),
            head_size: 10.0,
        }
    }

    fn marker_ann() -> Annotation {
        Marker {
            points: vec![Point::new(0.0, 0.0), Point::new(5.0, 5.0)],
            color: Color::RED.with_alpha(MARKER_ALPHA),
            width: 3.0,
        }
    }

    fn pencil_ann() -> Annotation {
        Pencil {
            points: vec![Point::new(0.0, 0.0), Point::new(5.0, 5.0)],
            color: Color::RED,
            width: 3.0,
        }
    }

    fn text_ann() -> Annotation {
        Text {
            pos: Point::new(0.0, 0.0),
            text: "hi".into(),
            font_size: 16.0,
            color: Color::RED,
            family: FontFamily::default(),
            weight: FontWeight::default(),
            style: FontStyle::default(),
            align: TextAlign::default(),
            line_spacing: DEFAULT_LINE_SPACING,
            outline: None,
            background: None,
        }
    }

    fn blur_ann() -> Annotation {
        Blur {
            rect: GeomRect::from_xywh(0.0, 0.0, 10.0, 10.0),
            kind: BlurKind::Gaussian,
            strength: 8,
        }
    }

    fn number_ann() -> Annotation {
        Number {
            pos: Point::new(5.0, 5.0),
            n: 1,
            color: Color::RED,
            diameter: 28.0,
        }
    }

    // --- Capabilities matrix (spec: property × annotation type) ---

    #[test]
    fn caps_matrix_per_annotation_type() {
        for (name, ann, color, width, fill, text, effects, number) in [
            ("rect", rect_ann(), true, true, true, false, false, false),
            (
                "ellipse",
                ellipse_ann(),
                true,
                true,
                true,
                false,
                false,
                false,
            ),
            ("line", line_ann(), true, true, false, false, false, false),
            ("arrow", arrow_ann(), true, true, false, false, false, false),
            (
                "marker",
                marker_ann(),
                true,
                true,
                false,
                false,
                false,
                false,
            ),
            (
                "pencil",
                pencil_ann(),
                true,
                true,
                false,
                false,
                false,
                false,
            ),
            ("text", text_ann(), true, false, false, true, false, false),
            ("blur", blur_ann(), false, false, false, false, true, false),
            (
                "number",
                number_ann(),
                true,
                false,
                false,
                false,
                false,
                true,
            ),
        ] {
            let caps = state_with(vec![ann]).selected_capabilities();
            assert_eq!(caps.stroke_color, color, "{name} stroke_color");
            assert_eq!(caps.stroke_width, width, "{name} stroke_width");
            assert_eq!(caps.fill, fill, "{name} fill");
            assert_eq!(caps.text_style, text, "{name} text_style");
            assert_eq!(caps.effects, effects, "{name} effects");
            assert_eq!(caps.number_size, number, "{name} number_size");
        }
    }

    #[test]
    fn caps_empty_selection_is_all_false() {
        let s = blank();
        assert_eq!(s.selected_capabilities(), SelectionCapabilities::default());
    }

    #[test]
    fn caps_mixed_set_yields_intersection() {
        // rect + text: colour is the only shared section.
        let caps = state_with(vec![rect_ann(), text_ann()]).selected_capabilities();
        assert!(caps.stroke_color);
        assert!(!caps.stroke_width, "text has no stroke width");
        assert!(!caps.fill, "text has no fill");
        assert!(!caps.text_style, "rect has no text style");
        assert!(!caps.effects);
        assert!(!caps.number_size);
        // rect + blur: nothing shared at all.
        let caps = state_with(vec![rect_ann(), blur_ann()]).selected_capabilities();
        assert!(!caps.stroke_color, "blur has no stroke colour");
        assert!(!caps.effects, "rect has no effects");
    }

    // --- Colour mutator ---

    #[test]
    fn color_mutator_writes_only_supporting_members_and_syncs_defaults() {
        let mut s = state_with(vec![rect_ann(), text_ann(), blur_ann()]);
        let new = Color::rgb(1, 2, 3);
        s.set_selected_color(new);
        match &s.anns[0] {
            Rect { stroke, .. } => assert_eq!(stroke.color, new),
            other => panic!("expected Rect, got {other:?}"),
        }
        match &s.anns[1] {
            Text { color, .. } => assert_eq!(*color, new, "text colour is its text colour"),
            other => panic!("expected Text, got {other:?}"),
        }
        assert!(matches!(&s.anns[2], Blur { .. }), "blur untouched");
        assert_eq!(s.color, new, "drawing default synced");
        assert_eq!(
            s.text_color, new,
            "text default synced (a text member was written)"
        );
    }

    #[test]
    fn color_mutator_reapplies_marker_alpha() {
        let mut s = state_with(vec![marker_ann()]);
        let new = Color::rgb(9, 9, 9);
        s.set_selected_color(new);
        match &s.anns[0] {
            Marker { color, .. } => {
                assert_eq!(
                    *color,
                    new.with_alpha(MARKER_ALPHA),
                    "translucent contract kept"
                );
            }
            other => panic!("expected Marker, got {other:?}"),
        }
        assert_eq!(s.color, new, "default stores the user's opaque colour");
    }

    #[test]
    fn number_color_written_and_synced() {
        let mut s = state_with(vec![number_ann()]);
        let new = Color::rgb(4, 5, 6);
        s.set_selected_color(new);
        match &s.anns[0] {
            Number { color, .. } => assert_eq!(*color, new),
            other => panic!("expected Number, got {other:?}"),
        }
        assert_eq!(s.color, new);
    }

    // --- Stroke width mutator ---

    #[test]
    fn width_mutator_writes_strokes_and_freehand_skips_text() {
        let mut s = state_with(vec![rect_ann(), pencil_ann(), text_ann()]);
        s.set_selected_stroke_width(7.0);
        match &s.anns[0] {
            Rect { stroke, .. } => assert_eq!(stroke.width, 7.0),
            other => panic!("expected Rect, got {other:?}"),
        }
        match &s.anns[1] {
            Pencil { width, .. } => assert_eq!(*width, 7.0),
            other => panic!("expected Pencil, got {other:?}"),
        }
        match &s.anns[2] {
            Text { font_size, .. } => assert_eq!(*font_size, 16.0, "text untouched"),
            other => panic!("expected Text, got {other:?}"),
        }
        assert_eq!(s.stroke_width, 7.0, "default synced");
    }

    // --- Fill mutator ---

    #[test]
    fn fill_mutator_uses_annotation_stroke_colour_and_syncs_default() {
        let mut s = state_with(vec![rect_ann(), line_ann()]);
        s.set_selected_fill(true);
        match &s.anns[0] {
            Rect { fill, stroke, .. } => assert_eq!(*fill, Some(stroke.color)),
            other => panic!("expected Rect, got {other:?}"),
        }
        assert!(
            matches!(&s.anns[1], Line { .. }),
            "line has no fill to write"
        );
        assert!(s.fill_shapes, "default synced");
        s.set_selected_fill(false);
        match &s.anns[0] {
            Rect { fill, .. } => assert_eq!(*fill, None),
            other => panic!("expected Rect, got {other:?}"),
        }
        assert!(!s.fill_shapes);
    }

    // --- Text style mutator ---

    #[test]
    fn text_style_mutator_stamps_bundle_and_syncs_text_defaults() {
        let mut s = state_with(vec![text_ann(), rect_ann()]);
        let style = TextStyle {
            size: 24.0,
            color: Color::rgb(7, 7, 7),
            family: FontFamily::Monospace,
            weight: FontWeight::Bold,
            style: FontStyle::Italic,
            align: TextAlign::Center,
            line_spacing: 1.5,
            outline: Some(TextOutline {
                color: Color::rgb(0, 0, 0),
                width: 2.0,
            }),
            background: Some(Color::rgb(255, 255, 0)),
        };
        s.set_selected_text_style(style);
        match &s.anns[0] {
            Text {
                font_size,
                color,
                family,
                weight,
                style: font_style,
                align,
                line_spacing,
                outline,
                background,
                ..
            } => {
                assert_eq!(*font_size, 24.0);
                assert_eq!(*color, style.color);
                assert_eq!(*family, FontFamily::Monospace);
                assert_eq!(*weight, FontWeight::Bold);
                assert_eq!(*font_style, FontStyle::Italic);
                assert_eq!(*align, TextAlign::Center);
                assert_eq!(*line_spacing, 1.5);
                assert_eq!(*outline, style.outline);
                assert_eq!(*background, style.background);
            }
            other => panic!("expected Text, got {other:?}"),
        }
        assert!(matches!(&s.anns[1], Rect { .. }), "rect untouched");
        assert_eq!(s.text_color, style.color, "text defaults synced");
        assert_eq!(s.font_size, 24.0);
        assert_eq!(s.font_family, FontFamily::Monospace);
        assert_eq!(s.font_weight, FontWeight::Bold);
        assert_eq!(s.font_style, FontStyle::Italic);
        assert_eq!(s.text_align, TextAlign::Center);
        assert_eq!(s.line_spacing, 1.5);
        assert_eq!(s.text_outline, style.outline);
        assert_eq!(s.text_background, style.background);
    }

    // --- Blur mutator ---

    #[test]
    fn blur_mutator_writes_kind_and_strength_syncs_kind_default() {
        let mut s = state_with(vec![blur_ann(), rect_ann()]);
        s.set_selected_blur(BlurKind::Pixelate, 12);
        match &s.anns[0] {
            Blur { kind, strength, .. } => {
                assert_eq!(*kind, BlurKind::Pixelate);
                assert_eq!(*strength, 12);
            }
            other => panic!("expected Blur, got {other:?}"),
        }
        assert!(matches!(&s.anns[1], Rect { .. }), "rect untouched");
        assert_eq!(s.blur_kind, BlurKind::Pixelate, "default synced");
    }

    // --- Number diameter mutator ---

    #[test]
    fn number_diameter_mutator_clamps_and_syncs_default() {
        let mut s = state_with(vec![number_ann()]);
        s.set_selected_number_diameter(500.0);
        match &s.anns[0] {
            Number { diameter, .. } => assert_eq!(*diameter, NUMBER_MAX_DIAMETER, "clamped high"),
            other => panic!("expected Number, got {other:?}"),
        }
        assert_eq!(s.number_diameter, NUMBER_MAX_DIAMETER);
        s.set_selected_number_diameter(1.0);
        match &s.anns[0] {
            Number { diameter, .. } => assert_eq!(*diameter, NUMBER_MIN_DIAMETER, "clamped low"),
            other => panic!("expected Number, got {other:?}"),
        }
        assert_eq!(s.number_diameter, NUMBER_MIN_DIAMETER);
    }

    // --- Coalescing ---

    #[test]
    fn consecutive_same_kind_edits_share_one_snapshot() {
        let mut s = state_with(vec![rect_ann()]);
        s.set_selected_color(Color::rgb(1, 1, 1));
        s.set_selected_color(Color::rgb(2, 2, 2));
        s.set_selected_color(Color::rgb(3, 3, 3));
        assert_eq!(s.undo.len(), 1, "one burst = one snapshot");
        s.undo();
        match &s.anns[0] {
            Rect { stroke, .. } => {
                assert_eq!(
                    stroke.color,
                    Color::RED,
                    "undo restores the pre-burst colour"
                );
            }
            other => panic!("expected Rect, got {other:?}"),
        }
    }

    #[test]
    fn kind_change_or_end_property_edit_starts_a_new_entry() {
        let mut s = state_with(vec![rect_ann()]);
        s.set_selected_color(Color::rgb(1, 1, 1));
        s.set_selected_color(Color::rgb(2, 2, 2));
        assert_eq!(s.undo.len(), 1);
        // Kind change → new entry.
        s.set_selected_stroke_width(5.0);
        assert_eq!(s.undo.len(), 2);
        // Explicit end → the next same-kind call is a fresh entry.
        s.set_selected_stroke_width(6.0);
        assert_eq!(s.undo.len(), 2, "still the width burst");
        s.end_property_edit();
        s.set_selected_stroke_width(7.0);
        assert_eq!(s.undo.len(), 3);
    }

    #[test]
    fn other_mutations_break_the_burst() {
        let mut s = state_with(vec![rect_ann()]);
        s.set_selected_color(Color::rgb(1, 1, 1));
        // A move gesture between colour edits must not fold the
        // second burst into the first snapshot.
        s.selected = vec![0];
        let _ = s.dispatch_input(&crate::input::InputEvent::PointerDown {
            pos: Point::new(5.0, 5.0),
            modifiers: crate::input::Modifiers::default(),
        });
        let _ = s.dispatch_input(&crate::input::InputEvent::PointerDrag {
            pos: Point::new(8.0, 8.0),
        });
        let _ = s.dispatch_input(&crate::input::InputEvent::PointerUp);
        s.set_selected_color(Color::rgb(2, 2, 2));
        assert_eq!(
            s.undo.len(),
            3,
            "colour S1 + move S2 + a fresh colour entry: the move broke the burst"
        );
        // Undo / redo break it too (undo also clears the set — the
        // set is chrome — so re-select before the next edit).
        s.undo();
        s.selected = vec![0];
        s.set_selected_color(Color::rgb(3, 3, 3));
        assert_eq!(s.undo.len(), 3, "undo broke the burst");
    }

    // --- No-op + purity guards ---

    #[test]
    fn mutators_are_noops_on_empty_selection() {
        let mut s = blank();
        s.anns.push(rect_ann());
        let before = s.anns.clone();
        s.set_selected_color(Color::rgb(1, 2, 3));
        s.set_selected_stroke_width(9.0);
        s.set_selected_fill(true);
        s.set_selected_text_style(TextStyle::simple(10.0, Color::RED));
        s.set_selected_blur(BlurKind::Pixelate, 3);
        s.set_selected_number_diameter(40.0);
        assert_eq!(s.anns, before, "annotations untouched");
        assert!(s.undo.is_empty(), "no snapshots pushed");
        assert_eq!(s.color, Color::RED, "defaults untouched");
        assert_eq!(s.stroke_width, 3.0);
        assert!(!s.fill_shapes);
    }

    #[test]
    fn mutators_never_touch_pending_or_the_set() {
        let mut s = state_with(vec![rect_ann(), text_ann()]);
        let pending_before = s.pending.clone();
        let set_before = s.selected.clone();
        s.set_selected_color(Color::rgb(1, 2, 3));
        s.set_selected_stroke_width(5.0);
        s.end_property_edit();
        assert_eq!(s.pending, pending_before);
        assert_eq!(
            s.selected, set_before,
            "the selection set is chrome, not content"
        );
    }

    #[test]
    fn snapshot_carries_pre_edit_state_only() {
        let mut s = state_with(vec![rect_ann()]);
        s.set_selected_color(Color::rgb(1, 1, 1));
        match &s.undo[0].anns[0] {
            Rect { stroke, .. } => {
                assert_eq!(stroke.color, Color::RED, "snapshot is pre-first-edit");
            }
            other => panic!("expected Rect, got {other:?}"),
        }
        assert!(
            s.redo.is_empty(),
            "a property edit clears redo like any mutation"
        );
    }
}
