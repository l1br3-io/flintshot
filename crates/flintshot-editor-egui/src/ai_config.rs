//! The AI settings page (`Route::AiConfig`) — Ask-the-shot's config
//! home.
//!
//! The settings page's grammar: labelled cutout section cards in a
//! centered column. Two sections — **Model** (provider preset,
//! model id, base URL, API key, Test connection) and **Transforms**
//! (the saved-transform CRUD) — filled by Tasks 010/011; this module
//! ships the page shell plus the config snapshot / save-debounce
//! lifecycle the sections will hang from.
//!
//! Reached from a Settings link row, the transform dialog's
//! **Configure AI…** link (the editor parks and returns on Esc), and
//! `app --route ai-config` (cold start).

use flintshot_core::Config;

use crate::theme;
use crate::widgets;

/// How long after the last edit the page waits before handing the
/// working config to the save path (matches the settings page).
const SAVE_DEBOUNCE: f64 = 1.0;

/// The AI settings page's state: the working config copy plus the
/// queued save (handed to the router through the debounce drain).
pub(crate) struct AiConfigApp {
    /// The working config copy (edits land here, save on debounce).
    config: Config,
    /// When the first unsaved edit landed. `None` when persisted.
    dirty_since: Option<f64>,
}

impl AiConfigApp {
    /// Snapshot the page from a config.
    pub(crate) fn new(config: Config) -> Self {
        Self {
            config,
            dirty_since: None,
        }
    }

    /// The working config (the router reads it on the save drain).
    /// Task 010's controls drive it; the debounce test reads it
    /// meanwhile.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn config(&self) -> &Config {
        &self.config
    }

    /// Flag the page dirty (an edit landed at `now`). Task 010's
    /// controls call this; the debounce test exercises it meanwhile.
    #[cfg(test)]
    pub(crate) fn mark_dirty(&mut self, now: f64) {
        if self.dirty_since.is_none() {
            self.dirty_since = Some(now);
        }
    }

    /// The debounced save: `Some(config)` once `SAVE_DEBOUNCE` has
    /// passed since the last edit, `None` while dirty-but-waiting or
    /// clean.
    pub(crate) fn drain_save(&mut self, now: f64) -> Option<Config> {
        let since = self.dirty_since?;
        if now - since < SAVE_DEBOUNCE {
            return None;
        }
        self.dirty_since = None;
        Some(self.config.clone())
    }

    /// Whether Escape was pressed this frame (the back affordance —
    /// same detection as the settings page).
    fn escape_pressed(ctx: &egui::Context) -> bool {
        ctx.input(|i| {
            i.events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::Key {
                        key: egui::Key::Escape,
                        pressed: true,
                        ..
                    }
                )
            })
        })
    }
}

/// The page's draw entry (the router's [`Route::AiConfig`] body).
/// Returns whether Escape fired (the router restores the parked
/// editor or routes home).
pub(crate) struct AiConfigPage;

impl AiConfigPage {
    /// Draw the AI settings body. `now` is the frame clock.
    pub(crate) fn show(ui: &mut egui::Ui, _page: &mut AiConfigApp, _now: f64) -> bool {
        let escape = AiConfigApp::escape_pressed(ui.ctx());
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                let column = 640.0_f32.min(ui.available_width());
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.set_min_width(column);
                    ui.add_space(theme::SPACE_SCALE[3]);
                    model_section(ui);
                    transforms_section(ui);
                    ui.add_space(theme::SPACE_SCALE[3]);
                });
            });
        escape
    }
}

/// The Model section — provider preset, model id, base URL, key,
/// Test connection (Task 010 fills the body).
fn model_section(ui: &mut egui::Ui) {
    section_card(ui, "Model", |ui| {
        ui.label(
            egui::RichText::new("Provider, model, base URL, and API key — landing in Task 010.")
                .size(theme::TYPE_SM)
                .color(theme::palette::INK_SOFT),
        );
    });
}

/// The Transforms section — the saved-transform CRUD (Task 011 fills
/// the body).
fn transforms_section(ui: &mut egui::Ui) {
    section_card(ui, "Transforms", |ui| {
        ui.label(
            egui::RichText::new("Saved transforms are managed here — landing in Task 011.")
                .size(theme::TYPE_SM)
                .color(theme::palette::INK_SOFT),
        );
    });
}

/// One cutout section card (the settings page's).
fn section_card(ui: &mut egui::Ui, title: &str, body: impl FnMut(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(theme::palette::PAPER_CREAM)
        .corner_radius(egui::CornerRadius::same(theme::CUTOUT_RADIUS))
        .inner_margin(egui::Margin::same(theme::POD_PAD + 2))
        .stroke(theme::HAIRLINE_STROKE)
        .shadow(theme::SHADOW_RESTING)
        .show(ui, |ui| {
            widgets::section(
                ui,
                egui::Id::new(("ai-settings-section", title)),
                title,
                true,
                body,
            );
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debounce_drains_once_after_the_window() {
        let mut page = AiConfigApp::new(Config::default());
        assert!(page.drain_save(10.0).is_none(), "clean page never saves");
        page.mark_dirty(10.0);
        assert!(page.drain_save(10.5).is_none(), "inside the window");
        let saved = page.drain_save(11.5).expect("past the window");
        assert_eq!(saved.ai.provider, "openai");
        assert_eq!(page.config().ai.provider, "openai");
        assert!(page.drain_save(20.0).is_none(), "drains exactly once");
    }
}
