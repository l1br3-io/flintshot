//! The AI settings page (`Route::AiConfig`) — Ask-the-shot's config
//! home.
//!
//! The settings page's grammar: labelled cutout section cards in a
//! centered 680 px column. Two sections — **Model** (provider preset,
//! model id + the **Browse…** pill over the endpoint's advertised
//! model list, base-URL override, the masked Replace-key affordance,
//! Test connection) and **Transforms** (the saved-transform CRUD,
//! Task 011) — over the page's config snapshot + save-debounce
//! lifecycle. The page stays pure data + an action queue: the
//! router services keyring writes, test connections, and model-list
//! fetches through the page bridge (keyring writes land immediately
//! on commit, config fields persist on the debounce).
//!
//! The Browse lifecycle (ai-config-ux): the pill's click is the only
//! thing that fires the `GET {base}/models` fetch — never page open,
//! provider switch, or save. The fetched list keys to the working
//! `(provider, base-url)` pair (the probe banner's triple-match
//! idiom) so a dropdown never offers another endpoint's models; the
//! painted dropdown (filter field, `CONTROL_H` rows, an
//! "accepts images" mark where the endpoint's metadata said so,
//! Refresh) fills the model field as if typed.
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
    /// The Model row's **Browse** pill fired: fetch the endpoint's
    /// model list (`GET {base}/models`). The click is the user
    /// action — nothing fires on open, switch, or save.
    ListModels {
        /// Provider id (whose keyring entry auths the fetch).
        provider: String,
        /// The effective base URL (override or preset).
        base_url: String,
    },
}

/// The Browse affordance's fetch lifecycle, keyed to the
/// `(provider, effective base URL)` it belongs to — every consumer
/// gates on that pair so a stale list never serves a different
/// endpoint (the probe banner's triple-match idiom).
struct ModelsState {
    /// The provider the fetch was keyed to.
    provider: String,
    /// The effective base URL the fetch hit.
    base_url: String,
    /// Where the fetch stands.
    status: ModelsStatus,
    /// The dropdown's filter draft (T6's field).
    filter: String,
    /// Whether the painted dropdown is open.
    open: bool,
}

/// One fetch's outcome state.
enum ModelsStatus {
    /// In flight (the loading line).
    Pending,
    /// The endpoint answered (the dropdown's rows).
    Arrived(Vec<flintshot_core::ModelListing>),
    /// The mapped failure copy (typing stays the path).
    Failed(String),
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
    /// The Browse affordance's fetch state (T5+T6): pending /
    /// arrived / failed, keyed to the working pair. Dropped on any
    /// provider switch; arrivals for a stale pair drop it too.
    models: Option<ModelsState>,
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
            models: None,
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

    /// The Browse pill fired. Arms one fetch for the working pair —
    /// a pending fetch for the same pair swallows the re-click (the
    /// bridge's one-flight policy), an arrived list for it toggles
    /// the dropdown, anything else (or nothing yet) queues a fresh
    /// fetch. Only meaningful when an endpoint exists.
    pub(crate) fn browse_models(&mut self) {
        let base_url = self.effective_base_url().to_string();
        if base_url.is_empty() {
            return;
        }
        let provider = self.config.ai.provider.clone();
        if let Some(state) = self.models.as_ref() {
            if state.provider == provider && state.base_url == base_url {
                match state.status {
                    ModelsStatus::Pending => return, // one flight per pair
                    ModelsStatus::Arrived(_) => {
                        let Some(state) = self.models.as_mut() else {
                            return;
                        };
                        state.open = !state.open;
                        return;
                    }
                    ModelsStatus::Failed(_) => {} // retry path re-fires
                }
            }
        }
        self.fetch_models();
    }

    /// Queue a fresh fetch for the working pair (the Browse pill's
    /// first click and the dropdown's Refresh share this core). The
    /// previous state — stale or failed — is replaced wholesale.
    fn fetch_models(&mut self) {
        let base_url = self.effective_base_url().to_string();
        if base_url.is_empty() {
            return;
        }
        let provider = self.config.ai.provider.clone();
        self.models = Some(ModelsState {
            provider: provider.clone(),
            base_url,
            status: ModelsStatus::Pending,
            filter: String::new(),
            open: false,
        });
        self.actions.push(AiConfigAction::ListModels {
            provider,
            base_url: self.effective_base_url().to_string(),
        });
    }

    /// The dropdown's Refresh fired: a fresh fetch even though a list
    /// already arrived (endpoints gain models; the list is never
    /// stale forever). The dropdown closes while the fetch runs and
    /// reopens on arrival.
    pub(crate) fn refresh_models(&mut self) {
        self.fetch_models();
    }

    /// A dropdown row was picked: the id lands in the model field as
    /// if typed (the debounce persists it) and the dropdown closes.
    pub(crate) fn select_model(&mut self, id: &str, now: f64) {
        self.config.ai.model = id.to_string();
        self.mark_dirty(now);
        if let Some(state) = self.models.as_mut() {
            state.open = false;
        }
    }

    /// Whether the painted dropdown is engaged this frame (an arrived
    /// list for the working pair, open). The page's Escape guard:
    /// an Esc that closes the dropdown must not also leave the page.
    pub(crate) fn dropdown_open(&self) -> bool {
        self.models.as_ref().is_some_and(|state| {
            state.open
                && state.provider == self.config.ai.provider
                && state.base_url == self.effective_base_url()
                && matches!(state.status, ModelsStatus::Arrived(_))
        })
    }

    /// The bridge answered a models fetch. Arrivals for anything but
    /// the working pair drop the state entirely (a superseded
    /// endpoint never populates the dropdown); the working pair's
    /// answer lands — arrived opens the dropdown, failed keeps one
    /// actionable line.
    pub(crate) fn note_models_arrival(&mut self, arrival: &crate::shell::ModelsArrival) {
        let working = (self.config.ai.provider.as_str(), self.effective_base_url());
        if (arrival.provider.as_str(), arrival.base_url.as_str()) != working {
            self.models = None; // stale — drop, never serve
            return;
        }
        let Some(state) = self.models.as_mut() else {
            return; // an answer nobody asked for
        };
        match arrival.result.clone() {
            Ok(models) => {
                state.status = ModelsStatus::Arrived(models);
                state.filter.clear();
                state.open = true;
            }
            Err(copy) => {
                state.status = ModelsStatus::Failed(copy);
                state.open = false;
            }
        }
    }

    /// The Model row's status line under the field: the loading line
    /// while pending, the failure copy after a refused fetch.
    /// `None` when quiet (nothing asked, or the list arrived — the
    /// dropdown speaks then) or when the state is stale.
    pub(crate) fn models_status_text(&self) -> Option<String> {
        let state = self.models.as_ref()?;
        if state.provider != self.config.ai.provider || state.base_url != self.effective_base_url()
        {
            return None;
        }
        match &state.status {
            ModelsStatus::Pending => {
                Some(format!("Loading models from {}…", host_of(&state.base_url)))
            }
            ModelsStatus::Failed(copy) => Some(copy.clone()),
            ModelsStatus::Arrived(_) => None,
        }
    }

    /// Test seam: the arrived list for the working pair, if any
    /// (assertion accessor).
    #[cfg(test)]
    pub(crate) fn models_arrived(&self) -> Option<Vec<flintshot_core::ModelListing>> {
        let state = self.models.as_ref()?;
        if state.provider != self.config.ai.provider || state.base_url != self.effective_base_url()
        {
            return None;
        }
        match &state.status {
            ModelsStatus::Arrived(models) => Some(models.clone()),
            _ => None,
        }
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
        // Neither does a fetched model list — the dropdown must never
        // offer another endpoint's models.
        self.models = None;
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
        // An Esc that closes the models dropdown must not also leave
        // the page — the dropdown swallows the first press.
        let dropdown_open = page.dropdown_open();
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
        escape && !dropdown_open
    }
}

/// The Model section: provider pills, model id (+ the Browse
/// affordance over the endpoint's advertised list), base-URL
/// override, the key affordance, Test connection.
fn model_section(ui: &mut egui::Ui, page: &mut AiConfigApp, now: f64) {
    section_card(ui, "Model", |ui| {
        provider_row(ui, page, now);
        // The dropdown anchors to the model field — capture its rect
        // out of the row for the popup below.
        let mut field_rect = None;
        set_row(ui, "Model", |ui| {
            // set_row lays the control right-to-left: reserve the
            // Browse pill first so it lands rightmost, then the field
            // fills the rest (the key row's idiom).
            let armed = !page.effective_base_url().is_empty();
            if ghost_pill(ui, egui::Id::new("ai-model-browse"), "Browse…", armed) {
                page.browse_models();
            }
            let hint = model_hint(page).to_owned();
            let response = ui.add(
                egui::TextEdit::singleline(&mut page.config.ai.model)
                    .id(egui::Id::new("ai-model"))
                    .hint_text(hint)
                    .desired_width(ui.available_width()),
            );
            field_rect = Some(response.rect);
            if response.changed() {
                page.mark_dirty(now);
            }
        });
        models_dropdown(ui, page, field_rect, now);
        if let Some(line) = page.models_status_text() {
            status_line(ui, &line);
        }
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

/// The endpoint's host + path, scheme stripped — the loading line's
/// legible form (`https://api.openai.com/v1` reads
/// `api.openai.com/v1`).
fn host_of(base_url: &str) -> &str {
    let after_scheme = base_url
        .split_once("://")
        .map_or(base_url, |(_, rest)| rest);
    after_scheme.trim_end_matches('/')
}

/// The painted check vector — a painted glyph, not a font codepoint
/// (the tofu ban; icons are painted glyphs in this crate). Shared by
/// the probe banner's sees-images line and the dropdown's
/// accepts-images flag.
fn check_glyph(ui: &egui::Ui, center: egui::Pos2, color: egui::Color32) {
    ui.painter().line_segment(
        [
            egui::pos2(center.x - 4.0, center.y + 0.5),
            egui::pos2(center.x - 1.0, center.y + 3.5),
        ],
        egui::Stroke::new(1.5, color),
    );
    ui.painter().line_segment(
        [
            egui::pos2(center.x - 1.0, center.y + 3.5),
            egui::pos2(center.x + 4.5, center.y - 3.5),
        ],
        egui::Stroke::new(1.5, color),
    );
}

/// The arrived model list as a painted dropdown under the model
/// field — the page's one memorable element (ai-config-ux). A filter
/// field over `CONTROL_H` painted rows (the current model rides the
/// forest-selected fill; `accepts images` flags ride the rows where
/// the endpoint's metadata said so), a Refresh affordance in the
/// footer. Selecting a row fills the field as if typed; the popup's
/// own machinery closes on outside click and Escape.
fn models_dropdown(
    ui: &mut egui::Ui,
    page: &mut AiConfigApp,
    anchor: Option<egui::Rect>,
    now: f64,
) {
    let Some(anchor) = anchor else {
        return;
    };
    if !page.dropdown_open() {
        return;
    }
    let mut open = true; // `dropdown_open` gated above; the popup writes back closes
                         // Hoist what the closure borrows: the filter draft, the row
                         // snapshot, the current model, and the frame's intents.
    let (filter_draft, rows) = match page.models.as_ref() {
        Some(state) => match &state.status {
            ModelsStatus::Arrived(models) => (state.filter.clone(), models.clone()),
            _ => return,
        },
        None => return,
    };
    let mut filter = filter_draft;
    let current_model = page.config.ai.model.trim().to_string();
    let mut selected: Option<String> = None;
    let mut refresh = false;

    let popup = egui::Popup::new(
        egui::Id::new("ai-models-popover"),
        ui.ctx().clone(),
        anchor,
        ui.layer_id(),
    )
    .open_bool(&mut open)
    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
    .gap(4.0)
    .width(anchor.width())
    .frame(
        egui::Frame::new()
            .fill(theme::palette::PAPER_CREAM)
            .corner_radius(egui::CornerRadius::same(theme::CUTOUT_RADIUS))
            .inner_margin(egui::Margin::same(8))
            .stroke(theme::HAIRLINE_STROKE)
            .shadow(theme::SHADOW_POP),
    );
    let _ = popup.show(|ui| {
        ui.set_width(ui.available_width());
        ui.add(
            egui::TextEdit::singleline(&mut filter)
                .id(egui::Id::new("ai-models-filter"))
                .hint_text("Filter models…")
                .desired_width(ui.available_width()),
        );
        ui.add_space(theme::SPACE_SCALE[0]);
        let needle = filter.trim().to_lowercase();
        egui::ScrollArea::vertical()
            .max_height(5.5 * theme::CONTROL_H)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let mut matched = 0;
                for listing in &rows {
                    if !needle.is_empty() && !listing.id.to_lowercase().contains(&needle) {
                        continue;
                    }
                    matched += 1;
                    if model_row(ui, listing, &current_model) {
                        selected = Some(listing.id.clone());
                    }
                }
                if matched == 0 {
                    ui.label(
                        egui::RichText::new(format!("No models match {}", filter.trim()))
                            .size(theme::TYPE_SM)
                            .color(theme::palette::INK_SOFT),
                    );
                }
            });
        ui.add_space(theme::SPACE_SCALE[0]);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ghost_pill(ui, egui::Id::new("ai-models-refresh"), "Refresh", true) {
                refresh = true;
            }
        });
    });

    // Write the frame's outcomes back (the popup may have closed on
    // outside click or Escape; a row may have been picked).
    if let Some(state) = page.models.as_mut() {
        state.filter = filter;
        state.open = open;
    }
    if let Some(id) = selected {
        page.select_model(&id, now);
    }
    if refresh {
        page.refresh_models();
    }
}

/// One painted dropdown row: the id at `TYPE_MD` left, the
/// `accepts images` check + flag right where the endpoint's metadata
/// said so (`Some(false)` and `None` stay silent — absence is never
/// "no"). The current model rides the forest-selected fill. Returns
/// whether the row was clicked.
fn model_row(ui: &mut egui::Ui, listing: &flintshot_core::ModelListing, current: &str) -> bool {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, theme::CONTROL_H), egui::Sense::hover());
    let resp = ui.interact(
        rect,
        egui::Id::new(("ai-model-row", listing.id.clone())),
        egui::Sense::click(),
    );
    let is_current = listing.id == current;
    if is_current {
        ui.painter().rect(
            rect,
            egui::CornerRadius::same(theme::POD_RADIUS),
            theme::palette::FOREST_MID,
            egui::Stroke::NONE,
            egui::StrokeKind::Middle,
        );
    } else if resp.hovered() {
        ui.painter().rect(
            rect,
            egui::CornerRadius::same(theme::POD_RADIUS),
            theme::palette::PAPER_DIM,
            egui::Stroke::NONE,
            egui::StrokeKind::Middle,
        );
    }
    let pad = 10.0;
    let id_galley = ui.painter().layout_no_wrap(
        listing.id.clone(),
        egui::FontId::proportional(theme::TYPE_MD),
        if is_current {
            theme::palette::CREAM_TEXT
        } else {
            theme::palette::INK
        },
    );
    ui.painter().galley(
        egui::pos2(
            rect.left() + pad,
            rect.center().y - id_galley.size().y / 2.0,
        ),
        id_galley,
        if is_current {
            theme::palette::CREAM_TEXT
        } else {
            theme::palette::INK
        },
    );
    if listing.vision == Some(true) {
        let flag_galley = ui.painter().layout_no_wrap(
            "accepts images".to_owned(),
            egui::FontId::proportional(theme::TYPE_SM),
            theme::palette::INK_SOFT,
        );
        let flag_x = rect.right() - pad - flag_galley.size().x - 14.0;
        ui.painter().galley(
            egui::pos2(flag_x, rect.center().y - flag_galley.size().y / 2.0),
            flag_galley,
            theme::palette::INK_SOFT,
        );
        check_glyph(
            ui,
            egui::pos2(rect.right() - pad - 6.0, rect.center().y),
            theme::palette::FOREST_MID,
        );
    }
    resp.clicked()
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
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Model sees images")
                        .size(theme::TYPE_SM)
                        .color(theme::palette::INK_SOFT),
                );
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                check_glyph(ui, rect.center(), theme::palette::FOREST_MID);
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

    // --- the Browse fetch lifecycle (ai-config-ux T5) ---

    fn custom_page(base_url: &str) -> AiConfigApp {
        let mut page = AiConfigApp::new(Config::default());
        page.config.ai.provider = "custom".into();
        page.config.ai.base_url = base_url.into();
        page
    }

    fn arrival(
        base_url: &str,
        models: Vec<flintshot_core::ModelListing>,
    ) -> crate::shell::ModelsArrival {
        crate::shell::ModelsArrival {
            provider: "custom".into(),
            base_url: base_url.into(),
            result: Ok(models),
        }
    }

    /// The Browse click queues the fetch keyed to the working pair
    /// and shows the loading line; a re-click while pending swallows
    /// (one flight per pair — the bridge replaces, the page need not
    /// spam it).
    #[test]
    fn browse_click_queues_the_fetch_and_shows_loading() {
        let mut page = custom_page("http://x/v1");
        assert!(
            page.models_status_text().is_none(),
            "no click, no status, no network"
        );
        page.browse_models();
        assert_eq!(
            page.drain_action(),
            Some(AiConfigAction::ListModels {
                provider: "custom".into(),
                base_url: "http://x/v1".into()
            })
        );
        assert!(
            page.models_status_text()
                .is_some_and(|t| t.contains("Loading models")),
            "the loading line names the work: {:?}",
            page.models_status_text()
        );
        page.browse_models();
        assert!(
            page.drain_action().is_none(),
            "a pending fetch swallows re-clicks"
        );
    }

    /// Without an endpoint there is nothing to browse — custom with
    /// an empty base URL never queues a fetch (and presets always
    /// carry one, so only this corner is inert).
    #[test]
    fn browse_without_an_endpoint_fires_nothing() {
        let mut page = custom_page("");
        page.browse_models();
        assert!(page.drain_action().is_none(), "nothing fires unarmed");
    }

    /// An arriving list lands only for the pair it was fired on: an
    /// answer for another endpoint drops the stale state entirely
    /// (never populates anything), the working pair's answer arrives.
    #[test]
    fn an_arrival_opens_the_list_only_for_the_working_pair() {
        let mut page = custom_page("http://x/v1");
        page.browse_models();
        let _ = page.drain_action();

        page.note_models_arrival(&arrival(
            "http://elsewhere/v1",
            vec![flintshot_core::ModelListing {
                id: "not-mine".into(),
                vision: None,
            }],
        ));
        assert!(
            page.models_status_text().is_none(),
            "a superseded answer drops the state — no phantom loading line"
        );

        page.browse_models(); // re-arm after the drop
        let _ = page.drain_action();
        page.note_models_arrival(&arrival(
            "http://x/v1",
            vec![flintshot_core::ModelListing {
                id: "llama3.2-vision".into(),
                vision: Some(true),
            }],
        ));
        assert!(page.models_status_text().is_none(), "arrived is quiet");
        assert_eq!(
            page.models_arrived().map(|m| m.len()),
            Some(1),
            "the working pair's answer lands"
        );
    }

    /// A failed fetch keeps one actionable line and never touches the
    /// typed model — typing stays the first-class path.
    #[test]
    fn failed_arrival_keeps_the_copy_and_the_typed_model() {
        let mut page = custom_page("http://x/v1");
        page.config.ai.model = "my-typed-model".into();
        page.browse_models();
        let _ = page.drain_action();
        page.note_models_arrival(&crate::shell::ModelsArrival {
            provider: "custom".into(),
            base_url: "http://x/v1".into(),
            result: Err(
                "Couldn't reach the provider — check the base URL and network, then retry.".into(),
            ),
        });
        assert!(
            page.models_status_text()
                .is_some_and(|t| t.contains("Couldn't reach the provider")),
            "the failure copy shows: {:?}",
            page.models_status_text()
        );
        assert_eq!(
            page.config.ai.model, "my-typed-model",
            "a failed fetch never mutates the field"
        );
    }

    /// A provider (or endpoint) switch drops any models state — the
    /// dropdown must never offer another endpoint's models.
    #[test]
    fn provider_switch_drops_the_models_state() {
        let mut page = custom_page("http://x/v1");
        page.browse_models();
        let _ = page.drain_action();
        page.note_models_arrival(&arrival(
            "http://x/v1",
            vec![flintshot_core::ModelListing {
                id: "llama3".into(),
                vision: None,
            }],
        ));
        assert_eq!(page.models_arrived().map(|m| m.len()), Some(1));
        page.select_provider("xai", 2.0);
        assert!(
            page.models_arrived().is_none(),
            "the switch drops the list (xai's base URL differs)"
        );
    }

    // --- the painted dropdown (ai-config-ux T6) ---

    fn arrived_page() -> AiConfigApp {
        let mut page = custom_page("http://x/v1");
        page.browse_models();
        let _ = page.drain_action();
        page.note_models_arrival(&arrival(
            "http://x/v1",
            vec![
                flintshot_core::ModelListing {
                    id: "llama3.2-vision".into(),
                    vision: Some(true),
                },
                flintshot_core::ModelListing {
                    id: "zephyr".into(),
                    vision: None,
                },
                flintshot_core::ModelListing {
                    id: "qwen2.5".into(),
                    vision: Some(false),
                },
            ],
        ));
        page
    }

    /// The arrived dropdown paints its rows (ids at `TYPE_MD`, the
    /// vision flag only where metadata said yes — Some(false) and
    /// None stay silent), and selecting a row fills the model field,
    /// marks the page dirty, and closes the dropdown.
    #[test]
    fn dropdown_paints_rows_and_selecting_fills_the_field() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::widgets::set_animations(&ctx, false);
        let mut page = arrived_page();

        let texts = page_texts(&ctx, &mut page);
        assert!(
            texts.iter().any(|t| t.contains("llama3.2-vision")),
            "the ids paint: {texts:?}"
        );
        assert!(
            texts.iter().any(|t| t.contains("accepts images")),
            "the vision flag paints where metadata said yes: {texts:?}"
        );
        assert_eq!(
            texts
                .iter()
                .filter(|t| t.contains("accepts images"))
                .count(),
            1,
            "Some(false) and None stay silent"
        );

        // The selected row's id at TYPE_MD (the token-system pin).
        let row = ctx
            .read_response(egui::Id::new(("ai-model-row", "llama3.2-vision")))
            .expect("the row registers")
            .rect;
        assert!(
            (row.height() - theme::CONTROL_H).abs() < 0.5,
            "rows ride the control height: {}",
            row.height()
        );

        // Click the row: the field fills, the page goes dirty, the
        // dropdown closes.
        let _ = page_frame(&ctx, &mut page, click_events(row.center()));
        assert_eq!(page.config.ai.model, "llama3.2-vision");
        page.mark_dirty(0.0);
        let saved = page.drain_save(2.0).expect("a selection is an edit");
        assert_eq!(saved.ai.model, "llama3.2-vision");
        let texts = page_texts(&ctx, &mut page);
        assert!(
            !texts.iter().any(|t| t.contains("zephyr")),
            "the dropdown closed on select: {texts:?}"
        );
    }

    /// The filter field narrows the rows (case-insensitive), and a
    /// filter that matches nothing invites rather than blanks.
    #[test]
    fn dropdown_filter_narrows_rows_and_empty_matches_invite() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::widgets::set_animations(&ctx, false);
        let mut page = arrived_page();
        let _ = page_texts(&ctx, &mut page);
        let filter = ctx
            .read_response(egui::Id::new("ai-models-filter"))
            .expect("the filter field registers")
            .rect;
        let _ = page_frame(&ctx, &mut page, click_events(filter.center()));
        let _ = page_frame(&ctx, &mut page, vec![egui::Event::Text("LLAMA".into())]);

        let texts = page_texts(&ctx, &mut page);
        assert!(
            texts.iter().any(|t| t.contains("llama3.2-vision")),
            "the case-insensitive match stays"
        );
        assert!(
            !texts.iter().any(|t| t == "zephyr"),
            "non-matches hide: {texts:?}"
        );

        let _ = page_frame(&ctx, &mut page, vec![egui::Event::Text("zzz".into())]);
        let texts = page_texts(&ctx, &mut page);
        assert!(
            texts.iter().any(|t| t.contains("No models match")),
            "the empty state invites: {texts:?}"
        );
    }

    /// The dropdown's Refresh re-fires the fetch (the loading line
    /// returns, the rows go) — the arrived list is never stale
    /// forever.
    #[test]
    fn dropdown_refresh_refires_the_fetch() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::widgets::set_animations(&ctx, false);
        let mut page = arrived_page();
        let _ = page_texts(&ctx, &mut page);
        let refresh = ctx
            .read_response(egui::Id::new("ai-models-refresh"))
            .expect("the Refresh pill registers")
            .rect;
        let _ = page_frame(&ctx, &mut page, click_events(refresh.center()));
        assert_eq!(
            page.drain_action(),
            Some(AiConfigAction::ListModels {
                provider: "custom".into(),
                base_url: "http://x/v1".into()
            }),
            "refresh queues the same action as Browse"
        );
        assert!(
            page.models_status_text()
                .is_some_and(|t| t.contains("Loading models")),
            "the loading line returns while refreshing"
        );
    }

    /// Esc with the dropdown open closes the dropdown, not the page —
    /// the second press leaves (the cascade a modal-over-page should
    /// have).
    #[test]
    fn escape_with_the_dropdown_open_closes_only_the_dropdown() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::widgets::set_animations(&ctx, false);
        let mut page = arrived_page();
        let escape_frame = |ctx: &egui::Context, page: &mut AiConfigApp| {
            let raw = egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1024.0, 768.0),
                )),
                ..egui::RawInput::default()
            };
            let mut escaped = false;
            let _ = ctx.run_ui(raw, |ui| {
                escaped = AiConfigPage::show(ui, page, ui.input(|i| i.time));
            });
            escaped
        };
        let _ = page_texts(&ctx, &mut page); // the dropdown registers
        assert!(
            !escape_frame(&ctx, &mut page),
            "the first Esc closes the dropdown, not the page"
        );
        assert!(!page.dropdown_open(), "the popup machinery closed it");
        assert!(
            escape_frame(&ctx, &mut page),
            "the second Esc leaves the page"
        );
    }
}
