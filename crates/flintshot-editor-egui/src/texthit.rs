//! Position → char-index resolution for the text-selection pumps.
//!
//! Architecture decision (text-selection-highlight plan): the shell
//! owns glyph metrics, core owns selection state. Pointer positions
//! can only be mapped to char boundaries with real font measurement,
//! which lives behind [`flintshot_core::Canvas`] implementations — so
//! the pumps resolve hits shell-side and feed core's char-index APIs
//! ([`text_press_at_char`](flintshot_core::EditorState::text_press_at_char)
//! et al.) instead of core growing a measurement surface.
//!
//! [`char_at_hit`] is pure: measurement is closure-injected, so unit
//! tests run on synthetic widths with zero egui context, while the
//! pumps pass the real closure built by [`egui_measurer`] — the same
//! font family and logical↔image conversion as
//! `EguiCanvas::measure_text`, so resolved boundaries can never drift
//! from the drawn glyphs (the drift guard).

use flintshot_core::geom::Point;
use flintshot_core::text::{TextAlign, TextStyle};

/// Resolve a pointer `hit` (canvas-physical px) to the nearest char
/// boundary in `text` (char index), or `None` when the hit lies
/// outside the laid-out block.
///
/// The block starts at `text_pos` (the annotation anchor, top-left);
/// each line occupies `line_height` image px (`size * line_spacing`).
/// The y coordinate picks the line; the x coordinate binary-searches
/// prefix widths on that line to the nearest boundary, ties snapping
/// right. A hit past a line's end resolves to that line's end
/// (industry: click past EOL goes to EOL, never the next line).
/// `TextAlign` shifts each line as a block, mirroring core's
/// `caret_coords` alignment math. Empty text resolves to `Some(0)`
/// for any inside hit.
#[allow(dead_code, reason = "consumed by the pump wiring (Tasks 009/010)")]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "line index derives from a non-negative y delta over a positive line height"
)]
pub(crate) fn char_at_hit(
    text: &str,
    style: &TextStyle,
    text_pos: Point,
    hit: Point,
    line_height: f32,
    measure: impl Fn(&str) -> f32,
) -> Option<usize> {
    if hit.x < text_pos.x || hit.y < text_pos.y || line_height <= 0.0 {
        return None;
    }
    let lines: Vec<&str> = text.split('\n').collect();
    let line = ((hit.y - text_pos.y) / line_height) as usize;
    if line >= lines.len() {
        return None;
    }
    let full = lines[line];
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "line counts fit usize; the sum indexes chars"
    )]
    let line_start: usize = lines[..line].iter().map(|l| l.chars().count() + 1).sum();
    let max_w = measure(text);
    let line_w = measure(full);
    // `TextAlign` is `#[non_exhaustive]`; Left and any future
    // variants fall back to no offset until taught otherwise.
    let align = match style.align {
        TextAlign::Center => (max_w - line_w) / 2.0,
        TextAlign::Right => max_w - line_w,
        _ => 0.0,
    };
    let local_x = hit.x - text_pos.x - align;
    // Binary search the smallest boundary index whose prefix width
    // reaches `local_x`; boundaries are 0..=n (n = chars on the line).
    let n = full.chars().count();
    let prefix_w = |i: usize| {
        let prefix: String = full.chars().take(i).collect();
        measure(&prefix)
    };
    let (mut lo, mut hi) = (0_usize, n);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if prefix_w(mid) >= local_x {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    // `lo` is the first boundary at/past the hit (`n` when past the
    // end); the tie test prefers it over the previous boundary.
    let idx = if lo == 0 {
        0
    } else {
        let (prev, at) = (prefix_w(lo - 1), prefix_w(lo));
        if at - local_x <= local_x - prev {
            lo
        } else {
            lo - 1
        }
    };
    Some(line_start + idx)
}

/// Build the real measurement closure the pumps pass to
/// [`char_at_hit`] — the drift guard: the same `FAMILY_ANNOTATION`
/// font and `view.len` / `image_len` logical↔image conversion as
/// `EguiCanvas::measure_text`, so hit resolution always agrees with
/// the drawn glyphs (zoom and alignment included).
#[allow(dead_code, reason = "consumed by the pump wiring (Tasks 009/010)")]
pub(crate) fn egui_measurer(
    ctx: &egui::Context,
    view: crate::canvas::CanvasView,
    size: f32,
) -> impl Fn(&str) -> f32 + '_ {
    move |s: &str| {
        let font_id = egui::FontId::new(
            view.len(size),
            egui::FontFamily::Name(crate::theme::FAMILY_ANNOTATION.into()),
        );
        let galley = ctx.fonts_mut(|f| {
            f.layout(
                s.to_owned(),
                font_id,
                egui::Color32::PLACEHOLDER,
                f32::INFINITY,
            )
        });
        view.image_len(galley.size().x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flintshot_core::text::TextStyle;

    /// Synthetic measurement: 10 px per char, block width = widest
    /// line (mirroring how a real galley reports max line width).
    #[allow(clippy::cast_precision_loss, reason = "synthetic char counts fit f32")]
    fn synth(s: &str) -> f32 {
        s.split('\n')
            .map(|l| l.chars().count() as f32 * 10.0)
            .fold(0.0, f32::max)
    }

    fn simple() -> TextStyle {
        TextStyle::simple(16.0, flintshot_core::Color::RED)
    }

    const LH: f32 = 20.0;
    const POS: Point = Point::new(0.0, 0.0);

    fn hit(x: f32, y: f32) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn hits_snap_to_nearest_boundary_ties_right() {
        // "hello": boundaries at 0,10,20,30,40,50.
        assert_eq!(
            char_at_hit("hello", &simple(), POS, hit(5.0, 5.0), LH, synth),
            Some(1),
            "tie on char 0 snaps right"
        );
        assert_eq!(
            char_at_hit("hello", &simple(), POS, hit(4.9, 5.0), LH, synth),
            Some(0)
        );
        assert_eq!(
            char_at_hit("hello", &simple(), POS, hit(15.0, 5.0), LH, synth),
            Some(2),
            "tie between 1 and 2"
        );
        assert_eq!(
            char_at_hit("hello", &simple(), POS, hit(24.0, 5.0), LH, synth),
            Some(2)
        );
        assert_eq!(
            char_at_hit("hello", &simple(), POS, hit(26.0, 5.0), LH, synth),
            Some(3)
        );
        assert_eq!(
            char_at_hit("hello", &simple(), POS, hit(0.0, 0.0), LH, synth),
            Some(0)
        );
    }

    #[test]
    fn hits_outside_the_block_are_none() {
        assert_eq!(
            char_at_hit("hello", &simple(), POS, hit(-1.0, 5.0), LH, synth),
            None,
            "left of block"
        );
        assert_eq!(
            char_at_hit("hello", &simple(), POS, hit(5.0, -1.0), LH, synth),
            None,
            "above"
        );
        assert_eq!(
            char_at_hit("a\nb", &simple(), POS, hit(5.0, 40.0), LH, synth),
            None,
            "below the last line (2 lines = 40 px tall)"
        );
        // Just inside the last line's row still resolves.
        assert_eq!(
            char_at_hit("a\nb", &simple(), POS, hit(5.0, 39.9), LH, synth),
            Some(3)
        );
    }

    #[test]
    fn hit_past_short_line_end_lands_on_line_end() {
        // Line 0 is "ab" (20 px); a far-right hit must not wrap to
        // line 1's start.
        assert_eq!(
            char_at_hit("ab\ncccc", &simple(), POS, hit(100.0, 5.0), LH, synth),
            Some(2),
            "past EOL goes to EOL"
        );
    }

    #[test]
    fn multiline_y_picks_line_x_resolves_within() {
        // Line 1 "cdef" starts at char 3; boundaries at +0,+10,...
        assert_eq!(
            char_at_hit("ab\ncdef", &simple(), POS, hit(25.0, 25.0), LH, synth),
            Some(6),
            "tie on line 1's char 1 snaps right (3 + 2 + 1)"
        );
        assert_eq!(
            char_at_hit("ab\ncdef", &simple(), POS, hit(4.9, 25.0), LH, synth),
            Some(3),
            "just before line 1 char 0"
        );
        assert_eq!(
            char_at_hit("ab\ncdef", &simple(), POS, hit(15.0, 25.0), LH, synth),
            Some(5),
            "mid of line 1 char 0 snaps right"
        );
        assert_eq!(
            char_at_hit("ab\ncdef", &simple(), POS, hit(35.0, 5.0), LH, synth),
            Some(2),
            "line 0 end"
        );
        // Trailing newline creates an empty last line: hits there
        // resolve just past the newline.
        assert_eq!(
            char_at_hit("ab\n", &simple(), POS, hit(5.0, 25.0), LH, synth),
            Some(3)
        );
    }

    #[test]
    fn alignment_shifts_lines_as_a_block() {
        let center = TextStyle {
            align: TextAlign::Center,
            ..simple()
        };
        // Block width 40 ("cdef"); line 0 "ab" (20) centers at +10.
        assert_eq!(
            char_at_hit("ab\ncdef", &center, POS, hit(12.0, 5.0), LH, synth),
            Some(0),
            "before the centered glyphs"
        );
        assert_eq!(
            char_at_hit("ab\ncdef", &center, POS, hit(32.0, 5.0), LH, synth),
            Some(2),
            "line end"
        );
        assert_eq!(
            char_at_hit("ab\ncdef", &center, POS, hit(22.0, 5.0), LH, synth),
            Some(1),
            "tie at 20 snaps right"
        );
        // Right-aligned: line 0 offset = 40 - 20 = 20.
        let right = TextStyle {
            align: TextAlign::Right,
            ..simple()
        };
        assert_eq!(
            char_at_hit("ab\ncdef", &right, POS, hit(24.9, 5.0), LH, synth),
            Some(0),
            "before the right-aligned glyphs"
        );
        assert_eq!(
            char_at_hit("ab\ncdef", &right, POS, hit(25.0, 5.0), LH, synth),
            Some(1),
            "mid-char tie snaps right under alignment too"
        );
        assert_eq!(
            char_at_hit("ab\ncdef", &right, POS, hit(39.0, 5.0), LH, synth),
            Some(2)
        );
    }

    #[test]
    fn empty_text_resolves_zero_for_inside_hits() {
        assert_eq!(
            char_at_hit("", &simple(), POS, hit(0.0, 0.0), LH, synth),
            Some(0)
        );
        assert_eq!(
            char_at_hit("", &simple(), POS, hit(50.0, 10.0), LH, synth),
            Some(0)
        );
        assert_eq!(
            char_at_hit("", &simple(), POS, hit(-1.0, 0.0), LH, synth),
            None
        );
        assert_eq!(
            char_at_hit("", &simple(), POS, hit(0.0, 25.0), LH, synth),
            None,
            "no second line exists"
        );
    }

    #[test]
    fn nonzero_text_pos_offsets_the_block() {
        let pos = Point::new(100.0, 50.0);
        assert_eq!(
            char_at_hit("hello", &simple(), pos, hit(105.0, 55.0), LH, synth),
            Some(1)
        );
        assert_eq!(
            char_at_hit("hello", &simple(), pos, hit(99.0, 55.0), LH, synth),
            None
        );
        assert_eq!(
            char_at_hit("hello", &simple(), pos, hit(105.0, 49.0), LH, synth),
            None
        );
    }

    #[test]
    fn multibyte_chars_are_single_boundaries() {
        // "🙂b": emoji prefix width 10 (one char), same synthetic rule.
        assert_eq!(
            char_at_hit("🙂b", &simple(), POS, hit(5.0, 5.0), LH, synth),
            Some(1),
            "emoji is one boundary"
        );
        assert_eq!(
            char_at_hit("🙂b", &simple(), POS, hit(15.0, 5.0), LH, synth),
            Some(2)
        );
    }
}
