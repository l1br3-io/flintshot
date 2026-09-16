//! Spring easing — the pop-up book motion signature.
//!
//! The spec's microinteractions ease with CSS
//! `cubic-bezier(0.34, 1.56, 0.64, 1)`: a back-out curve that
//! overshoots past 1.0 and settles. egui's built-in animation
//! helper (`Context::animate_value_with_time`) only does an
//! exponential ease-out toward the target — it asymptotically
//! approaches and can never express overshoot, so the spring lives
//! here as pure math. Callers own the raw 0→1 time progress
//! (see [`approach`]) and pass it through [`spring`].

/// Spring control points: cubic-bezier(0.34, 1.56, 0.64, 1).
const X1: f32 = 0.34;
const Y1: f32 = 1.56;
const X2: f32 = 0.64;
const Y2: f32 = 1.0;

/// Bézier x(s): the time axis as a function of the curve
/// parameter.
fn bez_x(s: f32) -> f32 {
    3.0 * (1.0 - s) * (1.0 - s) * s * X1 + 3.0 * (1.0 - s) * s * s * X2 + s * s * s
}

/// Bézier y(s): the eased value as a function of the curve
/// parameter.
fn bez_y(s: f32) -> f32 {
    3.0 * (1.0 - s) * (1.0 - s) * s * Y1 + 3.0 * (1.0 - s) * s * s * Y2 + s * s * s
}

/// Derivative of the time axis — the Newton-Raphson denominator.
fn bez_x_derivative(s: f32) -> f32 {
    3.0 * (1.0 - s) * (1.0 - s) * X1 + 6.0 * (1.0 - s) * s * (X2 - X1) + 3.0 * s * s * (1.0 - X2)
}

/// Invert `bez_x`: find the curve parameter whose time equals `t`.
/// Newton-Raphson first (the time axis is monotone for the spring's
/// control points), bisection as the safety net.
fn solve_parameter(t: f32) -> f32 {
    let mut s = t;
    for _ in 0..8 {
        let err = bez_x(s) - t;
        if err.abs() < 1e-6 {
            return s.clamp(0.0, 1.0);
        }
        let d = bez_x_derivative(s);
        if d.abs() < 1e-6 {
            break;
        }
        s -= err / d;
        if !(0.0..=1.0).contains(&s) {
            break;
        }
    }
    // Bisection fallback.
    let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
    for _ in 0..32 {
        s = f32::midpoint(lo, hi);
        if bez_x(s) < t {
            lo = s;
        } else {
            hi = s;
        }
    }
    f32::midpoint(lo, hi)
}

/// The spring curve: cubic-bezier(0.34, 1.56, 0.64, 1).
///
/// Input is linear time progress in `[0, 1]` (values outside clamp
/// to the endpoints); output is eased progress — rises past 1.0
/// mid-flight (the overshoot) and lands exactly on 1.0.
pub(crate) fn spring(t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    bez_y(solve_parameter(t))
}

/// Advance animation progress toward `target` by `dt` seconds at
/// `duration` — the manual-clock equivalent of egui's
/// `animate_value_with_time`, kept here so callers can feed the
/// result through [`spring`]. Linear (the easing is [`spring`]'s
/// job); clamps at the endpoints; a non-positive `duration` jumps
/// straight to the target.
pub(crate) fn approach(current: f32, target: f32, dt: f32, duration: f32) -> f32 {
    if duration <= 0.0 {
        return target;
    }
    let step = (dt / duration).clamp(0.0, 1.0);
    let next = if target >= current {
        current + step
    } else {
        current - step
    };
    next.clamp(current.min(target), current.max(target))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(got: f32, want: f32, what: &str) {
        assert!((got - want).abs() < 1e-4, "{what}: got {got}, want {want}");
    }

    #[test]
    fn spring_hits_both_endpoints_exactly() {
        assert_close(spring(0.0), 0.0, "spring(0)");
        assert_close(spring(1.0), 1.0, "spring(1)");
    }

    #[test]
    fn spring_clamps_input_outside_the_unit_interval() {
        assert_close(spring(-0.5), 0.0, "spring(-0.5)");
        assert_close(spring(1.5), 1.0, "spring(1.5)");
    }

    #[test]
    fn spring_overshoots_within_a_bounded_envelope() {
        let mut max = f32::MIN;
        let mut min = f32::MAX;
        let steps: u16 = 2000;
        for i in 0..=steps {
            let y = spring(f32::from(i) / f32::from(steps));
            max = max.max(y);
            min = min.min(y);
        }
        assert!(
            max > 1.0,
            "the curve must overshoot past 1.0 (the pop), max was {max}"
        );
        assert!(
            max <= 1.25,
            "overshoot stays within the bounded envelope (+25%), max was {max}"
        );
        assert!(
            min >= -1e-4,
            "no undershoot below 0.0 for this curve, min was {min}"
        );
    }

    #[test]
    fn spring_midpoint_is_rising() {
        // On the way in the value is already substantial (the curve
        // is back-*out*: slow-in, fast-past-1, settle).
        let y = spring(0.5);
        assert!(y > 0.8, "mid-flight should be near/past 1.0, was {y}");
        assert!(y <= 1.25);
    }

    #[test]
    fn spring_is_deterministic() {
        for t in [0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0] {
            assert_eq!(spring(t).to_bits(), spring(t).to_bits());
        }
    }

    #[test]
    fn approach_moves_toward_the_target_and_clamps() {
        let mut p = 0.0_f32;
        for _ in 0..100 {
            p = approach(p, 1.0, 1.0 / 60.0, 0.2);
        }
        assert_close(p, 1.0, "a half-second of frames completes the ramp");
        // Overshoot is impossible at this layer — it belongs to
        // spring().
        assert!(p <= 1.0);
        assert!(approach(1.0, 1.0, 0.5, 0.2) <= 1.0);
    }

    #[test]
    fn approach_descends_toward_zero() {
        let mut p = 1.0_f32;
        for _ in 0..100 {
            p = approach(p, 0.0, 1.0 / 60.0, 0.2);
        }
        assert_close(p, 0.0, "hover-end ramps back down to rest");
        assert!(p >= 0.0);
    }

    #[test]
    fn approach_jumps_on_non_positive_duration() {
        assert_close(approach(0.3, 1.0, 0.016, 0.0), 1.0, "zero duration");
        assert_close(approach(0.3, 1.0, 0.016, -1.0), 1.0, "negative duration");
    }
}
