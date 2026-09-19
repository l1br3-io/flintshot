//! Double-click synthesis for the two event pumps.
//!
//! Core's [`InputEvent::DoubleClick`] re-edit trigger needs the shell
//! to detect the gesture and emit the event **in addition to** the
//! second press's [`InputEvent::PointerDown`], in the same frame,
//! immediately after it — the dispatcher relies on that ordering
//! (select, then upgrade to a re-edit session).
//!
//! egui 0.35 classifies clicks on *release*
//! (`PointerEvent::Released { click }`), which is both later than
//! this contract wants and unaware of the pumps' own press gating
//! (toolbar swallows, region holdout, gesture grabs). So the tracker
//! does its own press-time detection using egui's own thresholds
//! (`Options::max_double_click_delay` / `Options::max_click_dist`),
//! keeping the timing feel identical to native egui widgets.
//!
//! The pumps feed the tracker exactly when they dispatch a
//! `PointerDown`; presses swallowed upstream never count toward a
//! double-click.
//!
//! [`InputEvent::DoubleClick`]: flintshot_core::input::InputEvent::DoubleClick
//! [`InputEvent::PointerDown`]: flintshot_core::input::InputEvent::PointerDown

/// Press-time consecutive-click detector shared by both shells.
///
/// Positions are screen-logical px — the space egui's
/// `max_click_dist` threshold lives in. The pumps pass the logical
/// press position for the distance test and emit the synthesized
/// event with their own canvas-physical coordinate.
#[derive(Debug, Default)]
pub(crate) struct DoubleClickTracker {
    /// Last press fed to the tracker: `(time, logical pos)`.
    last: Option<(f64, egui::Pos2)>,
    /// Consecutive-press count of the current burst (1, 2, 3, then
    /// wrapping back to 1 — a fourth rapid press starts a fresh
    /// click, matching desktop convention).
    count: u32,
}

impl DoubleClickTracker {
    /// Record a dispatched press and report its consecutive-press
    /// count: 1 for a fresh click, 2 when within `max_delay` seconds
    /// and `max_dist` logical px of the previous press, 3 for the
    /// burst's third, then 1 again. The pumps emit
    /// [`InputEvent::DoubleClick`] on 2 and
    /// [`InputEvent::TripleClick`] on 3.
    ///
    /// A non-firing press still becomes the new "last" and resets the
    /// count — the next pair is measured against it, so a slow click
    /// followed by a rapid one still fires on the second pair.
    pub(crate) fn on_press(
        &mut self,
        now: f64,
        pos: egui::Pos2,
        max_delay: f64,
        max_dist: f32,
    ) -> u32 {
        let consecutive = self
            .last
            .is_some_and(|(t, p)| (now - t) < max_delay && p.distance(pos) < max_dist);
        self.count = if consecutive { (self.count % 3) + 1 } else { 1 };
        self.last = Some((now, pos));
        self.count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// egui's shipped defaults (`Options::default`), used so the
    /// tracker's tests exercise the same thresholds users feel.
    const DELAY: f64 = 0.3;
    const DIST: f32 = 6.0;

    #[test]
    fn rapid_same_spot_presses_count_up_and_wrap() {
        let mut t = DoubleClickTracker::default();
        assert_eq!(t.on_press(1.0, egui::pos2(30.0, 25.0), DELAY, DIST), 1);
        assert_eq!(t.on_press(1.1, egui::pos2(31.0, 26.0), DELAY, DIST), 2);
        assert_eq!(t.on_press(1.2, egui::pos2(30.0, 25.0), DELAY, DIST), 3);
        // Fourth rapid press wraps to a fresh click.
        assert_eq!(t.on_press(1.3, egui::pos2(30.0, 25.0), DELAY, DIST), 1);
        assert_eq!(t.on_press(1.4, egui::pos2(30.0, 25.0), DELAY, DIST), 2);
    }

    #[test]
    fn slow_presses_reset_to_one() {
        let mut t = DoubleClickTracker::default();
        assert_eq!(t.on_press(1.0, egui::pos2(30.0, 25.0), DELAY, DIST), 1);
        assert_eq!(t.on_press(3.0, egui::pos2(30.0, 25.0), DELAY, DIST), 1);
        // …but the slow pair resets the window: the *next* rapid
        // press against it counts as 2.
        assert_eq!(t.on_press(3.1, egui::pos2(30.0, 25.0), DELAY, DIST), 2);
    }

    #[test]
    fn distant_presses_reset_to_one() {
        let mut t = DoubleClickTracker::default();
        assert_eq!(t.on_press(1.0, egui::pos2(10.0, 10.0), DELAY, DIST), 1);
        assert_eq!(t.on_press(1.1, egui::pos2(50.0, 40.0), DELAY, DIST), 1);
        assert_eq!(t.on_press(1.2, egui::pos2(50.0, 40.0), DELAY, DIST), 2);
    }

    #[test]
    fn first_press_never_fires() {
        let mut t = DoubleClickTracker::default();
        assert_eq!(t.on_press(0.0, egui::pos2(0.0, 0.0), DELAY, DIST), 1);
    }
}
