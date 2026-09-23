//! The AI settings page (`Route::AiConfig`) — Ask-the-shot's config
//! home.
//!
//! The settings page's grammar: labelled cutout section cards in a
//! centered column. Two sections — **Model** (provider preset,
//! model id, base-URL override, the masked Replace-key affordance,
//! Test connection) and **Transforms** (the saved-transform CRUD,
//! Task 011) — over the page's config snapshot + save-debounce
//! lifecycle. The page stays pure data + an action queue: the
//! router services keyring writes and test connections through the
//! page bridge (keyring writes land immediately on commit, config
//! fields persist on the debounce).
//!
//! The stored key is never read back into the UI: the key row shows
//! status copy ("Key saved in your system's secret store") and a
//! masked Replace-key field.
//!
//! Reached from a Settings link row, the transform dialog's
//! **Configure AI…** link (the editor parks and returns on Esc), and
//! `app --route ai-config` (cold start).

use flintshot_core::{Config, ProviderPreset};

use crate::theme;
use crate::widgets;

/// How long after the last edit the page waits before handing the
/// working config to the save path (matches the settings page).
const SAVE_DEBOUNCE: f64 = 1.0;

/// An action the page collected that the router must service
/// through the page bridge (the page stays pure data + this queue).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AiConfigAction {
    /// The Replace-key field committed: store the key in the OS
    /// keyring under `ai:<provider>` (immediate — never on the
    /// config debounce).
    SaveKey {
        /// Provider id (the keyring entry's target).
        provider: String,
        /// The new key (never displayed again after the commit).
        key: String,
    },
    /// The provider pills switched: refresh the key-status row for
    /// the new keyring entry id.
    ProviderChanged {
        /// The newly selected provider id.
        provider: String,
    },
    /// Test connection clicked: a text-only round-trip against the
    /// page's working provider settings.
    TestConnection {
        /// Provider id.
        provider: String,
        /// Model id.
        model: String,
        /// The effective base URL (override or preset).
        base_url: String,
    },
}

/// A test-connection attempt's lifecycle (the inline status row).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TestStatus {
    /// A request is in flight.
    Pending,
    /// The round-trip answered ("Connection works").
    Passed,
    /// The mapped failure copy.
    Failed(String),
}

/// The AI settings page's state.
pub(crate) struct AiConfigApp {
    /// The working config copy (edits land here, save on debounce).
    config: Config,
    /// When the first unsaved edit landed. `None` when persisted.
    dirty_since: Option<f64>,
    /// The Replace-key field's draft (masked in the UI; cleared on
    /// commit — a stored key never renders).
    key_draft: String,
    /// Whether the current provider has a stored key (bridge-served;
    /// tracks `key_saved_for` so a provider switch re-targets it).
    key_saved: bool,
    /// The provider id `key_saved` answers for.
    key_saved_for: String,
    /// The Test connection row's status.
    test: Option<TestStatus>,
    /// Clicked actions awaiting the router's drain.
    actions: Vec<AiConfigAction>,
}

impl AiConfigApp {
    /// Snapshot the page from a config.
    pub(crate) fn new(config: Config) -> Self {
        let key_saved_for = config.ai.provider.clone();
        Self {
            config,
            dirty_since: None,
            key_draft: String::new(),
            key_saved: false,
            key_saved_for,
            test: None,
            actions: Vec::new(),
        }
    }

    /// The working config.
    #[must_use]
    pub(crate) fn config(&self) -> &Config {
        &self.config
    }

    /// Flag the page dirty (an edit landed at `now`).
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

    /// Take the oldest clicked action, if one waits.
    pub(crate) fn drain_action(&mut self) -> Option<AiConfigAction> {
        if self.actions.is_empty() {
            None
        } else {
            Some(self.actions.remove(0))
        }
    }

    /// Test seam: seed the Replace-key draft (the field's typed
    /// path).
    #[cfg(test)]
    pub(crate) fn set_key_draft(&mut self, key: &str) {
        self.key_draft = key.into();
    }

    /// Test seam: mutate the working config directly (the fields'
    /// typed path).
    #[cfg(test)]
    pub(crate) fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }

    /// The bridge answered the key-status query for `provider`.
    pub(crate) fn set_key_saved(&mut self, provider: &str, saved: bool) {
        self.key_saved_for = provider.to_string();
        self.key_saved = saved;
    }

    /// The bridge answered the test connection.
    pub(crate) fn set_test_result(&mut self, result: Result<String, String>) {
        self.test = Some(match result {
            Ok(_) => TestStatus::Passed,
            Err(copy) => TestStatus::Failed(copy),
        });
    }

    /// Whether the current provider's key-status row is stale (a
    /// switch happened without a bridge answer yet).
    #[must_use]
    pub(crate) fn key_status_stale(&self) -> bool {
        self.key_saved_for != self.config.ai.provider
    }

    /// The key row's status copy. Never includes key material.
    #[must_use]
    pub(crate) fn key_status_text(&self) -> &'static str {
        if self.key_status_stale() {
            "Checking your secret store…"
        } else if self.key_saved {
            "Key saved in your system's secret store."
        } else {
            "No key stored yet — paste one below for this provider."
        }
    }

    /// The effective base URL (override or preset) — the value a
    /// Test connection / Run would hit.
    #[must_use]
    pub(crate) fn effective_base_url(&self) -> &str {
        self.config.ai.effective_base_url()
    }

    /// Whether Test connection is armed: a model and an endpoint are
    /// both required.
    #[must_use]
    pub(crate) fn test_armed(&self) -> bool {
        !self.config.ai.model.trim().is_empty() && !self.effective_base_url().is_empty()
    }

    /// Select a provider pill: preset the model when the current one
    /// is empty or was another preset's hint (the placeholder
    /// follows the provider choice), and re-target the key status.
    pub(crate) fn select_provider(&mut self, id: &str, now: f64) {
        let preset = ProviderPreset::for_id(id);
        let current = self.config.ai.model.trim();
        let was_hint = ProviderPreset::all()
            .iter()
            .any(|p| !p.model_hint.is_empty() && p.model_hint == current);
        if preset.id == "custom" {
            // Custom names no model — drop a stale hint so the field
            // visibly requires input.
            if was_hint {
                self.config.ai.model.clear();
            }
        } else if current.is_empty() || was_hint {
            self.config.ai.model = preset.model_hint.to_string();
        }
        self.config.ai.provider = preset.id.to_string();
        self.mark_dirty(now);
        self.actions.push(AiConfigAction::ProviderChanged {
            provider: preset.id.to_string(),
        });
        // A pending test answer no longer describes this provider.
        self.test = None;
    }

    /// Commit the Replace-key field: queue the keyring write and
    /// clear the draft (a stored key never renders again).
    pub(crate) fn commit_key_draft(&mut self) {
        let key = self.key_draft.trim().to_string();
        if key.is_empty() {
            return;
        }
        self.actions.push(AiConfigAction::SaveKey {
            provider: self.config.ai.provider.clone(),
            key,
        });
        self.key_draft.clear();
        // Optimistic: the write services on the next action drain;
        // a failure status would need the bridge's error surface
        // (logged app-side, the keyring is local-trusted).
        self.key_saved = true;
        self.key_saved_for = self.config.ai.provider.clone();
    }

    /// Fire Test connection (only meaningful when armed).
    pub(crate) fn test_connection(&mut self) {
        if !self.test_armed() || self.test == Some(TestStatus::Pending) {
            return;
        }
        self.actions.push(AiConfigAction::TestConnection {
            provider: self.config.ai.provider.clone(),
            model: self.config.ai.model.trim().to_string(),
            base_url: self.effective_base_url().to_string(),
        });
        self.test = Some(TestStatus::Pending);
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
    pub(crate) fn show(ui: &mut egui::Ui, page: &mut AiConfigApp, now: f64) -> bool {
        let escape = AiConfigApp::escape_pressed(ui.ctx());
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                let column = 640.0_f32.min(ui.available_width());
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.set_min_width(column);
                    ui.add_space(theme::SPACE_SCALE[3]);
                    model_section(ui, page, now);
                    transforms_section(ui);
                    ui.add_space(theme::SPACE_SCALE[3]);
                });
            });
        escape
    }
}

/// The Model section: provider pills, model id, base-URL override,
/// the key affordance, Test connection.
fn model_section(ui: &mut egui::Ui, page: &mut AiConfigApp, now: f64) {
    section_card(ui, "Model", |ui| {
        provider_row(ui, page, now);
        set_row(ui, "Model", |ui| {
            let hint = model_hint(page).to_owned();
            let response = ui.add(
                egui::TextEdit::singleline(&mut page.config.ai.model)
                    .id(egui::Id::new("ai-model"))
                    .hint_text(hint)
                    .desired_width(240.0),
            );
            if response.changed() {
                page.mark_dirty(now);
            }
        });
        set_row(ui, "Base URL", |ui| {
            let preset = page.config.ai.preset();
            let response = ui.add(
                egui::TextEdit::singleline(&mut page.config.ai.base_url)
                    .id(egui::Id::new("ai-base-url"))
                    .hint_text(if preset.base_url.is_empty() {
                        "http://localhost:11434/v1"
                    } else {
                        preset.base_url
                    })
                    .desired_width(240.0),
            );
            if response.changed() {
                page.mark_dirty(now);
            }
        });
        key_row(ui, page);
        test_row(ui, page);
    });
}

/// The provider pills: a horizontal well row over the preset names
/// (custom last). Selecting runs [`AiConfigApp::select_provider`].
fn provider_row(ui: &mut egui::Ui, page: &mut AiConfigApp, now: f64) {
    set_row(ui, "Provider", |ui| {
        let names: Vec<&str> = ProviderPreset::all().iter().map(|p| p.name).collect();
        let current = page.config.ai.preset();
        let Some(idx) = ProviderPreset::all()
            .iter()
            .position(|p| p.id == current.id)
        else {
            return;
        };
        if let Some(pick) = widgets::well_row(ui, egui::Id::new("ai-provider"), &names, idx) {
            if let Some(chosen) = ProviderPreset::all().get(pick) {
                if chosen.id != page.config.ai.provider {
                    page.select_provider(chosen.id, now);
                }
            }
        }
    });
}

/// The model field's hint: the current preset's suggestion.
fn model_hint(page: &AiConfigApp) -> &str {
    let hint = page.config.ai.preset().model_hint;
    if hint.is_empty() {
        "required — e.g. llama3.2-vision"
    } else {
        hint
    }
}

/// The key affordance: masked Replace-key field (Enter commits),
/// status copy beneath — a stored key never renders.
fn key_row(ui: &mut egui::Ui, page: &mut AiConfigApp) {
    set_row(ui, "API key", |ui| {
        let response = ui.add(
            egui::TextEdit::singleline(&mut page.key_draft)
                .id(egui::Id::new("ai-key"))
                .password(true)
                .hint_text("Replace key…")
                .desired_width(240.0),
        );
        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            page.commit_key_draft();
            response.request_focus();
        }
    });
    status_line(ui, page.key_status_text());
}

/// The Test connection row: the pill + the inline pass/fail line.
fn test_row(ui: &mut egui::Ui, page: &mut AiConfigApp) {
    set_row(ui, "Connection", |ui| {
        let pending = page.test.as_ref() == Some(&TestStatus::Pending);
        if ghost_pill(
            ui,
            egui::Id::new("ai-test-connection"),
            "Test connection",
            page.test_armed() && !pending,
        ) {
            page.test_connection();
        }
    });
    match page.test.as_ref() {
        None => {}
        Some(TestStatus::Pending) => status_line(ui, "Testing…"),
        Some(TestStatus::Passed) => status_line(ui, "Connection works"),
        Some(TestStatus::Failed(copy)) => status_line(ui, copy),
    }
}

/// A dim status/copy line under a row.
fn status_line(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(theme::TYPE_XS)
            .color(theme::palette::INK_SOFT),
    );
    ui.add_space(theme::SPACE_SCALE[1]);
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

/// A `set-row`: label left, control right (the settings page's).
fn set_row(ui: &mut egui::Ui, label: &str, control: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(label)
                .size(theme::TYPE_SM)
                .color(theme::palette::INK),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), control);
    });
    ui.add_space(theme::SPACE_SCALE[1]);
}

/// A quiet action pill (the Test connection button): forest fill
/// when armed, dim + inert when not. Returns whether it fired.
fn ghost_pill(ui: &mut egui::Ui, id: egui::Id, label: &str, enabled: bool) -> bool {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::proportional(theme::TYPE_SM),
        if enabled {
            theme::palette::CREAM_TEXT
        } else {
            theme::palette::LOCK_GREY
        },
    );
    let w = 12.0 + galley.size().x + 12.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 24.0), egui::Sense::hover());
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let resp = ui.interact(rect, id, sense);
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(theme::POD_RADIUS),
        if enabled {
            theme::palette::FOREST_MID
        } else {
            theme::palette::PAPER_DIM
        },
        egui::Stroke::NONE,
        egui::StrokeKind::Middle,
    );
    ui.painter().galley(
        egui::pos2(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley.clone(),
        if enabled {
            theme::palette::CREAM_TEXT
        } else {
            theme::palette::LOCK_GREY
        },
    );
    resp.clicked() && enabled
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

    #[test]
    fn provider_switch_presets_the_model_and_retargets_the_key() {
        let mut page = AiConfigApp::new(Config::default());
        page.select_provider("xai", 1.0);
        assert_eq!(page.config().ai.provider, "xai");
        assert_eq!(
            page.config().ai.model,
            "grok-2-vision-1212",
            "empty model takes the preset's hint"
        );
        assert_eq!(
            page.drain_action(),
            Some(AiConfigAction::ProviderChanged {
                provider: "xai".into()
            }),
            "the switch queues a key-status refresh"
        );
        assert!(page.key_status_stale(), "the status waits on the bridge");
        assert_eq!(
            page.key_status_text(),
            "Checking your secret store…",
            "never key material"
        );

        // A typed model survives switches; another preset's hint
        // doesn't.
        page.config.ai.model = "my-model".into();
        page.select_provider("gemini", 2.0);
        assert_eq!(page.config().ai.model, "my-model", "user input wins");
        page.config.ai.model = "gpt-4o-mini".into(); // openai's hint
        page.select_provider("xai", 3.0);
        assert_eq!(
            page.config().ai.model,
            "grok-2-vision-1212",
            "a stale preset hint follows the new preset"
        );
        page.select_provider("custom", 4.0);
        assert!(
            page.config().ai.model.is_empty(),
            "custom drops a stale hint so the field visibly requires input"
        );
    }

    #[test]
    fn key_commit_queues_the_write_and_never_echoes_the_key() {
        let mut page = AiConfigApp::new(Config::default());
        page.key_draft = "  sk-secret-123  ".into();
        page.commit_key_draft();
        assert_eq!(
            page.drain_action(),
            Some(AiConfigAction::SaveKey {
                provider: "openai".into(),
                key: "sk-secret-123".into()
            }),
            "the keyring write queues with the current provider, trimmed"
        );
        assert!(page.key_draft.is_empty(), "the draft clears on commit");
        assert_eq!(
            page.key_status_text(),
            "Key saved in your system's secret store.",
            "optimistic status after commit"
        );
        // The status copy surface never contains key material.
        assert!(!page.key_status_text().contains("sk-secret"));
    }

    #[test]
    fn test_connection_gates_on_model_and_endpoint() {
        let mut page = AiConfigApp::new(Config::default());
        assert!(!page.test_armed(), "default config has no model");
        page.test_connection();
        assert!(page.actions.is_empty(), "nothing fires unarmed");

        page.config.ai.model = "gpt-4o-mini".into();
        assert!(page.test_armed());
        page.test_connection();
        assert_eq!(
            page.drain_action(),
            Some(AiConfigAction::TestConnection {
                provider: "openai".into(),
                model: "gpt-4o-mini".into(),
                base_url: "https://api.openai.com/v1".into(),
            })
        );
        assert_eq!(page.test, Some(TestStatus::Pending));
        page.test_connection();
        assert!(
            page.drain_action().is_none(),
            "a pending test swallows re-clicks"
        );

        page.set_test_result(Err(
            "The provider rejected the key — replace it in AI settings.".into(),
        ));
        assert_eq!(
            page.test,
            Some(TestStatus::Failed(
                "The provider rejected the key — replace it in AI settings.".into()
            ))
        );
        page.set_test_result(Ok("ready".into()));
        assert_eq!(page.test, Some(TestStatus::Passed));
    }

    /// The `[ai]` section the debounce writes matches the page's
    /// fields (round-trip through TOML, kebab-case keys).
    #[test]
    fn config_round_trips_through_the_debounce() {
        let mut page = AiConfigApp::new(Config::default());
        page.config.ai.provider = "custom".into();
        page.config.ai.model = "llama3.2-vision".into();
        page.config.ai.base_url = "http://localhost:11434/v1".into();
        page.mark_dirty(0.0);
        let saved = page.drain_save(2.0).expect("past the window");
        let toml = toml::to_string(&saved).expect("serialize");
        assert!(toml.contains("[ai]"), "body:\n{toml}");
        assert!(toml.contains("provider = \"custom\""));
        assert!(toml.contains("model = \"llama3.2-vision\""));
        assert!(toml.contains("base-url = \"http://localhost:11434/v1\""));
    }
}
