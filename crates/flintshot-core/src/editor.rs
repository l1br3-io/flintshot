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
///
/// `pending` holds the annotation currently being drawn (e.g. during a
/// drag). The shared `render()` path draws it on top of `anns` so the
/// user sees live feedback.
#[derive(Debug, Clone)]
pub struct EditorState {
    /// Source screenshot, never mutated.
    pub image: image::RgbaImage,
    /// Committed annotations, in draw order.
    pub anns: Vec<Annotation>,
    /// Undo history (snapshots of `anns` taken at each commit).
    pub undo: Vec<Vec<Annotation>>,
    /// Redo history (snapshots of `anns` taken at each undo).
    pub redo: Vec<Vec<Annotation>>,
    /// In-flight annotation being drawn (live feedback during drag).
    /// `None` when no tool interaction is in progress.
    pub pending: Option<Annotation>,
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
            pending: None,
            tool: Tool::default(),
            color: Color::RED,
            stroke_width: 3.0,
            font_size: 16.0,
            next_number: 1,
            selection: None,
        }
    }

    /// Begin a drag-based annotation at `at`.
    ///
    /// Constructs an initial `Annotation` from the active `tool` /
    /// `color` / `stroke_width`, with zero or trivial extent, and
    /// stores it in `pending`. Subsequent pointer events should call
    /// [`update_annotation`](Self::update_annotation); release should
    /// call [`commit_annotation`](Self::commit_annotation).
    ///
    /// Tools with a non-drag input model (`Text`, `Number`, `Select`)
    /// are no-ops here; they have dedicated entry points (Text gets a
    /// `begin_text` method in Task 12; Number gets its own placement
    /// flow; Select is handled by the shell directly).
    ///
    /// If a `pending` annotation already exists it is replaced — the
    /// shell should `commit_annotation()` or `cancel_annotation()`
    /// first if it wants to preserve the in-flight one.
    pub fn begin_annotation(&mut self, at: Point) {
        let stroke = Stroke::new(self.color, self.stroke_width);
        let ann = match self.tool {
            Tool::Rect => Annotation::Rect {
                rect: Rect::new(at, at),
                stroke,
                fill: None,
            },
            Tool::Ellipse => Annotation::Ellipse {
                rect: Rect::new(at, at),
                stroke,
                fill: None,
            },
            Tool::Line => Annotation::Line {
                a: at,
                b: at,
                stroke,
            },
            Tool::Arrow => Annotation::Arrow {
                a: at,
                b: at,
                stroke,
                head_size: ARROW_DEFAULT_HEAD_SIZE,
            },
            Tool::Marker => Annotation::Marker {
                points: vec![at],
                color: self.color,
                width: self.stroke_width,
            },
            Tool::Pencil => Annotation::Pencil {
                points: vec![at],
                color: self.color,
                width: self.stroke_width,
            },
            Tool::Blur => Annotation::Blur {
                rect: Rect::new(at, at),
                kind: BlurKind::default(),
                strength: BLUR_DEFAULT_STRENGTH,
            },
            // Text / Number / Select have non-drag input models; their
            // entry points are tool-specific (see method doc).
            Tool::Text | Tool::Number | Tool::Select => return,
        };
        self.pending = Some(ann);
    }

    /// Update the in-flight annotation's geometry to follow `at`.
    ///
    /// No-op when `pending` is `None`. For rect-shaped annotations
    /// (`Rect` / `Ellipse` / `Blur`) `at` becomes the corner opposite
    /// the begin anchor — storage is intentionally *not* normalized
    /// during the drag so the anchor stays fixed; [`Rect::normalized`]
    /// is applied at commit / render / hit-test time. For `Line` /
    /// `Arrow` `at` becomes the head endpoint. For `Marker` / `Pencil`
    /// `at` is appended to the polyline.
    pub fn update_annotation(&mut self, at: Point) {
        let Some(ann) = self.pending.as_mut() else {
            return;
        };
        match ann {
            Annotation::Rect { rect, .. }
            | Annotation::Ellipse { rect, .. }
            | Annotation::Blur { rect, .. } => rect.max = at,
            Annotation::Line { b, .. } | Annotation::Arrow { b, .. } => *b = at,
            Annotation::Marker { points, .. } | Annotation::Pencil { points, .. } => {
                points.push(at);
            }
            // Text / Number pending annotations are not produced by
            // `begin_annotation`; their input flow owns its own state.
            Annotation::Text { .. } | Annotation::Number { .. } => {}
        }
    }

    /// Commit the in-flight annotation to `anns`.
    ///
    /// Pushes a snapshot of the current `anns` onto `undo`, clears
    /// `redo`, normalizes the pending annotation's rect (if any), then
    /// appends it to `anns`. No-op when `pending` is `None`.
    pub fn commit_annotation(&mut self) {
        let Some(mut ann) = self.pending.take() else {
            return;
        };
        normalize_rects(&mut ann);
        // Clone-then-push: snapshot must capture pre-commit `anns`
        // (without the new annotation). `mem::take` here would wipe
        // `self.anns` and lose prior annotations.
        self.undo.push(self.anns.clone());
        self.redo.clear();
        self.anns.push(ann);
    }

    /// Drop the in-flight annotation without committing.
    pub fn cancel_annotation(&mut self) {
        self.pending = None;
    }

    /// Pop the last committed state from `undo` and swap it into `anns`.
    ///
    /// Also clears `pending` (Ctrl+Z mid-drag cancels the in-flight
    /// annotation, matching typical editor behaviour). The current
    /// `anns` is pushed onto `redo` before the swap. No-op when
    /// `undo` is empty.
    pub fn undo(&mut self) {
        self.pending = None;
        let Some(prev) = self.undo.pop() else {
            return;
        };
        self.redo.push(std::mem::take(&mut self.anns));
        self.anns = prev;
    }

    /// Pop the last undone state from `redo` and swap it into `anns`.
    ///
    /// The current `anns` is pushed onto `undo` before the swap. No-op
    /// when `redo` is empty (which is always the case immediately
    /// after a commit, since `commit_annotation` clears `redo`).
    pub fn redo(&mut self) {
        let Some(next) = self.redo.pop() else {
            return;
        };
        self.undo.push(std::mem::take(&mut self.anns));
        self.anns = next;
    }

    /// Return the index of the topmost annotation containing `point`,
    /// or `None` if no annotation is hit.
    ///
    /// "Topmost" = highest index in `anns` (drawn last). Per-variant
    /// geometry: rect-shaped annotations use point-in-rect; `Line` /
    /// `Arrow` use distance-to-segment within `stroke.width / 2 +
    /// HIT_TEST_TOLERANCE_PX`; `Marker` / `Pencil` test each segment;
    /// `Number` uses circle distance; `Text` uses a coarse bounding-box
    /// approximation (real shaping lands in Task 12).
    #[must_use]
    pub fn hit_test(&self, point: Point) -> Option<usize> {
        self.anns
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, ann)| ann.contains(point).then_some(i))
    }

    /// Dispatch a toolkit-agnostic [`InputEvent`] and return the
    /// side-effect the shell should perform.
    ///
    /// This is the primary input boundary: shells translate their
    /// native events into [`InputEvent`]s at the toolkit edge (egui →
    /// `InputEvent`, future iced / slint shells likewise) and feed
    /// them through this method. The dispatcher handles every
    /// keyboard shortcut and every pointer-driven tool transition in
    /// a single pure function — no `egui::Context` or window-system
    /// types reach this code.
    ///
    /// The shell retains responsibility for:
    /// - filtering repeat events (key held, pointer moved without a
    ///   button change),
    /// - translating screen-logical to canvas-physical coordinates,
    /// - actually performing the side-effect when the outcome is
    ///   `Cancel` / `Save` / `Copy` (those need a window / file
    ///   dialog / clipboard, which core can't touch).
    ///
    /// # Pointer-up semantics
    ///
    /// [`InputEvent::PointerUp`] carries no position. The dispatcher
    /// commits with the in-flight annotation's last-drag geometry,
    /// matching the contract the user sees during the drag — what
    /// was rendered is what gets committed. If the shell wants to
    /// capture the release position precisely, it can emit a final
    /// `PointerDrag { pos: release_pos }` before the `PointerUp`.
    pub fn dispatch_input(
        &mut self,
        event: &crate::input::InputEvent,
    ) -> crate::input::DispatchOutcome {
        use crate::input::{DispatchOutcome, InputEvent};
        match event {
            InputEvent::Key { key, modifiers } => self.dispatch_key(*key, *modifiers),
            InputEvent::PointerDown { pos } => {
                self.begin_annotation(*pos);
                DispatchOutcome::Continue
            }
            InputEvent::PointerDrag { pos } => {
                self.update_annotation(*pos);
                DispatchOutcome::Continue
            }
            InputEvent::PointerUp => {
                self.finish_pointer_up();
                DispatchOutcome::Continue
            }
        }
    }

    /// Keyboard dispatch helper. See [`dispatch_input`](Self::dispatch_input).
    fn dispatch_key(
        &mut self,
        key: crate::geom::Key,
        modifiers: crate::input::Modifiers,
    ) -> crate::input::DispatchOutcome {
        use crate::geom::Key;
        use crate::input::DispatchOutcome;

        // macOS users typically press Cmd where Linux / Windows users
        // press Ctrl. The shell is responsible for mapping its native
        // "primary chord modifier" onto `Modifiers::ctrl` so this
        // dispatcher stays platform-agnostic.
        match key {
            Key::Escape => DispatchOutcome::Cancel,
            Key::Char('z' | 'Z') if modifiers.ctrl_only() => {
                self.undo();
                DispatchOutcome::Continue
            }
            Key::Char('z' | 'Z') if modifiers.ctrl_shift() => {
                self.redo();
                DispatchOutcome::Continue
            }
            Key::Char('y' | 'Y') if modifiers.ctrl_only() => {
                self.redo();
                DispatchOutcome::Continue
            }
            Key::Char('s' | 'S') if modifiers.ctrl_only() => DispatchOutcome::Save,
            Key::Char('c' | 'C') if modifiers.ctrl_only() => DispatchOutcome::Copy,
            _ => DispatchOutcome::Continue,
        }
    }

    /// Pointer-up dispatch helper: filter degenerate rects, then
    /// commit. See [`dispatch_input`](Self::dispatch_input).
    ///
    /// Matches the Task 8 close-out's "deviation 3" zero-area filter
    /// (a click without a drag cancels rather than committing a
    /// 0×0 rect). Now in core so all shells inherit the same
    /// behaviour without duplicating the predicate.
    fn finish_pointer_up(&mut self) {
        if let Some(
            Annotation::Rect { rect, .. }
            | Annotation::Ellipse { rect, .. }
            | Annotation::Blur { rect, .. },
        ) = &self.pending
        {
            let n = rect.normalized();
            if n.width().abs() < 1.0 || n.height().abs() < 1.0 {
                self.cancel_annotation();
                return;
            }
        }
        self.commit_annotation();
    }
}

/// Default arrow-head barb length in physical pixels.
const ARROW_DEFAULT_HEAD_SIZE: f32 = 10.0;

/// Default blur strength (sigma for Gaussian, block size for Pixelate).
const BLUR_DEFAULT_STRENGTH: u8 = 8;

/// Hit-test tolerance in physical pixels, added to half the stroke
/// width so thin lines are still grabbable.
const HIT_TEST_TOLERANCE_PX: f32 = 3.0;

/// Normalize any `Rect` fields in `ann` so `min` is top-left.
///
/// Called at commit time so committed storage always has normalized
/// rects; pending annotations may have un-normalized rects during the
/// drag (the anchor stays at `rect.min` until release).
fn normalize_rects(ann: &mut Annotation) {
    match ann {
        Annotation::Rect { rect, .. }
        | Annotation::Ellipse { rect, .. }
        | Annotation::Blur { rect, .. } => *rect = rect.normalized(),
        _ => {}
    }
}

impl Annotation {
    /// Axis-aligned bounding box of the annotation, normalized so
    /// `min` is top-left. Returns `None` for annotations with no
    /// extent (empty polyline, zero-size rect).
    ///
    /// Used by hit-testing and by future select-tool bounding-box
    /// rendering. Per variant:
    /// - Rect / Ellipse / Blur: the rect itself (normalized).
    /// - Line / Arrow: the rect spanning both endpoints.
    /// - Marker / Pencil: the rect spanning all polyline points.
    /// - Text: a coarse approximation (`font_size` × estimated
    ///   advance width); real metrics land with Task 12.
    /// - Number: a square centred on `pos` with side `diameter`.
    #[must_use]
    pub fn bounding_box(&self) -> Option<Rect> {
        let rect = match self {
            Annotation::Rect { rect, .. }
            | Annotation::Ellipse { rect, .. }
            | Annotation::Blur { rect, .. } => *rect,
            Annotation::Line { a, b, .. } | Annotation::Arrow { a, b, .. } => Rect::new(*a, *b),
            Annotation::Marker { points, .. } | Annotation::Pencil { points, .. } => {
                if points.is_empty() {
                    return None;
                }
                let mut min = points[0];
                let mut max = points[0];
                for &p in &points[1..] {
                    min.x = min.x.min(p.x);
                    min.y = min.y.min(p.y);
                    max.x = max.x.max(p.x);
                    max.y = max.y.max(p.y);
                }
                Rect::new(min, max)
            }
            Annotation::Text {
                pos,
                text,
                font_size,
                ..
            } => {
                // Coarse approximation: ~half the font size per glyph
                // of advance, line height ≈ 1.2 × font_size. Real
                // metrics come with cosmic-text in Task 12.
                // char_count fits in u32 losslessly for any realistic
                // text label (< 2^24 chars); the cast is exact.
                #[allow(
                    clippy::cast_precision_loss,
                    reason = "text length fits in f32's 24-bit mantissa"
                )]
                let char_count = text.chars().count() as f32;
                let approx_w = font_size * 0.5 * char_count;
                Rect::from_xywh(pos.x, pos.y, approx_w, font_size * 1.2)
            }
            Annotation::Number { pos, diameter, .. } => {
                let half = diameter / 2.0;
                Rect::from_xywh(pos.x - half, pos.y - half, *diameter, *diameter)
            }
        };
        Some(rect.normalized())
    }

    /// Whether `point` lies on or inside this annotation.
    ///
    /// Used by [`EditorState::hit_test`]. Geometry per variant — see
    /// [`bounding_box`](Self::bounding_box) for the coarse bbox;
    /// fine-grained tests below use stroke-width-aware tolerances so
    /// thin lines remain selectable.
    #[must_use]
    pub fn contains(&self, point: Point) -> bool {
        match self {
            Annotation::Rect { rect, .. }
            | Annotation::Blur { rect, .. }
            | Annotation::Ellipse { rect, .. } => {
                let n = rect.normalized();
                point_in_rect(point, n)
            }
            Annotation::Line { a, b, stroke } | Annotation::Arrow { a, b, stroke, .. } => {
                distance_to_segment(point, *a, *b) <= stroke.width / 2.0 + HIT_TEST_TOLERANCE_PX
            }
            Annotation::Marker { points, width, .. } | Annotation::Pencil { points, width, .. } => {
                let tol = width / 2.0 + HIT_TEST_TOLERANCE_PX;
                points
                    .windows(2)
                    .any(|w| distance_to_segment(point, w[0], w[1]) <= tol)
            }
            Annotation::Text { .. } => self
                .bounding_box()
                .is_some_and(|bbox| point_in_rect(point, bbox)),
            Annotation::Number { pos, diameter, .. } => {
                let r = *diameter / 2.0 + HIT_TEST_TOLERANCE_PX;
                let dx = point.x - pos.x;
                let dy = point.y - pos.y;
                (dx * dx + dy * dy) <= r * r
            }
        }
    }
}

/// Per-pixel distance from `point` to the segment `a -> b`.
fn distance_to_segment(point: Point, a: Point, b: Point) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len_sq = dx * dx + dy * dy;
    let t = if len_sq == 0.0 {
        0.0
    } else {
        (((point.x - a.x) * dx + (point.y - a.y) * dy) / len_sq).clamp(0.0, 1.0)
    };
    let proj = Point::new(a.x + t * dx, a.y + t * dy);
    let ex = point.x - proj.x;
    let ey = point.y - proj.y;
    (ex * ex + ey * ey).sqrt()
}

/// Whether `point` lies inside `rect` (boundaries inclusive).
fn point_in_rect(point: Point, rect: Rect) -> bool {
    point.x >= rect.min.x && point.x <= rect.max.x && point.y >= rect.min.y && point.y <= rect.max.y
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::geom::Key;
    use crate::input::{DispatchOutcome, InputEvent, Modifiers};

    fn blank_image() -> image::RgbaImage {
        image::RgbaImage::new(2, 2)
    }

    fn rect_ann(x1: f32, y1: f32, x2: f32, y2: f32) -> Annotation {
        Annotation::Rect {
            rect: Rect::new(Point::new(x1, y1), Point::new(x2, y2)),
            stroke: Stroke::new(Color::RED, 3.0),
            fill: None,
        }
    }

    #[test]
    fn new_state_has_sensible_defaults() {
        let s = EditorState::new(blank_image());
        assert_eq!(s.image.dimensions(), (2, 2));
        assert!(s.anns.is_empty());
        assert!(s.undo.is_empty());
        assert!(s.redo.is_empty());
        assert!(s.pending.is_none());
        assert_eq!(s.tool, Tool::Rect);
        assert_eq!(s.color, Color::RED);
        assert_eq!(s.stroke_width, 3.0);
        assert_eq!(s.font_size, 16.0);
        assert_eq!(s.next_number, 1);
        assert!(s.selection.is_none());
    }

    #[test]
    fn pending_is_none_in_new_state() {
        let s = EditorState::new(blank_image());
        assert!(s.pending.is_none());
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

    /// `begin_annotation` then `update_annotation` should grow the
    /// pending annotation's geometry without committing.
    #[test]
    fn begin_then_update_extends_pending() {
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Rect;
        s.begin_annotation(Point::new(10.0, 10.0));
        assert!(s.pending.is_some(), "begin produces a pending ann");
        s.update_annotation(Point::new(20.0, 30.0));
        match &s.pending {
            Some(Annotation::Rect { rect, .. }) => {
                assert_eq!(rect.min, Point::new(10.0, 10.0), "anchor preserved");
                assert_eq!(rect.max, Point::new(20.0, 30.0), "max follows pointer");
            }
            other => panic!("expected pending Rect, got {other:?}"),
        }
        assert!(s.anns.is_empty(), "update does not commit");
    }

    /// Drag up-left from the anchor: anchor stays at `min`, current
    /// pointer at `max`. Normalization happens at commit, not during
    /// the drag.
    #[test]
    fn update_preserves_anchor_across_drag_direction_changes() {
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Rect;
        s.begin_annotation(Point::new(10.0, 10.0));
        s.update_annotation(Point::new(5.0, 20.0));
        s.update_annotation(Point::new(15.0, 25.0));
        match &s.pending {
            Some(Annotation::Rect { rect, .. }) => {
                assert_eq!(rect.min, Point::new(10.0, 10.0));
                assert_eq!(rect.max, Point::new(15.0, 25.0));
            }
            other => panic!("unexpected pending {other:?}"),
        }
    }

    #[test]
    fn commit_normalizes_rect() {
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Rect;
        s.begin_annotation(Point::new(40.0, 40.0));
        s.update_annotation(Point::new(10.0, 10.0));
        s.commit_annotation();
        assert!(s.pending.is_none(), "commit takes pending");
        assert_eq!(s.anns.len(), 1);
        match &s.anns[0] {
            Annotation::Rect { rect, .. } => {
                assert_eq!(rect.min, Point::new(10.0, 10.0));
                assert_eq!(rect.max, Point::new(40.0, 40.0));
            }
            other => panic!("unexpected committed {other:?}"),
        }
    }

    /// TODO Task 7 acceptance: push 3 anns, undo twice, redo once,
    /// verify exact `anns` contents at each step.
    #[test]
    fn undo_redo_snapshot_roundtrip() {
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Rect;

        // Fixtures identified by an index so each call produces a
        // fresh clone — `Annotation` is not `Copy`.
        let ann = |n: f32| rect_ann(0.0, 0.0, n, n);
        for n in [1.0_f32, 2.0, 3.0] {
            s.pending = Some(ann(n));
            s.commit_annotation();
        }
        let (a1, a2, a3) = (ann(1.0), ann(2.0), ann(3.0));
        assert_eq!(s.anns.len(), 3);
        assert_eq!(s.undo.len(), 3);

        // Undo once: anns = [a1, a2], redo = [[a1,a2,a3]]
        s.undo();
        assert_eq!(s.anns, vec![a1.clone(), a2.clone()]);
        assert_eq!(s.redo.len(), 1);
        assert_eq!(s.redo[0], vec![a1.clone(), a2.clone(), a3.clone()]);

        // Undo twice: anns = [a1], redo = [[a1,a2,a3], [a1,a2]]
        s.undo();
        assert_eq!(s.anns, vec![a1.clone()]);
        assert_eq!(s.redo.len(), 2);
        assert_eq!(s.redo[1], vec![a1.clone(), a2.clone()]);

        // Redo once: anns = [a1, a2], redo back to length 1
        s.redo();
        assert_eq!(s.anns, vec![a1, a2]);
        assert_eq!(s.redo.len(), 1);
        assert_eq!(s.undo.len(), 2);
    }

    /// TODO Task 7 acceptance: committing after an undo wipes redo.
    #[test]
    fn commit_clears_redo() {
        let mut s = EditorState::new(blank_image());
        s.tool = Tool::Rect;
        for n in [1.0_f32, 2.0] {
            s.pending = Some(rect_ann(0.0, 0.0, n, n));
            s.commit_annotation();
        }
        s.undo();
        assert!(!s.redo.is_empty(), "undo populated redo");
        // Fresh commit should wipe redo.
        s.pending = Some(rect_ann(0.0, 0.0, 5.0, 5.0));
        s.commit_annotation();
        assert!(s.redo.is_empty(), "commit clears redo");
        assert_eq!(s.anns.len(), 2);
    }

    /// TODO Task 7 acceptance: pending is None after cancel.
    #[test]
    fn cancel_drops_pending() {
        let mut s = EditorState::new(blank_image());
        s.begin_annotation(Point::new(1.0, 1.0));
        assert!(s.pending.is_some());
        s.cancel_annotation();
        assert!(s.pending.is_none());
        assert!(s.anns.is_empty(), "cancel does not commit");
    }

    /// TODO Task 7 acceptance: undo with empty history is a no-op.
    #[test]
    fn undo_with_empty_history_is_noop() {
        let mut s = EditorState::new(blank_image());
        s.anns = vec![rect_ann(0.0, 0.0, 1.0, 1.0)];
        s.undo();
        assert!(s.undo.is_empty());
        assert!(s.redo.is_empty());
        assert_eq!(s.anns.len(), 1, "anns unchanged");
    }

    /// TODO Task 7 acceptance: redo after a new commit is a no-op
    /// (because commit clears redo).
    #[test]
    fn redo_after_new_commit_is_noop() {
        let mut s = EditorState::new(blank_image());
        s.pending = Some(rect_ann(0.0, 0.0, 1.0, 1.0));
        s.commit_annotation();
        s.undo();
        s.pending = Some(rect_ann(0.0, 0.0, 2.0, 2.0));
        s.commit_annotation();
        s.redo();
        assert!(s.redo.is_empty());
        assert_eq!(s.anns.len(), 1);
    }

    /// Per the user decision: `undo()` mid-drag clears `pending`.
    #[test]
    fn undo_clears_pending() {
        let mut s = EditorState::new(blank_image());
        s.pending = Some(rect_ann(0.0, 0.0, 1.0, 1.0));
        s.anns = vec![rect_ann(0.0, 0.0, 5.0, 5.0)];
        s.undo();
        assert!(s.pending.is_none(), "undo clears pending");
    }

    /// `hit_test` returns the topmost annotation (highest index)
    /// whose geometry contains the point.
    #[test]
    fn hit_test_returns_topmost() {
        let mut s = EditorState::new(blank_image());
        // Two overlapping rects: the later one is "topmost".
        s.anns = vec![rect_ann(0.0, 0.0, 10.0, 10.0), rect_ann(0.0, 0.0, 5.0, 5.0)];
        assert_eq!(s.hit_test(Point::new(2.0, 2.0)), Some(1));
        // Only the outer rect covers this corner.
        assert_eq!(s.hit_test(Point::new(8.0, 8.0)), Some(0));
    }

    #[test]
    fn hit_test_returns_none_when_empty() {
        let s = EditorState::new(blank_image());
        assert_eq!(s.hit_test(Point::new(0.0, 0.0)), None);
    }

    #[test]
    fn hit_test_returns_none_when_miss() {
        let mut s = EditorState::new(blank_image());
        s.anns = vec![rect_ann(0.0, 0.0, 5.0, 5.0)];
        assert_eq!(s.hit_test(Point::new(50.0, 50.0)), None);
    }

    /// `bounding_box` returns the rect spanned by the annotation.
    /// For drag-from-bottom-right the stored rect is un-normalized
    /// during drag; bbox normalizes.
    #[test]
    fn bounding_box_normalizes_unnormalized_rect() {
        let ann = rect_ann(40.0, 40.0, 10.0, 10.0);
        let bbox = ann.bounding_box().expect("some bbox");
        assert_eq!(bbox.min, Point::new(10.0, 10.0));
        assert_eq!(bbox.max, Point::new(40.0, 40.0));
    }

    /// Line hit-test uses stroke-width-aware tolerance.
    #[test]
    fn hit_test_line_uses_stroke_tolerance() {
        let ann = Annotation::Line {
            a: Point::new(0.0, 0.0),
            b: Point::new(100.0, 0.0),
            stroke: Stroke::new(Color::RED, 4.0),
        };
        // 2 px above the line — within stroke half-width + tolerance.
        assert!(ann.contains(Point::new(50.0, -3.0)));
        // Far away.
        assert!(!ann.contains(Point::new(50.0, -50.0)));
    }

    /// `begin_annotation` for `Tool::Text` / `Number` / `Select` is a no-op
    /// (different input model — those tools have dedicated entry points).
    #[test]
    fn begin_annotation_skips_non_drag_tools() {
        let mut s = EditorState::new(blank_image());
        for tool in [Tool::Text, Tool::Number, Tool::Select] {
            s.tool = tool;
            s.begin_annotation(Point::new(1.0, 1.0));
            assert!(s.pending.is_none(), "{tool:?} should be a no-op");
        }
    }

    // --- Task 10: dispatch_input tests ---
    //
    // The shell's translation layer is too thin to unit-test
    // meaningfully without a synthetic `egui::Context`; the
    // dispatcher is where every keyboard + pointer contract lives.
    // These tests are the "synthetic Key events through the shell's
    // dispatcher" verification the TODO spec asks for, just running
    // directly against `EditorState::dispatch_input` rather than a
    // shell wrapper.

    fn ctrl(ch: char) -> InputEvent {
        InputEvent::Key {
            key: Key::Char(ch),
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        }
    }

    fn ctrl_shift(ch: char) -> InputEvent {
        InputEvent::Key {
            key: Key::Char(ch),
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::default()
            },
        }
    }

    fn key(key: Key) -> InputEvent {
        InputEvent::Key {
            key,
            modifiers: Modifiers::default(),
        }
    }

    #[test]
    fn dispatch_esc_returns_cancel() {
        let mut s = EditorState::new(blank_image());
        assert_eq!(s.dispatch_input(&key(Key::Escape)), DispatchOutcome::Cancel);
        // Cancel is purely a side-effect request — state itself is
        // untouched. The shell is responsible for any state rollback
        // it wants before closing the window.
        assert_eq!(s.anns.len(), 0);
        assert!(s.pending.is_none());
    }

    #[test]
    fn dispatch_ctrl_z_undoes_last_commit() {
        let mut s = EditorState::new(blank_image());
        s.anns.push(rect_ann(0.0, 0.0, 10.0, 10.0));
        s.anns.push(rect_ann(20.0, 20.0, 30.0, 30.0));
        // Take a snapshot manually so undo has something to pop (the
        // state machine's `commit_annotation` normally does this; we
        // bypass it here to isolate dispatcher behaviour).
        s.undo.push(vec![rect_ann(0.0, 0.0, 10.0, 10.0)]);

        assert_eq!(s.dispatch_input(&ctrl('z')), DispatchOutcome::Continue);
        assert_eq!(s.anns.len(), 1);
        assert_eq!(s.anns[0], rect_ann(0.0, 0.0, 10.0, 10.0));
    }

    #[test]
    fn dispatch_ctrl_z_clears_pending() {
        // Ctrl+Z mid-drag cancels the in-flight annotation — matches
        // the `undo` doc comment (Task 7 deviation 2).
        let mut s = EditorState::new(blank_image());
        s.pending = Some(rect_ann(0.0, 0.0, 5.0, 5.0));
        assert_eq!(s.dispatch_input(&ctrl('z')), DispatchOutcome::Continue);
        assert!(s.pending.is_none());
    }

    #[test]
    fn dispatch_ctrl_y_redoes() {
        let mut s = EditorState::new(blank_image());
        s.anns.push(rect_ann(0.0, 0.0, 10.0, 10.0));
        s.undo.push(Vec::new());
        s.redo.push(vec![rect_ann(0.0, 0.0, 10.0, 10.0)]);
        s.anns.clear();

        assert_eq!(s.dispatch_input(&ctrl('y')), DispatchOutcome::Continue);
        assert_eq!(s.anns.len(), 1);
        assert_eq!(s.anns[0], rect_ann(0.0, 0.0, 10.0, 10.0));
    }

    #[test]
    fn dispatch_ctrl_shift_z_redoes() {
        let mut s = EditorState::new(blank_image());
        s.anns.push(rect_ann(0.0, 0.0, 10.0, 10.0));
        s.undo.push(Vec::new());
        s.redo.push(vec![rect_ann(0.0, 0.0, 10.0, 10.0)]);
        s.anns.clear();

        assert_eq!(
            s.dispatch_input(&ctrl_shift('z')),
            DispatchOutcome::Continue
        );
        assert_eq!(s.anns.len(), 1);
    }

    #[test]
    fn dispatch_ctrl_s_returns_save() {
        let mut s = EditorState::new(blank_image());
        assert_eq!(s.dispatch_input(&ctrl('s')), DispatchOutcome::Save);
        // The dispatcher does not touch state for Save — the shell
        // performs the save pipeline after seeing the outcome.
        assert_eq!(s.anns.len(), 0);
    }

    #[test]
    fn dispatch_ctrl_c_returns_copy() {
        let mut s = EditorState::new(blank_image());
        assert_eq!(s.dispatch_input(&ctrl('c')), DispatchOutcome::Copy);
    }

    #[test]
    fn dispatch_unknown_key_continues() {
        let mut s = EditorState::new(blank_image());
        assert_eq!(
            s.dispatch_input(&key(Key::Char('q'))),
            DispatchOutcome::Continue
        );
        assert_eq!(s.dispatch_input(&key(Key::Tab)), DispatchOutcome::Continue);
        assert_eq!(
            s.dispatch_input(&key(Key::Other)),
            DispatchOutcome::Continue
        );
    }

    #[test]
    fn dispatch_ignores_shortcut_when_modifiers_dont_match() {
        let mut s = EditorState::new(blank_image());
        // Shift+S alone (no Ctrl) — not a shortcut.
        assert_eq!(
            s.dispatch_input(&InputEvent::Key {
                key: Key::Char('s'),
                modifiers: Modifiers {
                    shift: true,
                    ..Modifiers::default()
                }
            }),
            DispatchOutcome::Continue
        );
        // Ctrl+Alt+S — Alt spoils the chord.
        assert_eq!(
            s.dispatch_input(&InputEvent::Key {
                key: Key::Char('s'),
                modifiers: Modifiers {
                    ctrl: true,
                    alt: true,
                    ..Modifiers::default()
                }
            }),
            DispatchOutcome::Continue
        );
        // Plain S with no modifiers — not a shortcut.
        assert_eq!(
            s.dispatch_input(&key(Key::Char('s'))),
            DispatchOutcome::Continue
        );
    }

    #[test]
    fn dispatch_pointer_down_begins_annotation() {
        let mut s = EditorState::new(blank_image());
        assert_eq!(
            s.dispatch_input(&InputEvent::PointerDown {
                pos: Point::new(5.0, 5.0)
            }),
            DispatchOutcome::Continue
        );
        assert!(s.pending.is_some());
    }

    #[test]
    fn dispatch_pointer_down_drag_up_commits_rect() {
        let mut s = EditorState::new(blank_image());
        s.dispatch_input(&InputEvent::PointerDown {
            pos: Point::new(10.0, 10.0),
        });
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(50.0, 60.0),
        });
        assert_eq!(
            s.dispatch_input(&InputEvent::PointerUp),
            DispatchOutcome::Continue
        );
        assert!(s.pending.is_none(), "pending cleared on commit");
        assert_eq!(s.anns.len(), 1);
        // The committed rect should be normalized to (10,10)-(50,60).
        if let Annotation::Rect { rect, .. } = &s.anns[0] {
            assert_eq!(rect.min, Point::new(10.0, 10.0));
            assert_eq!(rect.max, Point::new(50.0, 60.0));
        } else {
            panic!("expected Rect, got {:?}", s.anns[0]);
        }
    }

    #[test]
    fn dispatch_pointer_down_immediate_up_cancels_zero_area() {
        // Task 8 deviation 3 contract: a click without a drag does not
        // commit a 0×0 rect. The zero-area filter moved into the
        // dispatcher in Task 10 so all shells inherit it.
        let mut s = EditorState::new(blank_image());
        s.dispatch_input(&InputEvent::PointerDown {
            pos: Point::new(10.0, 10.0),
        });
        s.dispatch_input(&InputEvent::PointerUp);
        assert!(s.pending.is_none(), "pending cancelled, not committed");
        assert_eq!(s.anns.len(), 0, "no annotation committed");
    }

    #[test]
    fn dispatch_pointer_drag_without_down_is_noop() {
        // Stray drag event (e.g. the shell forwards one before the
        // first PointerDown due to a focus change) doesn't crash and
        // doesn't conjure an annotation.
        let mut s = EditorState::new(blank_image());
        s.dispatch_input(&InputEvent::PointerDrag {
            pos: Point::new(5.0, 5.0),
        });
        assert!(s.pending.is_none());
        assert_eq!(s.anns.len(), 0);
    }
}
