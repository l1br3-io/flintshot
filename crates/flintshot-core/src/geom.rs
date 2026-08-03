//! Framework-free geometry primitives used throughout `flintshot`.
//!
//! These types appear in the shared data model ([`crate::EditorState`],
//! [`crate::Annotation`]). They must never re-export or wrap a GUI
//! toolkit's types (egui, iced, etc.) — that would couple core to a
//! specific shell.

use serde::{Deserialize, Serialize};

/// A 2D point in image/logical space.
///
/// Coordinates are physical pixels relative to the captured image's
/// top-left corner. `HiDPI` scaling is handled at the shell boundary.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Point {
    /// Horizontal component, increasing rightward.
    pub x: f32,
    /// Vertical component, increasing downward.
    pub y: f32,
}

impl Point {
    /// Construct a point from `(x, y)`.
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Linear interpolation toward `other` by `t` in `0.0..=1.0`.
    #[must_use]
    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
        }
    }
}

/// An axis-aligned rectangle defined by two corners.
///
/// `min` is the top-left corner; `max` is the bottom-right corner.
/// Methods assume `min.x <= max.x` and `min.y <= max.y` but do not
/// enforce it; callers building a rect from a drag should normalize.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Rect {
    /// Top-left corner.
    pub min: Point,
    /// Bottom-right corner.
    pub max: Point,
}

impl Rect {
    /// Construct a rect from two corner points (not normalized).
    #[must_use]
    pub const fn new(min: Point, max: Point) -> Self {
        Self { min, max }
    }

    /// Construct from `(x, y, w, h)`.
    #[must_use]
    pub const fn from_xywh(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            min: Point::new(x, y),
            max: Point::new(x + w, y + h),
        }
    }

    /// Width (assumes normalized).
    #[must_use]
    pub fn width(self) -> f32 {
        self.max.x - self.min.x
    }

    /// Height (assumes normalized).
    #[must_use]
    pub fn height(self) -> f32 {
        self.max.y - self.min.y
    }

    /// Return a rect with corners swapped so `min` is top-left.
    #[must_use]
    pub fn normalized(self) -> Self {
        Self {
            min: Point::new(self.min.x.min(self.max.x), self.min.y.min(self.max.y)),
            max: Point::new(self.min.x.max(self.max.x), self.min.y.max(self.max.y)),
        }
    }

    /// Intersection of two rects. Returns `None` if they don't overlap
    /// or the overlap is degenerate (zero area).
    ///
    /// Both inputs are normalized first so flipped corners don't
    /// produce wrong results. The result is always normalized.
    #[must_use]
    pub fn intersect(self, other: Self) -> Option<Self> {
        let a = self.normalized();
        let b = other.normalized();
        let min_x = a.min.x.max(b.min.x);
        let min_y = a.min.y.max(b.min.y);
        let max_x = a.max.x.min(b.max.x);
        let max_y = a.max.y.min(b.max.y);
        if max_x > min_x && max_y > min_y {
            Some(Self::new(
                Point::new(min_x, min_y),
                Point::new(max_x, max_y),
            ))
        } else {
            None
        }
    }

    /// Whether `point` lies inside this rect (boundaries inclusive).
    ///
    /// Assumes the rect is normalized; call [`normalized`](Self::normalized)
    /// first if the corners may be flipped.
    #[must_use]
    pub fn contains(self, point: Point) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }
}

/// An 8-bit sRGB color with alpha.
///
/// Serializes as a CSS-style hex string (`"#rrggbb"` when opaque,
/// `"#rrggbbaa"` otherwise). See [`crate::serde_util::color_hex`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel (`0` = transparent, `255` = opaque).
    pub a: u8,
}

impl serde::Serialize for Color {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        crate::serde_util::color_hex::serialize(self, ser)
    }
}

impl<'de> serde::Deserialize<'de> for Color {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        crate::serde_util::color_hex::deserialize(de)
    }
}

impl Color {
    /// Construct an opaque RGB color.
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Construct an RGBA color.
    #[must_use]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Return this color with the alpha channel replaced by `a`.
    ///
    /// Used by the marker tool to drop a full-opacity user color to
    /// [`MARKER_ALPHA`](crate::MARKER_ALPHA) at construction time so
    /// every backend renders it translucently without render-path
    /// special-casing (ADR-0022).
    #[must_use]
    pub const fn with_alpha(self, a: u8) -> Self {
        Self {
            r: self.r,
            g: self.g,
            b: self.b,
            a,
        }
    }

    /// Pure red convenience constant (default annotation color).
    pub const RED: Self = Self::rgb(255, 0, 0);
}

/// Stroke style for outlined shapes.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Stroke {
    /// Stroke color.
    pub color: Color,
    /// Stroke width in physical pixels.
    pub width: f32,
}

impl Stroke {
    /// Construct a stroke.
    #[must_use]
    pub const fn new(color: Color, width: f32) -> Self {
        Self { color, width }
    }
}

/// Toolkit-agnostic key representation for keyboard input.
///
/// Deliberately minimal — grows as new features require new keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Key {
    /// `Escape`.
    Escape,
    /// `Enter` / return.
    Enter,
    /// `Backspace`.
    Backspace,
    /// `Tab`.
    Tab,
    /// `←` arrow.
    ArrowLeft,
    /// `→` arrow.
    ArrowRight,
    /// `↑` arrow.
    ArrowUp,
    /// `↓` arrow.
    ArrowDown,
    /// `Home` (start of line / text).
    Home,
    /// `End` (end of line / text).
    End,
    /// Left Ctrl.
    Ctrl,
    /// Left Shift.
    Shift,
    /// Left Alt / Option.
    Alt,
    /// Left Super / Cmd / Windows.
    Super,
    /// A printable character key.
    Char(char),
    /// Fallback for keys not yet modeled. Use sparingly.
    #[default]
    Other,
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn rect_from_xywh_has_expected_corners() {
        let r = Rect::from_xywh(10.0, 20.0, 30.0, 40.0);
        assert_eq!(r.min, Point::new(10.0, 20.0));
        assert_eq!(r.max, Point::new(40.0, 60.0));
        assert_eq!(r.width(), 30.0);
        assert_eq!(r.height(), 40.0);
    }

    #[test]
    fn rect_normalized_swaps_flipped_corners() {
        let r = Rect::new(Point::new(40.0, 60.0), Point::new(10.0, 20.0)).normalized();
        assert_eq!(r.min, Point::new(10.0, 20.0));
        assert_eq!(r.max, Point::new(40.0, 60.0));
    }

    #[test]
    fn point_lerp_endpoints_and_midpoint() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(10.0, 20.0);
        assert_eq!(a.lerp(b, 0.0), a);
        assert_eq!(a.lerp(b, 1.0), b);
        assert_eq!(a.lerp(b, 0.5), Point::new(5.0, 10.0));
    }

    #[test]
    fn color_rgb_is_opaque() {
        assert_eq!(Color::rgb(1, 2, 3).a, 255);
    }

    #[test]
    fn rect_intersect_overlapping() {
        let a = Rect::from_xywh(0.0, 0.0, 10.0, 10.0);
        let b = Rect::from_xywh(5.0, 5.0, 10.0, 10.0);
        let i = a.intersect(b).expect("overlap");
        assert_eq!(i.min, Point::new(5.0, 5.0));
        assert_eq!(i.max, Point::new(10.0, 10.0));
    }

    #[test]
    fn rect_intersect_disjoint_is_none() {
        let a = Rect::from_xywh(0.0, 0.0, 5.0, 5.0);
        let b = Rect::from_xywh(10.0, 10.0, 5.0, 5.0);
        assert!(a.intersect(b).is_none());
    }

    #[test]
    fn rect_intersect_edge_touch_is_none() {
        // Sharing an edge is zero-area overlap → None.
        let a = Rect::from_xywh(0.0, 0.0, 5.0, 5.0);
        let b = Rect::from_xywh(5.0, 0.0, 5.0, 5.0);
        assert!(a.intersect(b).is_none());
    }

    #[test]
    fn rect_contains_inclusive_bounds() {
        let r = Rect::from_xywh(0.0, 0.0, 10.0, 10.0);
        assert!(r.contains(Point::new(0.0, 0.0)), "min corner inclusive");
        assert!(r.contains(Point::new(10.0, 10.0)), "max corner inclusive");
        assert!(r.contains(Point::new(5.0, 5.0)), "interior");
        assert!(!r.contains(Point::new(10.1, 5.0)), "just outside");
    }
}
