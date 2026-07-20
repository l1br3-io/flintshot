//! The annotation data model and editor state machine.
//!
//! All types use [`crate::geom`] primitives, never GUI-toolkit types.
//! Phase 0 ships the type definitions; Phase 1 implements hit-testing,
//! undo/redo, and tool-specific state transitions.

use serde::{Deserialize, Serialize};

use crate::geom::{Color, Point, Rect, Stroke};

/// Available annotation tools.
///
/// `Default` is [`Tool::Rect`] so the editor opens with a useful tool
/// active; `flintshot-app` overrides this from config.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Tool {
    /// Region select / crop.
    Select,
    /// Rectangle outline.
    #[default]
    Rect,
    /// Ellipse outline.
    Ellipse,
    /// Straight line.
    Line,
    /// Arrow with barbs.
    Arrow,
    /// Marker (semi-transparent thick stroke).
    Marker,
    /// Freehand pencil.
    Pencil,
    /// Text label.
    Text,
    /// Blur / pixelate region.
    Blur,
    /// Auto-incrementing numbered bubble.
    Number,
}

/// Distinguishes Gaussian blur from pixelate. Computed in core, emitted
/// to the [`crate::Canvas`] as an image blit so output is identical
/// across toolkits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BlurKind {
    /// Gaussian blur.
    #[default]
    Gaussian,
    /// Blocky pixelate.
    Pixelate,
}

/// One annotation. Variants mirror the [`Tool`] enum. Stored in
/// [`EditorState::anns`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Annotation {
    /// Outlined / filled rectangle.
    Rect {
        /// Geometry.
        rect: Rect,
        /// Outline style.
        stroke: Stroke,
        /// Optional fill.
        fill: Option<Color>,
    },
    /// Outlined / filled ellipse.
    Ellipse {
        /// Geometry (ellipse is inscribed in `rect`).
        rect: Rect,
        /// Outline style.
        stroke: Stroke,
        /// Optional fill.
        fill: Option<Color>,
    },
    /// Straight line.
    Line {
        /// Endpoint A.
        a: Point,
        /// Endpoint B.
        b: Point,
        /// Stroke style.
        stroke: Stroke,
    },
    /// Arrow with barbed head.
    Arrow {
        /// Tail endpoint.
        a: Point,
        /// Head endpoint.
        b: Point,
        /// Stroke style.
        stroke: Stroke,
        /// barb length in physical px.
        head_size: f32,
    },
    /// Semi-transparent thick stroke.
    Marker {
        /// Polyline points.
        points: Vec<Point>,
        /// Marker color.
        color: Color,
        /// Stroke width.
        width: f32,
    },
    /// Opaque freehand stroke.
    Pencil {
        /// Polyline points.
        points: Vec<Point>,
        /// Stroke color.
        color: Color,
        /// Stroke width.
        width: f32,
    },
    /// Left-to-right text label.
    Text {
        /// Anchor (top-left of text).
        pos: Point,
        /// Text content.
        text: String,
        /// Font size in physical px.
        font_size: f32,
        /// Text color.
        color: Color,
    },
    /// Blur / pixelate region.
    Blur {
        /// Region to obscure.
        rect: Rect,
        /// Blur variant.
        kind: BlurKind,
        /// Strength (`0..=255` for pixelate block size, or sigma for Gaussian).
        strength: u8,
    },
    /// Auto-incrementing numbered bubble.
    Number {
        /// Bubble center.
        pos: Point,
        /// Number to display.
        n: u32,
        /// Bubble color.
        color: Color,
        /// Bubble diameter.
        diameter: f32,
    },
}

/// Full editor state: source image + committed annotations + history +
/// active tool settings.
///
/// `image` is the source screenshot, never mutated. Annotations are
/// appended to `anns`; undo/redo is tracked by snapshot. The
/// toolkit-specific texture handle does *not* live here — the shell
/// owns it on its side.
#[derive(Debug, Clone)]
pub struct EditorState {
    /// Source screenshot, never mutated.
    pub image: image::RgbaImage,
    /// Committed annotations, in draw order.
    pub anns: Vec<Annotation>,
    /// Undo history (snapshots of `anns`).
    pub undo: Vec<Vec<Annotation>>,
    /// Redo history (snapshots of `anns`).
    pub redo: Vec<Vec<Annotation>>,
    /// Currently-active tool.
    pub tool: Tool,
    /// Active annotation color.
    pub color: Color,
    /// Active stroke width.
    pub stroke_width: f32,
    /// Active font size for `Tool::Text`.
    pub font_size: f32,
    /// Next number to assign for `Tool::Number`.
    pub next_number: u32,
    /// Selected annotation index, if any.
    pub selection: Option<usize>,
}

impl EditorState {
    /// Construct a fresh editor state for a captured screenshot.
    #[must_use]
    pub fn new(image: image::RgbaImage) -> Self {
        Self {
            image,
            anns: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            tool: Tool::default(),
            color: Color::RED,
            stroke_width: 3.0,
            font_size: 16.0,
            next_number: 1,
            selection: None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    fn blank_image() -> image::RgbaImage {
        image::RgbaImage::new(2, 2)
    }

    #[test]
    fn new_state_has_sensible_defaults() {
        let s = EditorState::new(blank_image());
        assert_eq!(s.image.dimensions(), (2, 2));
        assert!(s.anns.is_empty());
        assert!(s.undo.is_empty());
        assert!(s.redo.is_empty());
        assert_eq!(s.tool, Tool::Rect);
        assert_eq!(s.color, Color::RED);
        assert_eq!(s.stroke_width, 3.0);
        assert_eq!(s.font_size, 16.0);
        assert_eq!(s.next_number, 1);
        assert!(s.selection.is_none());
    }

    #[test]
    fn tool_default_is_rect() {
        assert_eq!(Tool::default(), Tool::Rect);
    }

    #[test]
    fn blurkind_default_is_gaussian() {
        assert_eq!(BlurKind::default(), BlurKind::Gaussian);
    }

    #[test]
    fn annotation_serializes_roundtrip() {
        let ann = Annotation::Arrow {
            a: Point::new(1.0, 2.0),
            b: Point::new(3.0, 4.0),
            stroke: Stroke::new(Color::RED, 2.5),
            head_size: 10.0,
        };
        let json = serde_json::to_string(&ann).expect("serialize");
        let back: Annotation = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(ann, back);
    }
}
