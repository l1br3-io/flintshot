//! Freehand-stroke helpers shared by `Marker` and `Pencil`.
//!
//! Both tools store raw pointer samples and share the same polyline
//! render path ([`crate::render::render_ann`]) and hit-test path
//! ([`crate::editor::Annotation::contains`]). This module owns the one
//! piece they need that the raw samples lack: smoothing, and the marker
//! alpha constant that distinguishes a translucent highlight from an
//! opaque pencil stroke.
//!
//! See ADR-0022 for the design rationale (why share, why Catmull-Rom,
//! where smoothing is applied).

use crate::geom::Point;

/// Default alpha carried by a committed `Annotation::Marker` color.
///
/// Applied once at construction in
/// [`EditorState::begin_annotation`](crate::EditorState::begin_annotation)
/// so every backend (egui, [`crate::render::ImageCanvas`],
/// [`crate::render::MockCanvas`]) renders the marker translucently with
/// no render-path special-casing — the existing polyline path just
/// receives a color whose alpha is already reduced. `0x66` ≈ 0.4
/// opacity: readable as a highlight while keeping underlying pixels
/// visible through it.
pub const MARKER_ALPHA: u8 = 0x66;

/// Subdivisions per polyline span emitted by [`smooth_polyline`].
///
/// Eight samples per span is enough to make fast pointer strokes read
/// as a continuous curve at typical screenshot resolutions without
/// inflating the rendered segment count to the point that per-frame
/// smoothing dominates. The output point count is
/// `1 + SMOOTH_SUBDIVS * (input.len() - 1)` for inputs of ≥ 2 points.
const SMOOTH_SUBDIVS: usize = 8;

/// Smooth a freehand polyline with a Catmull-Rom (cardinal) spline.
///
/// Each consecutive pair of input points becomes a span subdivided
/// into [`SMOOTH_SUBDIVS`] sub-segments. Phantom endpoints (mirrored
/// about the first/last real point) let the boundary spans smooth the
/// same way as interior ones, so even a 2- or 3-point stroke gets
/// rounded rather than degenerating to straight segments.
///
/// `tension` scales the spline tangents: `0.5` is the classic
/// Catmull-Rom (the default throughout flintshot); `0.0` flattens the
/// tangents (a smoothstep blend between control points); higher values
/// exaggerate the curve. For a collinear, evenly-spaced 2-point input
/// the result is exactly linear regardless of `tension`, which is the
/// property the determinism test pins.
///
/// Applied in [`crate::render::render_ann`] on both the pending preview
/// and the committed polyline so WYSIWYG holds between the live editor
/// and the saved/copied output (ADR-0022).
///
/// # Degenerate inputs
///
/// Fewer than 2 input points are returned unchanged — there is no span
/// to interpolate, so smoothing is a no-op.
#[must_use]
pub fn smooth_polyline(points: &[Point], tension: f32) -> Vec<Point> {
    let n = points.len();
    if n < 2 {
        return points.to_vec();
    }
    let steps = SMOOTH_SUBDIVS;
    // Output size is fixed: one point per subdivision step per span,
    // plus the final endpoint (shared span endpoints are emitted once
    // — each span after the first skips its own start sample).
    let mut out = Vec::with_capacity(1 + steps * (n - 1));
    for i in 0..n - 1 {
        let p1 = points[i];
        let p2 = points[i + 1];
        // Catmull-Rom needs one neighbour on each side. Mirror the
        // nearest real point about the span endpoint to synthesise a
        // phantom neighbour at the boundaries, so the first and last
        // spans smooth the same way as interior ones.
        let p0 = if i > 0 {
            points[i - 1]
        } else {
            Point::new(2.0 * p1.x - p2.x, 2.0 * p1.y - p2.y)
        };
        let p3 = if i + 2 < n {
            points[i + 2]
        } else {
            Point::new(2.0 * p2.x - p1.x, 2.0 * p2.y - p1.y)
        };
        // Cardinal tangents scaled by `tension`; tension = 0.5 is the
        // classic Catmull-Rom. Collinear evenly-spaced control points
        // collapse this to exact linear sampling regardless of tension.
        let m1 = Point::new((p2.x - p0.x) * tension, (p2.y - p0.y) * tension);
        let m2 = Point::new((p3.x - p1.x) * tension, (p3.y - p1.y) * tension);
        // First span emits t=0 too; later spans skip it so shared
        // endpoints aren't duplicated.
        let start = usize::from(i != 0);
        for j in start..=steps {
            #[allow(clippy::cast_precision_loss, reason = "j ≤ SMOOTH_SUBDIVS ≤ 2^24")]
            let t = j as f32 / steps as f32;
            out.push(hermite(p1, m1, p2, m2, t));
        }
    }
    out
}

/// Cubic Hermite interpolation between `p1` and `p2` with tangents
/// `m1` / `m2` at parameter `t ∈ [0, 1]`. The standard Hermite basis;
/// Catmull-Rom is this with tangents derived from the neighbour span.
fn hermite(p1: Point, m1: Point, p2: Point, m2: Point, t: f32) -> Point {
    let t2 = t * t;
    let t3 = t2 * t;
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;
    Point::new(
        h00 * p1.x + h10 * m1.x + h01 * p2.x + h11 * m2.x,
        h00 * p1.y + h10 * m1.y + h01 * p2.y + h11 * m2.y,
    )
}

#[cfg(test)]
#[allow(clippy::float_cmp, clippy::cast_possible_truncation)]
mod tests {
    use super::*;
    use crate::geom::Point;

    /// Fewer than 2 points is a no-op: the slice is returned as-is so
    /// the caller never has to special-case empty / single-sample
    /// strokes.
    #[test]
    fn smooth_polyline_passthrough_for_under_two_points() {
        assert!(smooth_polyline(&[], 0.5).is_empty(), "empty -> empty");

        let one = [Point::new(3.0, 4.0)];
        assert_eq!(smooth_polyline(&one, 0.5), vec![Point::new(3.0, 4.0)]);
    }

    /// A 2-point span with phantom neighbours is collinear and evenly
    /// spaced, so Catmull-Rom degenerates to exact linear sampling
    /// (independent of `tension`). The output is `SMOOTH_SUBDIVS + 1`
    /// evenly-spaced points from `a` to `b`.
    #[test]
    fn smooth_polyline_two_points_is_evenly_sampled_line() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(10.0, 0.0);
        let out = smooth_polyline(&[a, b], 0.5);

        let expected_len = 1 + SMOOTH_SUBDIVS;
        assert_eq!(
            out.len(),
            expected_len,
            "2-point span -> SMOOTH_SUBDIVS + 1 points"
        );
        assert_eq!(out.first(), Some(&a), "first sample is the start point");
        assert_eq!(out.last(), Some(&b), "last sample is the end point");
        // Mid-sample sits exactly at the midpoint (tension-independent
        // for evenly-spaced collinear control points).
        let mid = out[SMOOTH_SUBDIVS / 2];
        assert_eq!(mid, Point::new(5.0, 0.0), "mid sample at the midpoint");
    }

    /// Output length is `1 + SMOOTH_SUBDIVS * spans` for any ≥ 2-point
    /// input — the shared endpoints between adjacent spans are emitted
    /// once, not duplicated. Pins the per-span subdivision contract.
    #[test]
    fn smooth_polyline_length_grows_with_spans() {
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(5.0, 5.0),
            Point::new(10.0, 0.0),
            Point::new(15.0, 5.0),
        ];
        let out = smooth_polyline(&pts, 0.5);
        let spans = pts.len() - 1;
        assert_eq!(out.len(), 1 + SMOOTH_SUBDIVS * spans);
    }

    /// A purely collinear input stays collinear: the spline must not
    /// introduce curvature where none exists. This is the core
    /// "doesn't bend straight lines" guarantee.
    #[test]
    fn smooth_polyline_preserves_collinearity() {
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(20.0, 20.0),
            Point::new(30.0, 30.0),
        ];
        let out = smooth_polyline(&pts, 0.5);
        for p in &out {
            assert!(
                (p.x - p.y).abs() < 1e-4,
                "collinear input (y=x) must stay on y=x, got {p:?}"
            );
        }
    }

    /// Endpoints are preserved exactly: the spline passes through the
    /// first and last control points (Catmull-Rom is interpolating).
    #[test]
    fn smooth_polyline_preserves_endpoints() {
        let pts = [
            Point::new(1.0, 2.0),
            Point::new(5.0, 8.0),
            Point::new(9.0, 3.0),
            Point::new(12.0, 7.0),
        ];
        let out = smooth_polyline(&pts, 0.5);
        assert_eq!(out.first(), Some(&pts[0]), "first endpoint preserved");
        assert_eq!(
            out.last(),
            Some(&pts[pts.len() - 1]),
            "last endpoint preserved"
        );
    }

    /// Determinism: the same input yields byte-identical output across
    /// calls. The smoother is a pure function with no RNG / threading,
    /// so this should hold trivially — but pinning it guards the
    /// WYSIWYG contract (pending preview and committed form must smooth
    /// identically across frames).
    #[test]
    fn smooth_polyline_is_deterministic() {
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(3.0, 7.0),
            Point::new(6.0, 1.0),
            Point::new(9.0, 8.0),
            Point::new(12.0, 2.0),
        ];
        let a = smooth_polyline(&pts, 0.5);
        let b = smooth_polyline(&pts, 0.5);
        assert_eq!(a, b, "repeated calls produce identical output");
    }

    /// Smoothing subdivides: any ≥ 2-point input produces strictly more
    /// output points than input (the whole point of the smoother).
    #[test]
    fn smooth_polyline_subdivides_input() {
        let pts = [Point::new(0.0, 0.0), Point::new(1.0, 1.0)];
        let out = smooth_polyline(&pts, 0.5);
        assert!(out.len() > pts.len(), "smoothing must add samples");
    }
}
