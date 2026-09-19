//! Move-tool object chrome, shared by both shells.
//!
//! The prominent-move affordance (prominent-move-tool spec): under
//! [`Tool::Move`], the object under the pointer announces itself.
//! Industry convention (Flameshot, Snagit, Excalidraw): hover is a
//! subtle dashed outline + grab cursor; a live grab is a solid,
//! accent-colored outline that tracks the object through the drag.
//!
//! Everything here is a read-only query over [`EditorState`] — no
//! dispatcher events, no history entries, no `selected` / `moving`
//! mutation. The shells gate *when* to ask (pointer on
//! canvas, not over the side panel, no gesture in flight) and own the
//! physical → logical transform; this module decides *what* the
//! answer is and how it paints.

use flintshot_core::{Annotation, EditorState, Point, Rect, Tool};

use crate::theme;

/// Which affordance a chrome rect represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MoveChromeKind {
    /// Pointer rests on an annotation: dashed 1 px outline, the
    /// "grab me" invitation.
    Hover,
    /// A dispatcher move drag is live: solid forest outline tracking
    /// the grabbed annotation — the active-object emphasis.
    Grab,
}

/// This frame's move chrome: bounding boxes in canvas-physical px
/// plus the kind.
///
/// Priority: a live [`EditorState::moving`] drag wins — a multi-drag
/// translates the whole selection set, so **every member** renders
/// the `Grab` outline regardless of where the pointer is now.
/// Otherwise, `hover_at` — a position the shell has already gated —
/// hit-tests against `anns`; a hit yields a single-rect `Hover`
/// outline. `None` when the tool is not Move (and no drag is live),
/// the pointer misses, or the hit annotation has no extent
/// (degenerate bbox).
#[must_use]
pub(crate) fn move_chrome(
    state: &EditorState,
    hover_at: Option<Point>,
) -> Option<(Vec<Rect>, MoveChromeKind)> {
    if let Some(drag) = state.moving {
        // The set is what moves; outline every member. (`moving`
        // without a set is unreachable through the dispatcher — the
        // routing writes the set before the grab — but fall back to
        // the pressed annotation so any synthetic state still
        // outlines something.)
        let mut indices = state.selected.clone();
        if indices.is_empty() {
            indices.push(drag.index);
        }
        let rects: Vec<Rect> = indices
            .iter()
            .filter_map(|&index| state.anns.get(index))
            .filter_map(Annotation::bounding_box)
            .collect();
        if rects.is_empty() {
            return None;
        }
        return Some((rects, MoveChromeKind::Grab));
    }
    if state.tool != Tool::Move {
        return None;
    }
    let at = hover_at?;
    let ann = state.anns.get(state.hit_test(at)?)?;
    ann.bounding_box().map(|r| (vec![r], MoveChromeKind::Hover))
}

/// Paint a chrome rect (screen-logical, shell-transformed) in its
/// kind's visual: the theme's dashed hover stroke or the solid forest
/// grab stroke, padded off the annotation's own ink so the outline
/// never overlaps what it outlines.
pub(crate) fn paint_move_chrome(painter: &egui::Painter, rect: egui::Rect, kind: MoveChromeKind) {
    let rect = rect.expand(theme::MOVE_CHROME_PAD);
    match kind {
        MoveChromeKind::Hover => {
            for segment in dashed_rect_segments(rect, theme::MOVE_DASH, theme::MOVE_GAP) {
                painter.line_segment(segment, theme::MOVE_HOVER_STROKE);
            }
        }
        MoveChromeKind::Grab => {
            painter.rect_stroke(
                rect,
                egui::CornerRadius::ZERO,
                theme::MOVE_GRAB_STROKE,
                egui::StrokeKind::Middle,
            );
        }
    }
}

/// The persistent-selection chrome (annotation-delete spec): one
/// bounding box in canvas-physical px **per selected annotation**,
/// in selection-set order.
///
/// Rendered under the selection-owning tools — Move (grab-to-select,
/// one rect per member) and Text (click-to-select over committed
/// text, text-select-edit spec; the set is a singleton there, so at
/// most one rect). Empty when the tool owns no selection, the set is
/// empty, an index is stale, or the annotation's bbox is degenerate
/// (nothing to outline). Read-only like the rest of this module —
/// the shells own any deselect that follows an all-degenerate set
/// (a selection with nothing paintable must not survive to the next
/// frame).
#[must_use]
pub(crate) fn selected_chrome(state: &EditorState) -> Vec<Rect> {
    if !matches!(state.tool, Tool::Move | Tool::Text) {
        return Vec::new();
    }
    state
        .selected
        .iter()
        .filter_map(|&index| state.anns.get(index))
        .filter_map(Annotation::bounding_box)
        .collect()
}

/// The Text-tool hover affordance (text-select-edit): the topmost
/// committed Text under the pointer, announcing selectability with
/// the same dashed rhythm the Move tool's hover uses (paint with
/// [`move_chrome`](crate::chrome)'s `Hover` kind).
///
/// `None` when the tool is not Text, no pointer, the pointer is over
/// empty canvas, or the topmost annotation under it is not a Text —
/// non-Text hits are misses under the Text tool (Task 001's rule), so
/// they get no affordance. Read-only like the rest of this module.
#[must_use]
pub(crate) fn text_hover_chrome(state: &EditorState, hover_at: Option<Point>) -> Option<Rect> {
    if state.tool != Tool::Text {
        return None;
    }
    let at = hover_at?;
    state.anns.get(state.text_hit_test(at)?)?.bounding_box()
}

/// Paint the persistent-selection outline (screen-logical,
/// shell-transformed): 1 px dashed forest on the move chrome's dash /
/// pad rhythm, over the shared render output with the other chrome —
/// editor-only by construction, never in composed output.
pub(crate) fn paint_selected_chrome(painter: &egui::Painter, rect: egui::Rect) {
    let rect = rect.expand(theme::MOVE_CHROME_PAD);
    for segment in dashed_rect_segments(rect, theme::MOVE_DASH, theme::MOVE_GAP) {
        painter.line_segment(segment, theme::SELECTED_STROKE);
    }
}

/// Dash segments along an axis-aligned rect's perimeter: each edge
/// dashed independently (edges start on a dash; corners may end on a
/// gap — the standard marching-ants look).
fn dashed_rect_segments(rect: egui::Rect, dash: f32, gap: f32) -> Vec<[egui::Pos2; 2]> {
    let mut segments = Vec::new();
    let corners = [
        (rect.left_top(), rect.right_top()),
        (rect.right_top(), rect.right_bottom()),
        (rect.right_bottom(), rect.left_bottom()),
        (rect.left_bottom(), rect.left_top()),
    ];
    for (a, b) in corners {
        segments.extend(dashed_edge_segments(a, b, dash, gap));
    }
    segments
}

/// Dash segments along one axis-aligned edge from `a` to `b`: a
/// `dash`-long stroke, a `gap`-long pause, repeating; the final dash
/// clips to the edge's end. An edge shorter than one dash gets a
/// single full-length segment.
fn dashed_edge_segments(a: egui::Pos2, b: egui::Pos2, dash: f32, gap: f32) -> Vec<[egui::Pos2; 2]> {
    let length = (b - a).length();
    if length <= f32::EPSILON {
        return Vec::new();
    }
    let step = dash + gap;
    let mut segments = Vec::new();
    let mut t = 0.0_f32;
    while t < length {
        let end = (t + dash).min(length);
        segments.push([lerp_pos(a, b, t / length), lerp_pos(a, b, end / length)]);
        t += step;
    }
    segments
}

/// Linear interpolation along the segment `a` → `b` (axis-aligned in
/// practice; general for safety).
fn lerp_pos(a: egui::Pos2, b: egui::Pos2, t: f32) -> egui::Pos2 {
    egui::pos2(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flintshot_core::editor::AnnotationDrag;
    use flintshot_core::{Annotation, Point, Stroke};

    fn blank_state() -> EditorState {
        EditorState::new(image::RgbaImage::new(64, 64))
    }

    fn rect_ann(x1: f32, y1: f32, x2: f32, y2: f32) -> Annotation {
        Annotation::Rect {
            rect: flintshot_core::Rect::new(Point::new(x1, y1), Point::new(x2, y2)),
            stroke: Stroke::new(flintshot_core::Color::RED, 3.0),
            fill: None,
        }
    }

    fn state_with(tool: Tool, anns: &[Annotation]) -> EditorState {
        let mut s = blank_state();
        s.tool = tool;
        s.anns = anns.to_vec();
        s
    }

    #[test]
    fn hover_chrome_tracks_hit_annotation_bbox() {
        let s = state_with(Tool::Move, &[rect_ann(10.0, 10.0, 50.0, 40.0)]);
        let (rects, kind) = move_chrome(&s, Some(Point::new(30.0, 25.0)))
            .expect("hover inside the rect must yield chrome");
        assert_eq!(kind, MoveChromeKind::Hover);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].min, Point::new(10.0, 10.0));
        assert_eq!(rects[0].max, Point::new(50.0, 40.0));
    }

    #[test]
    fn hover_chrome_miss_returns_none() {
        let s = state_with(Tool::Move, &[rect_ann(10.0, 10.0, 50.0, 40.0)]);
        assert!(move_chrome(&s, Some(Point::new(5.0, 5.0))).is_none());
        assert!(move_chrome(&s, None).is_none(), "no pointer → no hover");
    }

    #[test]
    fn hover_chrome_requires_move_tool() {
        let s = state_with(Tool::Rect, &[rect_ann(10.0, 10.0, 50.0, 40.0)]);
        assert!(
            move_chrome(&s, Some(Point::new(30.0, 25.0))).is_none(),
            "non-Move tools get no hover chrome"
        );
    }

    #[test]
    fn hover_chrome_line_uses_distance_hit_and_spanning_bbox() {
        let line = Annotation::Line {
            a: Point::new(10.0, 20.0),
            b: Point::new(50.0, 20.0),
            stroke: Stroke::new(flintshot_core::Color::RED, 2.0),
        };
        let s = state_with(Tool::Move, &[line]);
        // On the line (within hit tolerance) → chrome; the bbox spans
        // both endpoints even though the line has zero height.
        let (rects, _) = move_chrome(&s, Some(Point::new(30.0, 21.0)))
            .expect("distance hit on the line must yield chrome");
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].min, Point::new(10.0, 20.0));
        assert_eq!(rects[0].max, Point::new(50.0, 20.0));
        // Far off the line → nothing.
        assert!(move_chrome(&s, Some(Point::new(30.0, 40.0))).is_none());
    }

    #[test]
    fn grab_chrome_wins_over_hover_and_tracks_index() {
        let anns = [
            rect_ann(10.0, 10.0, 30.0, 30.0),
            rect_ann(40.0, 40.0, 60.0, 60.0),
        ];
        let mut s = state_with(Tool::Move, &anns);
        s.selected = vec![1];
        s.moving = Some(AnnotationDrag {
            index: 1,
            last_pos: Point::new(1.0, 1.0),
            moved: true,
        });
        // Hover points at ann 0; the live drag on ann 1 must win.
        let (rects, kind) =
            move_chrome(&s, Some(Point::new(20.0, 20.0))).expect("live drag must yield chrome");
        assert_eq!(kind, MoveChromeKind::Grab);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].min, Point::new(40.0, 40.0));
        assert_eq!(rects[0].max, Point::new(60.0, 60.0));
        // And without any hover position at all (mid-drag pointer
        // state is the shell's concern).
        let (rects, _) = move_chrome(&s, None).expect("grab ignores hover position");
        assert_eq!(rects[0].min, Point::new(40.0, 40.0));
    }

    /// A multi-drag renders the grab outline on every member of the
    /// set, in selection order.
    #[test]
    fn multi_drag_grab_covers_every_member() {
        let anns = [
            rect_ann(10.0, 10.0, 30.0, 30.0),
            rect_ann(40.0, 40.0, 60.0, 60.0),
        ];
        let mut s = state_with(Tool::Move, &anns);
        s.selected = vec![0, 1];
        s.moving = Some(AnnotationDrag {
            index: 0,
            last_pos: Point::new(1.0, 1.0),
            moved: false,
        });
        let (rects, kind) = move_chrome(&s, None).expect("a live drag yields grab chrome");
        assert_eq!(kind, MoveChromeKind::Grab);
        assert_eq!(
            rects,
            vec![
                flintshot_core::Rect::new(Point::new(10.0, 10.0), Point::new(30.0, 30.0)),
                flintshot_core::Rect::new(Point::new(40.0, 40.0), Point::new(60.0, 60.0)),
            ],
            "every member outlines, in selection order"
        );
    }

    #[test]
    fn grab_chrome_survives_mid_drag_tool_switch() {
        // tool_for_key writes `state.tool` directly and can fire
        // mid-drag; the grab outline keys off `moving`, not `tool`.
        let mut s = state_with(Tool::Rect, &[rect_ann(10.0, 10.0, 30.0, 30.0)]);
        s.selected = vec![0];
        s.moving = Some(AnnotationDrag {
            index: 0,
            last_pos: Point::new(1.0, 1.0),
            moved: false,
        });
        assert!(move_chrome(&s, None).is_some());
    }

    #[test]
    fn dashed_edge_shorter_than_dash_is_one_full_segment() {
        let segs = dashed_edge_segments(egui::pos2(0.0, 0.0), egui::pos2(2.0, 0.0), 4.0, 3.0);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0][0], egui::pos2(0.0, 0.0));
        assert_eq!(segs[0][1], egui::pos2(2.0, 0.0));
    }

    #[test]
    fn dashed_edge_follows_dash_gap_rhythm() {
        // Length 10, dash 4, gap 3 → dashes [0,4] and [7,10].
        let segs = dashed_edge_segments(egui::pos2(0.0, 0.0), egui::pos2(10.0, 0.0), 4.0, 3.0);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0][0], egui::pos2(0.0, 0.0));
        assert_eq!(segs[0][1], egui::pos2(4.0, 0.0));
        assert_eq!(segs[1][0], egui::pos2(7.0, 0.0));
        assert_eq!(
            segs[1][1],
            egui::pos2(10.0, 0.0),
            "last dash clips to the edge"
        );
    }

    #[test]
    fn dashed_rect_segments_covers_all_four_edges() {
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(10.0, 10.0));
        let segs = dashed_rect_segments(rect, 10.0, 3.0);
        // Each edge fits exactly one dash → four segments, one per
        // edge, corners included.
        assert_eq!(segs.len(), 4);
        assert!(segs.contains(&[egui::pos2(0.0, 0.0), egui::pos2(10.0, 0.0)]));
        assert!(segs.contains(&[egui::pos2(10.0, 0.0), egui::pos2(10.0, 10.0)]));
        assert!(segs.contains(&[egui::pos2(10.0, 10.0), egui::pos2(0.0, 10.0)]));
        assert!(segs.contains(&[egui::pos2(0.0, 10.0), egui::pos2(0.0, 0.0)]));
    }

    #[test]
    fn dashed_edge_zero_length_yields_nothing() {
        let p = egui::pos2(5.0, 5.0);
        assert!(dashed_edge_segments(p, p, 4.0, 3.0).is_empty());
    }

    // --- Persistent-selection chrome (annotation-delete Task 003) ---

    #[test]
    fn selected_chrome_returns_one_bbox_per_member_under_move() {
        let mut s = state_with(
            Tool::Move,
            &[
                rect_ann(10.0, 10.0, 50.0, 40.0),
                rect_ann(60.0, 10.0, 90.0, 40.0),
            ],
        );
        s.selected = vec![0, 1];
        let rects = selected_chrome(&s);
        assert_eq!(rects.len(), 2, "one rect per selected annotation");
        assert_eq!(rects[0].min, Point::new(10.0, 10.0));
        assert_eq!(rects[0].max, Point::new(50.0, 40.0));
        assert_eq!(rects[1].min, Point::new(60.0, 10.0));
        assert_eq!(rects[1].max, Point::new(90.0, 40.0));
    }

    #[test]
    fn selected_chrome_requires_selection_owning_tool() {
        let mut s = state_with(Tool::Rect, &[rect_ann(10.0, 10.0, 50.0, 40.0)]);
        s.selected = vec![0];
        assert!(
            selected_chrome(&s).is_empty(),
            "non-selection-owning tools get no selection chrome"
        );
        s.tool = Tool::Pencil;
        assert!(
            selected_chrome(&s).is_empty(),
            "Pencil gets no selection chrome either"
        );
    }

    #[test]
    fn selected_chrome_returns_selected_bbox_under_text() {
        // text-select-edit: the Text tool owns a persistent selection
        // too (click-to-select over committed text); its chrome is
        // the same dashed outline Move renders.
        let mut s = state_with(
            Tool::Text,
            &[Annotation::text(
                Point::new(10.0, 10.0),
                "hello",
                flintshot_core::text::TextStyle::simple(16.0, flintshot_core::Color::RED),
            )],
        );
        s.selected = vec![0];
        let rect = selected_chrome(&s)
            .into_iter()
            .next()
            .expect("a selected Text under the Text tool must yield its bbox");
        assert_eq!(rect.min, Point::new(10.0, 10.0));
        assert_eq!(
            rect.max,
            Point::new(10.0 + 16.0 * 0.5 * 5.0, 10.0 + 16.0 * 1.2),
            "coarse bbox: half-em advance per char, one line at line-spacing height"
        );
    }

    // --- Text-tool hover affordance (text-select-edit Task 007) ---

    fn text_ann_state(tool: Tool) -> EditorState {
        let mut s = state_with(
            tool,
            &[Annotation::text(
                Point::new(10.0, 10.0),
                "hello",
                flintshot_core::text::TextStyle::simple(16.0, flintshot_core::Color::RED),
            )],
        );
        s.tool = tool;
        s
    }

    #[test]
    fn text_hover_chrome_tracks_committed_text_under_text_tool() {
        let s = text_ann_state(Tool::Text);
        let rect = text_hover_chrome(&s, Some(Point::new(20.0, 15.0)))
            .expect("hover on a committed Text must yield its bbox");
        assert_eq!(rect.min, Point::new(10.0, 10.0));
        assert_eq!(rect.max, Point::new(10.0 + 40.0, 10.0 + 16.0 * 1.2));
        assert!(
            text_hover_chrome(&s, None).is_none(),
            "no pointer → no hover"
        );
        assert!(
            text_hover_chrome(&s, Some(Point::new(60.0, 60.0))).is_none(),
            "empty canvas → no hover"
        );
    }

    #[test]
    fn text_hover_chrome_ignores_non_text_hits_and_other_tools() {
        // A non-Text annotation on top: transparent under Text.
        let mut s = text_ann_state(Tool::Text);
        s.anns.push(rect_ann(12.0, 12.0, 40.0, 24.0));
        assert!(
            text_hover_chrome(&s, Some(Point::new(20.0, 15.0))).is_none(),
            "a rect covering the text is a miss under the Text tool"
        );
        // Under Move the affordance belongs to move_chrome instead.
        let s = text_ann_state(Tool::Move);
        assert!(text_hover_chrome(&s, Some(Point::new(20.0, 15.0))).is_none());
        // Non-selection-owning tools get nothing.
        let s = text_ann_state(Tool::Rect);
        assert!(text_hover_chrome(&s, Some(Point::new(20.0, 15.0))).is_none());
    }

    #[test]
    fn selected_chrome_none_without_selection() {
        let s = state_with(Tool::Move, &[rect_ann(10.0, 10.0, 50.0, 40.0)]);
        assert!(selected_chrome(&s).is_empty(), "no selection → no chrome");
    }

    #[test]
    fn selected_chrome_skips_degenerate_or_stale_members() {
        let mut degenerate = blank_state();
        degenerate.tool = Tool::Move;
        degenerate.anns.push(Annotation::Pencil {
            points: vec![],
            color: flintshot_core::Color::RED,
            width: 2.0,
        });
        degenerate.selected = vec![0];
        assert!(
            selected_chrome(&degenerate).is_empty(),
            "an empty-points polyline has no bbox to outline"
        );
        degenerate.selected = vec![7];
        assert!(
            selected_chrome(&degenerate).is_empty(),
            "a stale out-of-range index has no annotation to outline"
        );
    }
}
