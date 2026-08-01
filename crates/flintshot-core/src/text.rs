//! Toolkit-agnostic text-style types for the shared data model.
//!
//! These types appear in [`crate::Annotation`] and
//! [`crate::EditorState`], so they must stay free of any GUI-toolkit
//! types (egui / `ab_glyph`) — the `no_gui_types_in_core` guard
//! enforces that boundary. Renderers translate them into their own
//! native equivalents at the shell edge.

use serde::{Deserialize, Serialize};

use crate::geom::Color;

/// General-purpose font family classification.
///
/// Renderers map this onto their native font selection. Serif is a
/// follow-up (egui bundles no serif face by default); `SansSerif` and
/// `Monospace` are supported up front.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum FontFamily {
    /// Proportional sans-serif (the default, matches prior behaviour).
    #[default]
    SansSerif,
    /// Serif typeface. Not yet wired into either renderer; falls back
    /// to `SansSerif` until a serif font is bundled/registered.
    Serif,
    /// Fixed-width monospace.
    Monospace,
}

/// Font weight. Phase 1 supports the two common cases; the variant is
/// `#[non_exhaustive]` so finer weights can join later without breaking
/// downstream matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum FontWeight {
    /// Regular weight.
    #[default]
    Normal,
    /// Bold weight.
    Bold,
}

/// Font style: upright vs italic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum FontStyle {
    /// Upright / regular.
    #[default]
    Normal,
    /// Italic / slanted.
    Italic,
}

/// Horizontal alignment of a text run within its bounding box.
///
/// Only matters once multiline text lands (a single line left-aligned
/// at the anchor is the historical default).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum TextAlign {
    /// Left edge at the anchor (the historical default).
    #[default]
    Left,
    /// Centred under the anchor.
    Center,
    /// Right edge at the anchor.
    Right,
}

/// Outline (stroke) drawn around glyphs for legibility over bright or
/// noisy screenshot regions.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct TextOutline {
    /// Outline colour.
    pub color: Color,
    /// Outline width in physical px.
    pub width: f32,
}

/// Default line-spacing multiplier: leading is `1.2 × font_size`, the
/// conventional single-line height used by the prior coarse
/// [`crate::Annotation::bounding_box`] estimate. Public so config
/// defaults and tests reference one source of truth.
pub const DEFAULT_LINE_SPACING: f32 = 1.2;

/// Serde default for the `line_spacing` field of [`crate::Annotation::Text`].
/// Kept as a function (not just the const) because `#[serde(default = …)]`
/// requires a path to a `fn() -> T`.
#[must_use]
pub fn default_line_spacing() -> f32 {
    DEFAULT_LINE_SPACING
}

/// Every property a [`Canvas`](crate::render::Canvas) needs to lay out
/// and paint a text run.
///
/// Built from an [`crate::Annotation::Text`] (via
/// [`crate::Annotation::text_style`]) or an
/// [`crate::EditorState`]'s live text-tool settings. Carrying the full
/// bundle through the render trait (rather than growing its positional
/// argument list) keeps the [`Canvas`](crate::render::Canvas) signature
/// stable as new text properties are added.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TextStyle {
    /// Font size in physical px.
    pub size: f32,
    /// Text colour.
    pub color: Color,
    /// Font family.
    pub family: FontFamily,
    /// Font weight.
    pub weight: FontWeight,
    /// Font style.
    pub style: FontStyle,
    /// Horizontal alignment.
    pub align: TextAlign,
    /// Line-spacing multiplier applied between wrapped / explicit
    /// newlines. Line height = `size * line_spacing`.
    pub line_spacing: f32,
    /// Optional glyph outline for readability.
    pub outline: Option<TextOutline>,
    /// Optional filled background drawn behind the text block.
    pub background: Option<Color>,
}

impl TextStyle {
    /// Build the minimal style used by [`crate::Annotation::Number`]'s
    /// inline label: a plain, left-aligned, sans-serif run with no
    /// outline or background. Kept here so the Number render path
    /// doesn't need to know `TextStyle`'s field set.
    #[must_use]
    pub fn simple(size: f32, color: Color) -> Self {
        Self {
            size,
            color,
            family: FontFamily::SansSerif,
            weight: FontWeight::Normal,
            style: FontStyle::Normal,
            align: TextAlign::Left,
            line_spacing: DEFAULT_LINE_SPACING,
            outline: None,
            background: None,
        }
    }
}

/// Measurements of a laid-out text run, in physical px.
///
/// Produced by [`Canvas::measure_text`](crate::render::Canvas::measure_text).
/// For a single-line run: `width == last_line_width` and
/// `height == size * line_spacing`. For a multiline run, `width` is the
/// widest line, `height` spans every line, and `last_line_width` is
/// where the caret sits at the end of the final line.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TextMetrics {
    /// Widest line's advance width.
    pub width: f32,
    /// Full block height across all lines.
    pub height: f32,
    /// Advance width of the final line (caret x-offset from the anchor).
    pub last_line_width: f32,
}
