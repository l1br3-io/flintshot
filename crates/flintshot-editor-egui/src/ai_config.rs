//! The AI settings page (`Route::AiConfig`) — Ask-the-shot's config
//! home.
//!
//! The settings page's grammar: labelled cutout section cards in a
//! centered 680 px column. Two sections — **Model** (provider preset,
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
//! Reached from the topbar overflow's **AI settings** row (every
//! routed surface; routing from the editor parks it, and Esc
//! restores the annotated editor), a Settings link row, the
//! transform dialog's **Configure AI…** link, and
//! `app --route ai-config` (cold start).

use flintshot_core::{Config, ProviderPreset, TransformStore};

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
    /// Test connection clicked: a vision round-trip (a tiny inline
    /// PNG + one-word prompt — the same `image_url` content part the
    /// transform path sends) against the page's working provider
    /// settings.
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
    /// The round-trip answered (image input accepted).
    Passed,
    /// The mapped failure copy.
    Failed(String),
}

/// The inline transform editor's draft (creating when `id` is
/// `None`).
#[derive(Debug, Clone, PartialEq, Eq)]
struct EditingTransform {
    /// The transform being edited; `None` = a fresh one.
    id: Option<String>,
    /// Name draft.
    name: String,
    /// Prompt draft.
    prompt: String,
}

impl EditingTransform {
    /// Whether the draft commits (both fields non-empty after
    /// trim).
    fn committable(&self) -> bool {
        !self.name.trim().is_empty() && !self.prompt.trim().is_empty()
    }
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
    /// Whether the last keyring write for the current provider
    /// failed (bridge-served; shown instead of a phantom saved copy).
    key_store_failed: bool,
    /// The provider id `key_saved` answers for.
    key_saved_for: String,
    /// The last known vision-probe verdict + the triple it was
    /// measured on (seeded from the persisted `[ai]` fields on open,
    /// updated live by the probe-arrival poll). Renders the banner
    /// while the triple matches the working settings.
    probe: Option<flintshot_core::ProbeOutcome>,
    /// The Test connection row's status.
    test: Option<TestStatus>,
    /// Clicked actions awaiting the router's drain.
    actions: Vec<AiConfigAction>,
    /// The saved-transform store (CRUD user data — deliberately not
    /// part of `config.toml`). Writes persist immediately on
    /// commit, never on the config debounce.
    transforms: TransformStore,
    /// Where the transforms store persists.
    transforms_path: std::path::PathBuf,
    /// The inline transform editor's open draft, if any.
    editing: Option<EditingTransform>,
}

impl AiConfigApp {
    /// Snapshot the page from a config.
    pub(crate) fn new(config: Config) -> Self {
        let key_saved_for = config.ai.provider.clone();
        let (transforms, transforms_path) = default_transform_store();
        // The persisted verdict rehydrates the banner on open (zero
        // network — the save-change probe only fires on a save).
        let probe = match (
            config.ai.probe_model.as_deref(),
            config.ai.probe_base_url.as_deref(),
            config.ai.probe_verdict.as_deref(),
        ) {
            (Some(model), Some(base_url), Some(verdict)) => Some(flintshot_core::ProbeOutcome {
                verdict: flintshot_core::ProbeVerdict::parse(verdict),
                model: model.to_string(),
                base_url: base_url.to_string(),
            }),
            _ => None,
        };
        Self {
            config,
            dirty_since: None,
            key_draft: String::new(),
            key_saved: false,
            key_store_failed: false,
            key_saved_for,
            probe,
            test: None,
            actions: Vec::new(),
            transforms,
            transforms_path,
            editing: None,
        }
    }

    /// Test seam: a page over an explicit transforms store path
    /// (tempdir-backed tests never touch the real config dir).
    #[cfg(test)]
    pub(crate) fn with_transforms_path(config: Config, path: std::path::PathBuf) -> Self {
        let key_saved_for = config.ai.provider.clone();
        let mut page = Self::new(config);
        page.transforms = TransformStore::load_from(&path);
        page.transforms_path = path;
        let _ = key_saved_for;
        page
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
        self.key_store_failed = false;
    }

    /// The bridge's keyring write for `provider` failed: surface the
    /// failure instead of a phantom "saved" copy.
    pub(crate) fn set_key_store_failed(&mut self, provider: &str) {
        self.key_saved_for = provider.to_string();
        self.key_saved = false;
        self.key_store_failed = true;
    }

    /// The bridge answered the test connection.
    pub(crate) fn set_test_result(&mut self, result: Result<String, String>) {
        self.test = Some(match result {
            Ok(_) => TestStatus::Passed,
            Err(copy) => TestStatus::Failed(copy),
        });
    }

    /// The bridge settled a vision probe: adopt the verdict + triple
    /// (the banner's live update path).
    pub(crate) fn note_probe_outcome(&mut self, outcome: &flintshot_core::ProbeOutcome) {
        self.probe = Some(outcome.clone());
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
        } else if self.key_store_failed {
            "Couldn't store the key — check your secret store is unlocked, then retry."
        } else if self.key_saved {
            "Key saved in your system's secret store."
        } else {
            "No key stored yet — paste a key above and press Save key."
        }
    }

    /// The key field's context-aware hint (shown only while the
    /// draft is empty): the replace copy once a key is stored for
    /// the current provider, otherwise a paste prompt naming the
    /// provider — the preset's display name with any parenthetical
    /// suffix stripped, so "xAI (Grok)" reads `xAI`. Stale (post
    /// provider-switch) and store-failed states keep the paste
    /// prompt: neither changes what the field wants. Never includes
    /// key material.
    #[must_use]
    pub(crate) fn key_hint_text(&self) -> String {
        if self.key_saved && !self.key_status_stale() {
            "Paste a new key to replace the stored one…".to_string()
        } else {
            let name = self.config.ai.preset().name;
            let short = name.split(" (").next().unwrap_or(name);
            format!("Paste your {short} API key…")
        }
    }

    /// Whether the Save-key pill is armed: the draft holds content
    /// beyond whitespace (the same bar [`AiConfigApp::commit_key_draft`]
    /// applies, so an armed click always commits something).
    #[must_use]
    pub(crate) fn key_save_armed(&self) -> bool {
        !self.key_draft.trim().is_empty()
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
    /// clear the draft (a stored key never renders again). The
    /// status copy waits for the bridge's answer — the router
    /// services the write the same frame and reports success or
    /// failure; the page never claims "saved" on its own.
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

    // --- Transforms CRUD (immediate store writes, never the
    // config debounce) ---

    /// The saved transforms, display order.
    #[must_use]
    pub(crate) fn transforms(&self) -> &[flintshot_core::AiTransform] {
        self.transforms.all()
    }

    /// Open the inline creator (the *New transform…* affordance).
    pub(crate) fn begin_create_transform(&mut self) {
        self.editing = Some(EditingTransform {
            id: None,
            name: String::new(),
            prompt: String::new(),
        });
    }

    /// Open the inline editor over an existing transform.
    pub(crate) fn begin_edit_transform(&mut self, id: &str) {
        let Some(target) = self.transforms.get(id) else {
            return;
        };
        self.editing = Some(EditingTransform {
            id: Some(target.id.clone()),
            name: target.name.clone(),
            prompt: target.prompt.clone(),
        });
    }

    /// Commit the inline editor: insert (creating) or update
    /// (editing), then persist immediately. Returns whether the
    /// commit landed (a disabled Save or an unknown id is a no-op).
    pub(crate) fn commit_transform(&mut self) -> bool {
        let Some(draft) = self.editing.take() else {
            return false;
        };
        if !draft.committable() {
            // Keep the draft: the fields need input.
            self.editing = Some(draft);
            return false;
        }
        let landed = match draft.id.as_deref() {
            None => {
                self.transforms
                    .insert(draft.name.trim(), draft.prompt.trim());
                true
            }
            Some(id) => self
                .transforms
                .update(id, draft.name.trim(), draft.prompt.trim()),
        };
        if landed {
            self.persist_transforms();
        }
        landed
    }

    /// Discard the inline editor.
    pub(crate) fn cancel_transform_edit(&mut self) {
        self.editing = None;
    }

    /// Delete a transform by id (immediate persist).
    pub(crate) fn delete_transform(&mut self, id: &str) {
        if self.transforms.remove(id) {
            self.persist_transforms();
        }
    }

    /// Reorder by delta (clamped at the ends — the store's move
    /// op); immediate persist.
    pub(crate) fn move_transform(&mut self, id: &str, delta: isize) {
        if self.transforms.move_by(id, delta) {
            self.persist_transforms();
        }
    }

    /// The inline editor's open draft, if any (assertion accessor
    /// for tests).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn editing_prompt(&self) -> Option<&str> {
        self.editing.as_ref().map(|draft| draft.prompt.as_str())
    }

    /// Persist the transforms store (atomic save). A failure logs —
    /// the list stays usable in-memory and the next write retries.
    fn persist_transforms(&self) {
        if let Err(e) = self.transforms.save_to(&self.transforms_path) {
            tracing::warn!(
                path = %self.transforms_path.display(),
                error = %e,
                "saving the transforms store"
            );
        }
    }
}

/// The platform transforms store + its path (the default page
/// construction; tests inject via
/// [`AiConfigApp::with_transforms_path`]).
fn default_transform_store() -> (TransformStore, std::path::PathBuf) {
    match flintshot_core::transform_store_path() {
        Some(path) => (TransformStore::load_from(&path), path),
        None => (TransformStore::new(), std::path::PathBuf::new()),
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
                // A real centered column: cap the content width so
                // wide windows center the 680 px column instead of
                // stretching the cards edge-to-edge.
                let column = 680.0_f32.min(ui.available_width());
                let inset = ((ui.available_width() - column) / 2.0).max(0.0);
                ui.horizontal(|ui| {
                    ui.add_space(inset);
                    ui.vertical(|ui| {
                        ui.set_min_width(column);
                        ui.set_max_width(column);
                        ui.add_space(theme::SPACE_SCALE[3]);
                        model_section(ui, page, now);
                        transforms_section(ui, page);
                        ui.add_space(theme::SPACE_SCALE[3]);
                    });
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
                    .desired_width(ui.available_width()),
            );
            if response.changed() {
                page.mark_dirty(now);
            }
        });
        probe_banner(ui, page);
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
                    .desired_width(ui.available_width()),
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

/// The vision-probe verdict banner under the model field. Shows the
/// persisted verdict only while it was measured on exactly the
/// working `(model, base-url)` — a stale verdict never badges a
/// different model. Strictly advisory: ok renders a muted
/// sees-images line, unsupported a berry not-seen line with a
/// **Check again** re-test affordance; unknown stays silent (it
/// says nothing about vision).
fn probe_banner(ui: &mut egui::Ui, page: &mut AiConfigApp) {
    let Some(outcome) = page.probe.as_ref() else {
        return;
    };
    if page.config.ai.model.trim() != outcome.model || page.effective_base_url() != outcome.base_url
    {
        return;
    }
    match outcome.verdict {
        flintshot_core::ProbeVerdict::VisionOk => {
            // The check is a painted vector, not a font codepoint —
            // the tofu ban (icons are painted glyphs in this crate).
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Model sees images")
                        .size(theme::TYPE_SM)
                        .color(theme::palette::INK_SOFT),
                );
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                let center = rect.center();
                ui.painter().line_segment(
                    [
                        egui::pos2(center.x - 4.0, center.y + 0.5),
                        egui::pos2(center.x - 1.0, center.y + 3.5),
                    ],
                    egui::Stroke::new(1.5, theme::palette::FOREST_MID),
                );
                ui.painter().line_segment(
                    [
                        egui::pos2(center.x - 1.0, center.y + 3.5),
                        egui::pos2(center.x + 4.5, center.y - 3.5),
                    ],
                    egui::Stroke::new(1.5, theme::palette::FOREST_MID),
                );
            });
            ui.add_space(theme::SPACE_SCALE[1]);
        }
        flintshot_core::ProbeVerdict::VisionUnsupported => {
            // Hoist the copy before the pill closure's mutable page
            // borrow (the key row's idiom).
            let line = format!(
                "Model `{}` may not accept image input — answers arrived without seeing \
                 the PNG",
                outcome.model
            );
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(line)
                        .size(theme::TYPE_SM)
                        .color(theme::palette::BERRY_RED),
                );
                if ghost_pill(ui, egui::Id::new("ai-probe-retest"), "Check again", true) {
                    page.test_connection();
                }
            });
            ui.add_space(theme::SPACE_SCALE[1]);
        }
        flintshot_core::ProbeVerdict::VisionUnknown => {}
    }
}

/// The key affordance: a masked field plus a **Save key** pill to
/// its right — the visible commit path, dim + inert until the draft
/// holds content, with Enter kept as the keyboard shortcut. The
/// field's hint is context-aware
/// ([`AiConfigApp::key_hint_text`]); status copy beneath — a stored
/// key never renders.
fn key_row(ui: &mut egui::Ui, page: &mut AiConfigApp) {
    set_row(ui, "API key", |ui| {
        // set_row lays the control right-to-left: reserve the pill
        // first so it lands rightmost, then the field fills the rest.
        let armed = page.key_save_armed();
        if ghost_pill(ui, egui::Id::new("ai-key-save"), "Save key", armed) {
            page.commit_key_draft();
        }
        // Hoist the hint before the mutable draft borrow (the Model
        // row's idiom): the field borrows `key_draft` for the frame.
        let hint = page.key_hint_text();
        let response = ui.add(
            egui::TextEdit::singleline(&mut page.key_draft)
                .id(egui::Id::new("ai-key"))
                .password(true)
                .hint_text(hint)
                .desired_width(ui.available_width()),
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
        Some(TestStatus::Passed) => {
            status_line(ui, "Connection works — the model described the image");
        }
        Some(TestStatus::Failed(copy)) => status_line(ui, copy),
    }
}

/// A dim status/copy line under a row.
fn status_line(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(theme::TYPE_SM)
            .color(theme::palette::INK_SOFT),
    );
    ui.add_space(theme::SPACE_SCALE[1]);
}

/// The Transforms section: the saved-transform list (name + prompt
/// excerpt) with add / edit / delete / reorder, persisted to
/// `ai-transforms.toml` immediately on commit.
fn transforms_section(ui: &mut egui::Ui, page: &mut AiConfigApp) {
    section_card(ui, "Transforms", |ui| {
        if page.transforms().is_empty() {
            ui.label(
                egui::RichText::new("No saved transforms — add one")
                    .size(theme::TYPE_SM)
                    .color(theme::palette::INK_SOFT),
            );
            ui.add_space(theme::SPACE_SCALE[1]);
        } else {
            let snapshot: Vec<flintshot_core::AiTransform> = page.transforms().to_vec();
            for transform in snapshot {
                transform_row(ui, page, &transform);
            }
        }
        if page.editing.is_some() {
            // The editor closure borrows the draft; the intent comes
            // back as a value so the page applies it after.
            let mut draft = page.editing.take().expect("checked is_some");
            match transform_editor(ui, &mut draft) {
                EditorIntent::Save => {
                    page.editing = Some(draft);
                    let _ = page.commit_transform();
                }
                EditorIntent::Cancel => page.cancel_transform_edit(),
                EditorIntent::None => page.editing = Some(draft),
            }
        } else if ghost_pill(
            ui,
            egui::Id::new("ai-transform-new"),
            "New transform…",
            true,
        ) {
            page.begin_create_transform();
        }
    });
}

/// What the inline editor asked for this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorIntent {
    /// Nothing fired.
    None,
    /// Save clicked (the page commits the draft).
    Save,
    /// Cancel clicked (the page discards the draft).
    Cancel,
}

/// One saved-transform row: name + one-line prompt excerpt left,
/// reorder / edit / delete controls right.
fn transform_row(
    ui: &mut egui::Ui,
    page: &mut AiConfigApp,
    transform: &flintshot_core::AiTransform,
) {
    let editing_this = page
        .editing
        .as_ref()
        .is_some_and(|draft| draft.id.as_deref() == Some(transform.id.as_str()));
    egui::Frame::new()
        .fill(theme::palette::PAPER_DIM)
        .corner_radius(egui::CornerRadius::same(theme::CUTOUT_RADIUS))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // One exact allocation for the whole row; the labels
            // paint left and the pill strip places itself from the
            // right edge. Nested layout children (a vertical label
            // column, a right_to_left control column) mis-place in
            // egui's flow here — the controls ended up on their own
            // row below the card.
            let w = ui.available_width();
            let sx = ui.spacing().item_spacing.x;
            let sy = ui.spacing().item_spacing.y;
            let name_galley = ui.painter().layout_no_wrap(
                transform.name.clone(),
                egui::FontId::proportional(theme::TYPE_MD),
                theme::palette::INK,
            );
            let excerpt_galley = ui.painter().layout_no_wrap(
                prompt_excerpt(&transform.prompt),
                egui::FontId::proportional(theme::TYPE_SM),
                theme::palette::INK_SOFT,
            );
            let labels_h = name_galley.size().y + sy + excerpt_galley.size().y;
            let h = labels_h.max(theme::CONTROL_H).max(32.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
            let name_h = name_galley.size().y;
            ui.painter().galley(
                egui::pos2(rect.left(), rect.center().y - labels_h / 2.0),
                name_galley,
                theme::palette::INK,
            );
            ui.painter().galley(
                egui::pos2(rect.left(), rect.center().y - labels_h / 2.0 + name_h + sy),
                excerpt_galley,
                theme::palette::INK_SOFT,
            );
            let half = theme::CONTROL_H / 2.0;
            let mut right = rect.right();
            let pill_rect = |right: f32, w: f32| {
                egui::Rect::from_min_max(
                    egui::pos2(right - w, rect.center().y - half),
                    egui::pos2(right, rect.center().y + half),
                )
            };
            let del_w = pill_width(ui, "Delete");
            if pill_at(
                ui,
                pill_rect(right, del_w),
                egui::Id::new(("ai-transform-del", transform.id.clone())),
                "Delete",
                true,
            ) {
                page.cancel_transform_edit();
                page.delete_transform(&transform.id);
            }
            right -= del_w + sx;
            if !editing_this {
                let edit_w = pill_width(ui, "Edit");
                if pill_at(
                    ui,
                    pill_rect(right, edit_w),
                    egui::Id::new(("ai-transform-edit", transform.id.clone())),
                    "Edit",
                    true,
                ) {
                    page.begin_edit_transform(&transform.id);
                }
                right -= edit_w + sx;
            }
            let down_w = pill_width(ui, "↓");
            if pill_at(
                ui,
                pill_rect(right, down_w),
                egui::Id::new(("ai-transform-down", transform.id.clone())),
                "↓",
                true,
            ) {
                page.move_transform(&transform.id, 1);
            }
            right -= down_w + sx;
            let up_w = pill_width(ui, "↑");
            if pill_at(
                ui,
                pill_rect(right, up_w),
                egui::Id::new(("ai-transform-up", transform.id.clone())),
                "↑",
                true,
            ) {
                page.move_transform(&transform.id, -1);
            }
        });
    ui.add_space(theme::SPACE_SCALE[1]);
}

/// The inline creator/editor: name + prompt fields, Save (gated on
/// both non-empty) and Cancel. Returns the frame's intent; the
/// draft itself stays owned by the caller.
fn transform_editor(ui: &mut egui::Ui, draft: &mut EditingTransform) -> EditorIntent {
    let mut intent = EditorIntent::None;
    egui::Frame::new()
        .fill(theme::palette::PAPER)
        .corner_radius(egui::CornerRadius::same(theme::CUTOUT_RADIUS))
        .inner_margin(egui::Margin::same(10))
        .stroke(theme::HAIRLINE_STROKE)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Name")
                        .size(theme::TYPE_SM)
                        .color(theme::palette::INK),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut draft.name)
                        .id(egui::Id::new("ai-transform-name"))
                        .desired_width(ui.available_width()),
                );
            });
            ui.add_space(theme::SPACE_SCALE[0]);
            ui.add(
                egui::TextEdit::singleline(&mut draft.prompt)
                    .id(egui::Id::new("ai-transform-prompt"))
                    .hint_text("The instruction sent with the image…")
                    .desired_width(ui.available_width()),
            );
            ui.add_space(theme::SPACE_SCALE[1]);
            ui.horizontal(|ui| {
                if ghost_pill(
                    ui,
                    egui::Id::new("ai-transform-save"),
                    "Save",
                    draft.committable(),
                ) {
                    intent = EditorIntent::Save;
                }
                if ghost_pill(ui, egui::Id::new("ai-transform-cancel"), "Cancel", true) {
                    intent = EditorIntent::Cancel;
                }
            });
        });
    ui.add_space(theme::SPACE_SCALE[1]);
    intent
}

/// One line of prompt copy, ellipsized (the row's excerpt).
fn prompt_excerpt(prompt: &str) -> String {
    let first = prompt.lines().next().unwrap_or_default();
    if first.chars().count() > 48 {
        let cut: String = first.chars().take(46).collect();
        format!("{cut}…")
    } else {
        first.to_string()
    }
}

/// A `set-row`: label left, control right (the settings page's).
fn set_row(ui: &mut egui::Ui, label: &str, control: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(label)
                .size(theme::TYPE_MD)
                .color(theme::palette::INK),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), control);
    });
    ui.add_space(theme::SPACE_SCALE[1]);
}

/// Measure a [`ghost_pill`] for its label (pads included) — rows
/// that lay their own geometry budget their controls first.
fn pill_width(ui: &egui::Ui, label: &str) -> f32 {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::proportional(theme::TYPE_MD),
        theme::palette::CREAM_TEXT,
    );
    14.0 + galley.size().x + 14.0
}

/// Paint a [`ghost_pill`] into an exact rect (rows that lay their
/// own geometry). Returns whether it fired.
fn pill_at(ui: &mut egui::Ui, rect: egui::Rect, id: egui::Id, label: &str, enabled: bool) -> bool {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::proportional(theme::TYPE_MD),
        if enabled {
            theme::palette::CREAM_TEXT
        } else {
            theme::palette::LOCK_GREY
        },
    );
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
        galley,
        if enabled {
            theme::palette::CREAM_TEXT
        } else {
            theme::palette::LOCK_GREY
        },
    );
    resp.clicked() && enabled
}

/// A quiet action pill (the Test connection button): forest fill
/// when armed, dim + inert when not. Returns whether it fired.
fn ghost_pill(ui: &mut egui::Ui, id: egui::Id, label: &str, enabled: bool) -> bool {
    let w = pill_width(ui, label);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, theme::CONTROL_H), egui::Sense::hover());
    pill_at(ui, rect, id, label, enabled)
}

/// One cutout section card (the settings page's).
fn section_card(ui: &mut egui::Ui, title: &str, body: impl FnMut(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(theme::palette::PAPER_CREAM)
        .corner_radius(egui::CornerRadius::same(theme::CUTOUT_RADIUS))
        // SPACE_SCALE[2] — generous card gutters on the bumped scale.
        .inner_margin(egui::Margin::same(14))
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
            "grok-4.7",
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
            "grok-4.7",
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
        // The commit alone claims nothing — the bridge's answer (same
        // frame, router-side) decides the status. Regression: the
        // page used to optimistically claim "Key saved" even when the
        // keyring write failed, sending the user chasing a phantom
        // bad key.
        assert_eq!(
            page.key_status_text(),
            "No key stored yet — paste a key above and press Save key.",
            "no optimistic saved copy before the bridge answers"
        );
        page.set_key_saved("openai", true);
        assert_eq!(
            page.key_status_text(),
            "Key saved in your system's secret store.",
            "the bridge's confirmation lands"
        );
        // The status copy surface never contains key material.
        assert!(!page.key_status_text().contains("sk-secret"));
    }

    #[test]
    fn key_save_armed_requires_content_beyond_whitespace() {
        let mut page = AiConfigApp::new(Config::default());
        assert!(!page.key_save_armed(), "an empty draft keeps the pill dim");
        page.set_key_draft("   ");
        assert!(!page.key_save_armed(), "whitespace-only stays dim");
        page.set_key_draft("sk-fake-key");
        assert!(page.key_save_armed(), "real content arms the pill");
    }

    #[test]
    fn key_hint_prompts_for_the_current_provider_when_nothing_is_stored() {
        let mut page = AiConfigApp::new(Config::default());
        page.set_key_draft("sk-fake-key");
        assert_eq!(
            page.key_hint_text(),
            "Paste your OpenAI API key…",
            "the default page names its provider"
        );
        // The hint surface never echoes key material.
        assert!(!page.key_hint_text().contains("sk-fake"));
    }

    #[test]
    fn key_hint_switches_to_replace_copy_once_a_key_is_stored() {
        let mut page = AiConfigApp::new(Config::default());
        page.set_key_saved("openai", true);
        assert_eq!(
            page.key_hint_text(),
            "Paste a new key to replace the stored one…"
        );
    }

    #[test]
    fn key_hint_retargets_to_the_new_provider_while_stale() {
        let mut page = AiConfigApp::new(Config::default());
        page.set_key_saved("openai", true);
        page.select_provider("xai", 1.0);
        assert!(
            page.key_status_stale(),
            "the bridge hasn't answered for xai"
        );
        // A stored key for the *old* provider must not leak the
        // replace copy — the field wants a key for the new one.
        assert_eq!(page.key_hint_text(), "Paste your xAI API key…");
    }

    #[test]
    fn key_hint_keeps_the_paste_prompt_after_a_store_failure() {
        let mut page = AiConfigApp::new(Config::default());
        page.set_key_store_failed("openai");
        assert_eq!(
            page.key_hint_text(),
            "Paste your OpenAI API key…",
            "a failed store doesn't change what the field wants"
        );
    }

    #[test]
    fn key_hint_strips_parenthetical_suffixes_from_preset_names() {
        let mut page = AiConfigApp::new(Config::default());
        page.select_provider("custom", 1.0);
        assert_eq!(
            page.key_hint_text(),
            "Paste your Custom API key…",
            "'Custom (OpenAI-compatible)' shortens to 'Custom'"
        );
    }

    #[test]
    fn key_store_failure_surfaces_instead_of_a_phantom_saved_copy() {
        let mut page = AiConfigApp::new(Config::default());
        page.key_draft = "sk-secret-123".into();
        page.commit_key_draft();
        let _ = page.drain_action();

        // The bridge answers: the store rejected the write.
        page.set_key_store_failed("openai");
        assert_eq!(
            page.key_status_text(),
            "Couldn't store the key — check your secret store is unlocked, then retry.",
            "a failed write must be visible, never 'Key saved'"
        );
        assert!(!page.key_status_text().contains("sk-secret"));

        // A later successful save clears the failure.
        page.set_key_saved("openai", true);
        assert_eq!(
            page.key_status_text(),
            "Key saved in your system's secret store."
        );
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

    // --- Transforms CRUD (tempdir-backed; the real config dir is
    // never touched) ---

    fn page_with_store() -> (AiConfigApp, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ai-transforms.toml");
        (
            AiConfigApp::with_transforms_path(Config::default(), path),
            dir,
        )
    }

    fn create(page: &mut AiConfigApp, name: &str, prompt: &str) -> String {
        page.begin_create_transform();
        page.editing = Some(EditingTransform {
            id: None,
            name: name.into(),
            prompt: prompt.into(),
        });
        assert!(page.commit_transform(), "a full draft commits");
        page.transforms().last().unwrap().id.clone()
    }

    #[test]
    fn creating_a_transform_persists_the_transform_table() {
        let (mut page, dir) = page_with_store();
        assert!(
            page.transforms().is_empty(),
            "the empty store invites creation"
        );
        let id = create(
            &mut page,
            "Extract text",
            "Extract the text inside this image",
        );
        assert_eq!(page.transforms().len(), 1);
        assert_eq!(page.transforms()[0].id, id);

        // The file on disk carries the [[transform]] table.
        let body = std::fs::read_to_string(dir.path().join("ai-transforms.toml")).unwrap();
        assert!(body.contains("[[transform]]"), "body:\n{body}");
        assert!(body.contains("name = \"Extract text\""));
        assert!(body.contains("prompt = \"Extract the text inside this image\""));

        // A fresh page over the same path sees it (the dialog's
        // fresh-snapshot semantics).
        let reloaded = TransformStore::load_from(&dir.path().join("ai-transforms.toml"));
        assert_eq!(reloaded.all(), page.transforms());
    }

    #[test]
    fn editing_persists_name_and_prompt() {
        let (mut page, _dir) = page_with_store();
        let id = create(&mut page, "OCR", "read it");
        page.begin_edit_transform(&id);
        page.editing = Some(EditingTransform {
            id: Some(id.clone()),
            name: "Extract text".into(),
            prompt: "Extract the text inside this image".into(),
        });
        assert!(page.commit_transform());
        assert_eq!(page.transforms().len(), 1, "an edit never duplicates");
        assert_eq!(page.transforms()[0].name, "Extract text");
        assert_eq!(
            page.transforms()[0].prompt,
            "Extract the text inside this image"
        );
    }

    #[test]
    fn deleting_removes_exactly_that_id() {
        let (mut page, _dir) = page_with_store();
        let a = create(&mut page, "A", "a");
        let b = create(&mut page, "B", "b");
        page.delete_transform(&a);
        assert_eq!(page.transforms().len(), 1);
        assert_eq!(page.transforms()[0].id, b, "the survivor is intact");
        page.delete_transform(&a);
        assert_eq!(page.transforms().len(), 1, "re-delete is a no-op");
    }

    #[test]
    fn reordering_swaps_neighbours_and_no_ops_at_the_ends() {
        let (mut page, _dir) = page_with_store();
        let a = create(&mut page, "A", "p");
        let b = create(&mut page, "B", "p");
        let c = create(&mut page, "C", "p");

        page.move_transform(&c, -1);
        assert_eq!(
            ids(&page),
            vec![a.clone(), c.clone(), b.clone()],
            "C moves up over B"
        );
        page.move_transform(&a, -1);
        assert_eq!(
            ids(&page),
            vec![a.clone(), c.clone(), b.clone()],
            "top no-op"
        );
        page.move_transform(&b, 1);
        assert_eq!(ids(&page), vec![a, c, b], "bottom no-op (order unchanged)");
    }

    fn ids(page: &AiConfigApp) -> Vec<String> {
        page.transforms().iter().map(|t| t.id.clone()).collect()
    }

    #[test]
    fn an_empty_draft_never_commits() {
        let (mut page, _dir) = page_with_store();
        page.begin_create_transform();
        page.editing = Some(EditingTransform {
            id: None,
            name: "  ".into(),
            prompt: "filled".into(),
        });
        assert!(!page.commit_transform(), "a nameless draft stays open");
        assert!(page.editing.is_some(), "the draft survives for input");
        assert!(page.transforms().is_empty());
    }

    // --- Typing harness: real egui frames into the page ---

    fn page_frame(ctx: &egui::Context, page: &mut AiConfigApp, events: Vec<egui::Event>) -> bool {
        let raw = egui::RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1024.0, 768.0),
            )),
            ..egui::RawInput::default()
        };
        let mut escape = false;
        let _ = ctx.run_ui(raw, |ui| {
            let surface = crate::topbar::Surface::Settings;
            let enable = crate::topbar::enable_for(surface);
            let _ = crate::topbar::show(ui, surface, None, enable, false, false);
            let panel = egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(crate::theme::palette::PAPER_CREAM))
                .show(ui, |ui| AiConfigPage::show(ui, page, ui.input(|i| i.time)));
            escape = panel.inner;
        });
        escape
    }

    fn click_events(at: egui::Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(at),
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }

    /// The page pins the density bump: the centered 680 px column,
    /// full-row model field, `CONTROL_H` pills, and transform rows
    /// whose controls sit inside a ≥ 32 px card at `TYPE_MD`.
    #[test]
    fn aiconfig_page_pins_the_density_bump() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::widgets::set_animations(&ctx, false);
        let (mut page, _dir) = page_with_store();
        let _id = create(&mut page, "Extract text", "Extract the text");
        // One more pass than `page_frame`'s shape: every render uses
        // the same harness (topbar + central panel) so response
        // rects and painted shapes describe the same layout.
        let shapes_frame = |page: &mut AiConfigApp| {
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1024.0, 768.0),
                )),
                ..egui::RawInput::default()
            };
            ctx.run_ui(raw, |ui| {
                let surface = crate::topbar::Surface::Settings;
                let enable = crate::topbar::enable_for(surface);
                let _ = crate::topbar::show(ui, surface, None, enable, false, false);
                let panel = egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(crate::theme::palette::PAPER_CREAM))
                    .show(ui, |ui| AiConfigPage::show(ui, page, ui.input(|i| i.time)));
                let _ = panel.inner;
            })
            .shapes
        };
        let _ = shapes_frame(&mut page);
        let _ = shapes_frame(&mut page);
        let out = shapes_frame(&mut page);

        // The Model section card spans the centered 680 px column
        // (minus its gutters).
        let section = ctx
            .read_response(egui::Id::new(("ai-settings-section", "Model")))
            .expect("the Model section header registers")
            .rect;
        assert!(
            (section.width() - (680.0 - 28.0)).abs() < 12.0,
            "the section spans the 680 px column: {}",
            section.width()
        );
        assert!(
            section.left() > 100.0 && section.right() < 900.0,
            "the column centers in the 1024 px window: {section:?}"
        );

        // The model field fills its row (no fixed 240 px width).
        let model = ctx
            .read_response(egui::Id::new("ai-model"))
            .expect("the model field registers")
            .rect;
        assert!(
            model.width() > 400.0,
            "the model field fills the row: {}",
            model.width()
        );

        // Pills at the control height.
        let test = ctx
            .read_response(egui::Id::new("ai-test-connection"))
            .expect("the test pill registers")
            .rect;
        assert!(
            (test.height() - theme::CONTROL_H).abs() < 0.5,
            "test pill height {}",
            test.height()
        );

        // The transform row: a PAPER_DIM card clearing 32 px with
        // its enabled (forest) pills inside — the controls ride the
        // row, never a stray line below the card.
        let pills: Vec<egui::Rect> = out
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Rect(r) if r.fill == theme::palette::FOREST_MID => Some(r.rect),
                _ => None,
            })
            .collect();
        let row_card = out
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Rect(r) if r.fill == theme::palette::PAPER_DIM => Some(r.rect),
                _ => None,
            })
            .find(|card| {
                // A row card clears 32 px and carries the whole
                // control strip (the provider well's selected
                // segment also paints forest, but rides alone).
                card.height() >= 32.0
                    && pills.iter().filter(|p| card.contains_rect(**p)).count() >= 4
            })
            .expect("the transform row's card paints with its pills");
        assert!(
            row_card.height() >= 32.0,
            "the transform row clears 32 px: {}",
            row_card.height()
        );
        let inside = pills.iter().filter(|p| row_card.contains_rect(**p)).count();
        assert!(inside >= 4, "the row's controls sit inside the card");

        let name_size = out.iter().find_map(|cs| match &cs.shape {
            egui::Shape::Text(t) if t.galley.job.text == "Extract text" => {
                t.galley.job.sections.first().map(|s| s.format.font_id.size)
            }
            _ => None,
        });
        assert_eq!(
            name_size,
            Some(theme::TYPE_MD),
            "transform names paint at TYPE_MD"
        );
    }

    #[test]
    fn typing_lands_in_the_transform_prompt_field() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let (mut page, _dir) = page_with_store();
        page.begin_create_transform();
        for _ in 0..3 {
            let _ = page_frame(&ctx, &mut page, vec![]);
        }
        let field = ctx
            .read_response(egui::Id::new("ai-transform-prompt"))
            .expect("the prompt field registers");
        let _ = page_frame(&ctx, &mut page, click_events(field.rect.center()));
        let _ = page_frame(&ctx, &mut page, vec![]);
        let _ = page_frame(&ctx, &mut page, vec![egui::Event::Text("ocr it".into())]);
        assert_eq!(
            page.editing.as_ref().expect("draft open").prompt,
            "ocr it",
            "a focused prompt field accepts typed text"
        );
    }

    #[test]
    fn char_by_char_typing_survives_quiet_frames() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let (mut page, _dir) = page_with_store();
        page.begin_create_transform();
        let mut t = 0.0;
        let mut step = |ctx: &egui::Context, page: &mut AiConfigApp, events: Vec<egui::Event>| {
            t += 1.0 / 60.0;
            let raw = egui::RawInput {
                time: Some(t),
                predicted_dt: 1.0 / 60.0,
                events,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1024.0, 768.0),
                )),
                ..egui::RawInput::default()
            };
            let _ = ctx.run_ui(raw, |ui| {
                let _ = AiConfigPage::show(ui, page, ui.input(|i| i.time));
            });
        };
        for _ in 0..30 {
            step(&ctx, &mut page, vec![]);
        }
        let field = ctx
            .read_response(egui::Id::new("ai-transform-prompt"))
            .expect("the prompt field registers")
            .rect;
        step(&ctx, &mut page, click_events(field.center()));
        // The quiet frames are the regression: the section body used
        // to run through an invisible measuring probe that
        // surrendered the field's keyboard focus every frame, so a
        // click kept focus for at most the next frame and typing
        // landed nothing (the section-render fix runs the body once).
        for _ in 0..5 {
            step(&ctx, &mut page, vec![]);
        }
        assert_eq!(
            ctx.memory(egui::Memory::focused),
            Some(egui::Id::new("ai-transform-prompt")),
            "a field inside a section holds focus across quiet frames"
        );
        for ch in ["o", "c", "r"] {
            step(&ctx, &mut page, vec![egui::Event::Text(ch.into())]);
        }
        assert_eq!(
            page.editing.as_ref().expect("draft open").prompt,
            "ocr",
            "every typed char lands, not just the first"
        );
    }

    // --- the vision-probe verdict banner (vision-validate T5) ---

    /// A config carrying a persisted probe verdict for exactly its
    /// working `(model, base-url)`.
    fn config_with_probe(verdict: &str, probed_model: &str, probed_base: &str) -> Config {
        let mut config = Config::default();
        config.ai.provider = "custom".into();
        config.ai.model = "llama3".into();
        config.ai.base_url = "http://x/v1".into();
        config.ai.probe_model = Some(probed_model.into());
        config.ai.probe_base_url = Some(probed_base.into());
        config.ai.probe_verdict = Some(verdict.into());
        config.ai.probe_date = Some(1);
        config
    }

    /// One rendered page's painted texts (the banner-visibility
    /// assertion seam).
    fn page_texts(ctx: &egui::Context, page: &mut AiConfigApp) -> Vec<String> {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1024.0, 768.0),
            )),
            ..egui::RawInput::default()
        };
        ctx.run_ui(raw, |ui| {
            let _ = AiConfigPage::show(ui, page, ui.input(|i| i.time));
        })
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::Shape::Text(t) => Some(t.galley.job.text.clone()),
            _ => None,
        })
        .collect()
    }

    /// The banner shows the three verdict states with the spec's
    /// copy — and only while the persisted triple matches the
    /// working settings.
    #[test]
    fn banner_renders_verdict_states_only_on_triple_match() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::widgets::set_animations(&ctx, false);

        // Unsupported + match: berry copy names the model.
        let mut page = AiConfigApp::new(config_with_probe(
            "vision-unsupported",
            "llama3",
            "http://x/v1",
        ));
        let texts = page_texts(&ctx, &mut page);
        assert!(
            texts
                .iter()
                .any(|t| t.contains("may not accept image input")),
            "the unsupported copy paints: {texts:?}"
        );
        assert!(
            texts.iter().any(|t| t.contains("llama3")),
            "the banner names the model: {texts:?}"
        );

        // Ok + match: the muted sees-images line.
        let mut page = AiConfigApp::new(config_with_probe("vision-ok", "llama3", "http://x/v1"));
        let texts = page_texts(&ctx, &mut page);
        assert!(
            texts.iter().any(|t| t.contains("Model sees images")),
            "the ok copy paints: {texts:?}"
        );

        // Unknown: no banner claim either way.
        let mut page =
            AiConfigApp::new(config_with_probe("vision-unknown", "llama3", "http://x/v1"));
        let texts = page_texts(&ctx, &mut page);
        assert!(
            !texts
                .iter()
                .any(|t| t.contains("may not accept") || t.contains("sees images")),
            "unknown stays silent: {texts:?}"
        );

        // Triple mismatch (model differs): hidden.
        let mut page = AiConfigApp::new(config_with_probe(
            "vision-unsupported",
            "other-model",
            "http://x/v1",
        ));
        let texts = page_texts(&ctx, &mut page);
        assert!(
            !texts.iter().any(|t| t.contains("may not accept")),
            "a stale verdict never badges a different model: {texts:?}"
        );

        // Triple mismatch (base differs): hidden.
        let mut page = AiConfigApp::new(config_with_probe(
            "vision-unsupported",
            "llama3",
            "http://y/v1",
        ));
        let texts = page_texts(&ctx, &mut page);
        assert!(
            !texts.iter().any(|t| t.contains("may not accept")),
            "a stale verdict never badges a different endpoint: {texts:?}"
        );
    }

    /// An arriving verdict updates the banner live (the page-poll
    /// handoff), even when the page opened before any probe existed.
    #[test]
    fn banner_updates_from_a_live_arrival() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::widgets::set_animations(&ctx, false);
        let mut page = AiConfigApp::new(Config::default());
        page.config.ai.provider = "custom".into();
        page.config.ai.model = "llama3".into();
        page.config.ai.base_url = "http://x/v1".into();
        let texts = page_texts(&ctx, &mut page);
        assert!(
            !texts.iter().any(|t| t.contains("may not accept")),
            "no verdict yet, no banner"
        );
        page.note_probe_outcome(&flintshot_core::ProbeOutcome {
            verdict: flintshot_core::ProbeVerdict::VisionUnsupported,
            model: "llama3".into(),
            base_url: "http://x/v1".into(),
        });
        let texts = page_texts(&ctx, &mut page);
        assert!(
            texts
                .iter()
                .any(|t| t.contains("may not accept image input")),
            "the arrival paints the banner: {texts:?}"
        );
    }

    /// The banner's re-test affordance re-fires the manual probe
    /// (the same Test connection path).
    #[test]
    fn banner_re_test_refires_the_probe() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::widgets::set_animations(&ctx, false);
        let mut page = AiConfigApp::new(config_with_probe(
            "vision-unsupported",
            "llama3",
            "http://x/v1",
        ));
        let _ = page_texts(&ctx, &mut page);
        let retest = ctx
            .read_response(egui::Id::new("ai-probe-retest"))
            .expect("the Check again pill registers under the banner")
            .rect;
        let _ = page_texts(&ctx, &mut page); // warm the id
        let _ = page_texts(&ctx, &mut page);
        let _ = page_frame(&ctx, &mut page, click_events(retest.center()));
        assert_eq!(
            page.drain_action(),
            Some(AiConfigAction::TestConnection {
                provider: "custom".into(),
                model: "llama3".into(),
                base_url: "http://x/v1".into(),
            }),
            "re-test queues the same action as Test connection"
        );
        assert_eq!(page.test, Some(TestStatus::Pending));
    }

    /// `TestStatus::Passed` means the model described the image
    /// (probe v1), not merely that the wire shape was accepted.
    #[test]
    fn passed_copy_pins_the_described_image_meaning() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::widgets::set_animations(&ctx, false);
        let mut page = AiConfigApp::new(Config::default());
        page.set_test_result(Ok(String::new()));
        let texts = page_texts(&ctx, &mut page);
        assert!(
            texts.iter().any(|t| t.contains("described the image")),
            "the pass line says what passed: {texts:?}"
        );
    }
}
