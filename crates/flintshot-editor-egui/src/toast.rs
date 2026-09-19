//! The transient toast: a short-lived notice pill over any routed
//! surface (top-center, under the topbar). One live toast at a time —
//! a fresh notice replaces the old. Expiry runs on egui's frame time,
//! so the consumer must keep frames coming while one is up (the
//! Settings route's repaint heartbeat already does; [`Toast::show`]
//! asks for the remaining repaints itself).

use std::time::Duration;

/// How long a toast stays on screen.
const LIFETIME_SECS: f64 = 2.5;

/// Vertical offset below the topbar (44 px) the pill hangs at.
const TOP_OFFSET: f32 = 56.0;

/// One transient notice.
pub(crate) struct Toast {
    message: String,
    /// egui frame time the toast was raised at (`Context` time, not a
    /// wall clock — tests drive it through `RawInput::time`).
    born: f64,
}

impl Toast {
    /// Raise a toast at frame time `now`.
    pub(crate) fn new(message: impl Into<String>, now: f64) -> Self {
        Self {
            message: message.into(),
            born: now,
        }
    }

    /// Whether the toast's lifetime has passed.
    pub(crate) fn expired(&self, now: f64) -> bool {
        now - self.born >= LIFETIME_SECS
    }
}

/// Draw the live toast (if any), expiring it past its lifetime. The
/// single consumer is the window router's `ui_impl` — one pill per
/// frame, replaced wholesale by a newer notice.
pub(crate) fn show(ui: &mut egui::Ui, toast: &mut Option<Toast>) {
    let now = ui.input(|i| i.time);
    if toast.as_ref().is_some_and(|t| t.expired(now)) {
        *toast = None;
    }
    let Some(t) = toast.as_ref() else {
        return;
    };
    // Keep frames coming so the pill disappears without input.
    let remaining = LIFETIME_SECS - (now - t.born);
    ui.ctx()
        .request_repaint_after(Duration::from_secs_f64(remaining.max(0.0)));
    egui::Area::new(egui::Id::new("flintshot-toast"))
        .order(egui::Order::Foreground)
        .interactable(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, TOP_OFFSET])
        .show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(crate::theme::palette::FOREST_DEEP)
                .stroke(egui::Stroke::new(1.0, crate::theme::palette::GOLD))
                .inner_margin(egui::Margin::symmetric(14, 8))
                .corner_radius(egui::CornerRadius::same(crate::theme::CUTOUT_RADIUS))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(&t.message).color(crate::theme::palette::PAPER_CREAM),
                    );
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expires_after_its_lifetime() {
        let toast = Toast::new("hi", 10.0);
        assert!(!toast.expired(10.0 + LIFETIME_SECS - 0.1));
        assert!(toast.expired(10.0 + LIFETIME_SECS));
    }
}
