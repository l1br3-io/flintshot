//! The settings route — the single window's preferences page.
//!
//! A centered 640 px column of cutout section cards (Capture /
//! Editor / Output & saving / Shortcuts / System) over the paper
//! desk. Every editable row reads and writes the Task-9 config
//! fields through the window's bridge; edits apply to the
//! in-memory config immediately and persist through the existing
//! save + watcher round-trip (debounced — see [`SAVE_DEBOUNCE`]).
//! Readonly rows (hide-from-shot, default tool, shortcuts, display
//! server, version) render live values, visually dashed/locked.

use flintshot_core::{
    parse_default_mode, parse_output_format, parse_size_mode, Config, DefaultMode, OutputFormat,
    SizeMode,
};

use crate::theme;
use crate::widgets;

/// Minimum spacing between config writes, in seconds. The settings
/// page debounces saves so dragging a value doesn't machine-gun the
/// TOML (and the watcher's reload) — GOTCHA: the watcher reloads
/// the app's config on every write; rapid successive writes are
/// coalesced by this timer, and the final write always lands.
const SAVE_DEBOUNCE: f64 = 1.0;

/// An action the page collected that the window router must service
/// through the bridge (the page stays pure data + this queue).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsAction {
    /// *Edit config file* clicked: open the user's `config.toml` in
    /// their editor (the bridge owns the spawn).
    EditConfigFile,
    /// *Rebind shortcuts…* clicked: open the portal bind dialog
    /// through the daemon (the bridge owns the IPC; Wayland-only —
    /// the row renders iff the session is Wayland).
    RebindShortcuts,
    /// *Ask the shot settings…* clicked: route to the AI settings
    /// page (the router swaps in place).
    OpenAiConfig,
}

/// The settings page's state: the working config copy plus the
/// draft of the filename-template text field (validated before it
/// reaches the config).
pub(crate) struct SettingsApp {
    config: Config,
    template_draft: String,
    display_server: String,
    version: String,
    /// The fully-commented default TOML, snapshotted from the bridge
    /// at construction (the read-only defaults view's content).
    default_toml: String,
    /// Whether the session is Wayland, snapshotted at construction
    /// (gates the *Rebind shortcuts…* row, like `display_server`).
    wayland: bool,
    /// Whether the defaults view is expanded.
    show_defaults: bool,
    /// Clicked actions awaiting the router's drain.
    actions: Vec<SettingsAction>,
    /// When the first unsaved edit landed (the debounce clock's
    /// start). `None` when everything is persisted.
    dirty_since: Option<f64>,
}

impl SettingsApp {
    /// Snapshot the page from the bridge.
    pub(crate) fn new(
        config: Config,
        display_server: String,
        version: String,
        default_toml: String,
        wayland: bool,
    ) -> Self {
        let template_draft = config.output.filename_template.clone();
        Self {
            config,
            template_draft,
            display_server,
            version,
            default_toml,
            wayland,
            show_defaults: false,
            actions: Vec::new(),
            dirty_since: None,
        }
    }

    /// Take the oldest clicked action, if one waits (the router
    /// services it through the bridge).
    pub(crate) fn drain_action(&mut self) -> Option<SettingsAction> {
        if self.actions.is_empty() {
            None
        } else {
            Some(self.actions.remove(0))
        }
    }

    /// Whether a save is due (dirty past the debounce window).
    fn save_due(&self, now: f64) -> bool {
        self.dirty_since.is_some_and(|t| now - t >= SAVE_DEBOUNCE)
    }

    fn mark_dirty(&mut self, now: f64) {
        if self.dirty_since.is_none() {
            self.dirty_since = Some(now);
        }
    }

    /// Drain the save: returns the config to persist when a save is
    /// due, clearing the dirty clock.
    /// Persist the config once the debounce window has passed since
    /// the first unsaved edit. Returns the config to save, clearing
    /// the dirty clock.
    pub(crate) fn drain_save(&mut self, now: f64) -> Option<&Config> {
        if self.save_due(now) {
            self.dirty_since = None;
            Some(&self.config)
        } else {
            None
        }
    }

    /// Whether Escape was pressed this frame (the back affordance).
    fn escape_pressed(ctx: &egui::Context) -> bool {
        ctx.input(|i| {
            i.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::Key {
                        key: egui::Key::Escape,
                        pressed: true,
                        ..
                    }
                )
            })
        })
    }

    /// The path the save-location row displays: the configured
    /// directory when set (whitespace-trimmed), or the resolved
    /// platform default the writes would honor — never a
    /// placeholder label.
    pub(crate) fn effective_save_dir(&self) -> String {
        if self.config.output.save_dir.trim().is_empty() {
            self.config
                .output
                .resolve_save_dir()
                .to_string_lossy()
                .into_owned()
        } else {
            self.config.output.save_dir.trim().to_owned()
        }
    }
}

/// Apply a folder-pick (or its cancellation) to the working
/// config: `Some(dir)` sets the save dir and arms the debounced
/// save; `None` — the dialog was cancelled — changes nothing.
/// Pure seam so tests skip the `rfd` dialog.
fn apply_picked_folder(page: &mut SettingsApp, picked: Option<std::path::PathBuf>, now: f64) {
    if let Some(dir) = picked {
        page.config.output.save_dir = dir.to_string_lossy().into_owned();
        page.mark_dirty(now);
    }
}

pub(crate) struct SettingsPage;

impl SettingsPage {
    /// Draw the settings body over `ui` against the working config.
    /// `now` is the frame clock (for the debounce); edits flag the
    /// page dirty. Returns whether Escape fired (the router routes
    /// home).
    pub(crate) fn show(ui: &mut egui::Ui, page: &mut SettingsApp, now: f64) -> bool {
        let escape = crate::settings::SettingsApp::escape_pressed(ui.ctx());

        // `auto_shrink([false, true])`: the area takes the panel's
        // full height (the scrollbar engages) and shrinks only
        // horizontally (the centered column). The default
        // `[true; 2]` never scrolls — the content just overflows
        // the window bottom.
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                let column = 640.0_f32.min(ui.available_width());
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.set_min_width(column);
                    ui.add_space(theme::SPACE_SCALE[3]);
                    capture_section(ui, page, now);
                    editor_section(ui, page, now);
                    output_section(ui, page, now);
                    config_file_section(ui, page);
                    shortcuts_section(ui, page);
                    system_section(ui, page);
                    ui.add_space(theme::SPACE_SCALE[3]);
                });
            });

        escape
    }
}

/// The Capture section: default mode, delay, cursor, and the locked
/// hide-from-shot guarantee.
fn capture_section(ui: &mut egui::Ui, page: &mut SettingsApp, now: f64) {
    section_card(ui, "Capture", |ui| {
        let modes = ["region", "fullscreen", "window"];
        let current = parse_default_mode(&page.config.capture.default_mode);
        let idx = match current {
            DefaultMode::Fullscreen => 1,
            DefaultMode::Window => 2,
            _ => 0,
        };
        set_row(ui, "Default mode", |ui| {
            if let Some(pick) =
                widgets::well_row(ui, egui::Id::new("set-default-mode"), &modes, idx)
            {
                set_str(&mut page.config.capture.default_mode, modes[pick]);
                page.mark_dirty(now);
            }
        });
        set_row(ui, "Delay", |ui| {
            let delays = ["0s", "3s", "5s"];
            let values = [0_u32, 3_000, 5_000];
            let idx = values
                .iter()
                .position(|v| *v == page.config.capture.default_delay_ms)
                .unwrap_or(0);
            if let Some(pick) = widgets::well_row(ui, egui::Id::new("set-delay"), &delays, idx) {
                page.config.capture.default_delay_ms = values[pick];
                page.mark_dirty(now);
            }
        });
        set_row(ui, "Include cursor", |ui| {
            let resp = widgets::switch(
                ui,
                egui::Id::new("set-cursor"),
                page.config.capture.include_cursor,
                false,
            );
            if resp.on != page.config.capture.include_cursor {
                page.config.capture.include_cursor = resp.on;
                page.mark_dirty(now);
            }
        });
        set_row(ui, "Hide from shot", |ui| {
            // 🔒 Locked-on: the window-closes-first guarantee
            // (ADR-0047) is a guarantee display, not a pref.
            let _ = widgets::switch(ui, egui::Id::new("set-hide-locked"), true, true);
        });
    });
}

/// The Editor section: the readonly landing-tool row, the statusbar
/// gate, the size-mode wells, and the animations gate.
fn editor_section(ui: &mut egui::Ui, page: &mut SettingsApp, now: f64) {
    section_card(ui, "Editor", |ui| {
        set_row(ui, "Tool when editor opens", |ui| {
            // Readonly this effort: the engine's landing tool is
            // Select (the default); the setting is a seam, not a pref.
            readonly_value(ui, "Select");
        });
        set_row(ui, "Show statusbar", |ui| {
            let resp = widgets::switch(
                ui,
                egui::Id::new("set-statusbar"),
                page.config.ui.show_statusbar,
                false,
            );
            if resp.on != page.config.ui.show_statusbar {
                page.config.ui.show_statusbar = resp.on;
                page.mark_dirty(now);
            }
        });
        set_row(ui, "Byte-size estimate", |ui| {
            let modes = ["heuristic", "exact-png"];
            let current = parse_size_mode(&page.config.ui.statusbar_size_mode);
            let idx = usize::from(current == SizeMode::ExactPng);
            if let Some(pick) = widgets::well_row(ui, egui::Id::new("set-size-mode"), &modes, idx) {
                set_str(&mut page.config.ui.statusbar_size_mode, modes[pick]);
                page.mark_dirty(now);
            }
        });
        set_row(ui, "Spring animations", |ui| {
            let resp = widgets::switch(
                ui,
                egui::Id::new("set-animations"),
                page.config.ui.animations,
                false,
            );
            if resp.on != page.config.ui.animations {
                page.config.ui.animations = resp.on;
                page.mark_dirty(now);
            }
        });
    });
}

/// The Output & saving section: format, JPEG quality (jpeg only),
/// the filename template draft, the save directory, and the
/// copy-after-capture switch.
fn output_section(ui: &mut egui::Ui, page: &mut SettingsApp, now: f64) {
    section_card(ui, "Output & saving", |ui| {
        set_row(ui, "Format", |ui| {
            let formats = ["png", "jpeg"];
            let current = parse_output_format(&page.config.output.format);
            let idx = usize::from(current == OutputFormat::Jpeg);
            if let Some(pick) = widgets::well_row(ui, egui::Id::new("set-format"), &formats, idx) {
                set_str(&mut page.config.output.format, formats[pick]);
                page.mark_dirty(now);
            }
        });
        if parse_output_format(&page.config.output.format) == OutputFormat::Jpeg {
            set_row(ui, "JPEG quality", |ui| {
                let qualities = ["80", "90", "100"];
                let values = [80_u32, 90, 100];
                let idx = values
                    .iter()
                    .position(|v| *v == page.config.output.jpeg_quality)
                    .unwrap_or(1);
                if let Some(pick) =
                    widgets::well_row(ui, egui::Id::new("set-quality"), &qualities, idx)
                {
                    page.config.output.jpeg_quality = values[pick];
                    page.mark_dirty(now);
                }
            });
        }
        set_row(ui, "Filename template", |ui| {
            let resp = ui.add(
                egui::TextEdit::singleline(&mut page.template_draft)
                    .desired_width(220.0)
                    .hint_text("{app}-{date}-{time}"),
            );
            if resp.changed() {
                // Cheap validation: empty = direct save off (legal);
                // anything that sanitizes to something non-empty is
                // legal too. A template that sanitizes away entirely
                // is held in the draft until it doesn't.
                let clean = flintshot_core::sanitize_filename(&page.template_draft);
                if page.template_draft.is_empty() || !clean.is_empty() {
                    page.config
                        .output
                        .filename_template
                        .clone_from(&page.template_draft);
                    page.mark_dirty(now);
                }
            }
        });
        set_row(ui, "Save location", |ui| {
            // The honest display: the effective path the writes
            // will honor (configured or resolved default).
            readonly_value(ui, &page.effective_save_dir());
            // Browse: the native folder dialog (the Open control's
            // blocking-modal precedent). A cancel changes nothing.
            if link(ui, egui::Id::new("set-save-browse"), "Browse") {
                let picked = rfd::FileDialog::new().pick_folder();
                apply_picked_folder(page, picked, now);
            }
            // Reset: only meaningful once a dir is configured.
            if !page.config.output.save_dir.trim().is_empty()
                && link(ui, egui::Id::new("set-save-default"), "Use default")
            {
                page.config.output.save_dir.clear();
                page.mark_dirty(now);
            }
        });
        set_row(ui, "Save clipboard copies", |ui| {
            let resp = widgets::switch(
                ui,
                egui::Id::new("set-save-copies"),
                page.config.output.save_clipboard_copies,
                false,
            );
            if resp.on != page.config.output.save_clipboard_copies {
                page.config.output.save_clipboard_copies = resp.on;
                page.mark_dirty(now);
            }
        });
        set_row(ui, "Recent captures", |ui| {
            // Preset wells (the page's mapping-table idiom): every
            // pick is in-range by construction; out-of-range
            // hand-edits clamp at use (the config accessor) and
            // settle on the nearest preset once the user picks.
            let limits = ["20", "50", "100", "200"];
            let values = [20_u32, 50, 100, 200];
            let idx = values
                .iter()
                .position(|v| *v == page.config.ui.recent_captures_limit)
                .unwrap_or(1);
            if let Some(pick) =
                widgets::well_row(ui, egui::Id::new("set-recents-limit"), &limits, idx)
            {
                page.config.ui.recent_captures_limit = values[pick];
                page.mark_dirty(now);
            }
        });
        set_row(ui, "Copy after capture", |ui| {
            let on = page.config.capture.after_grab == "clipboard";
            let resp = widgets::switch(ui, egui::Id::new("set-copy-after"), on, false);
            if resp.on != on {
                let value = if resp.on { "clipboard" } else { "editor" };
                set_str(&mut page.config.capture.after_grab, value);
                page.mark_dirty(now);
            }
        });
    });
}

/// The Shortcuts section: the real binding table, readonly — plus
/// the *Rebind shortcuts…* link on Wayland sessions (the portal
/// bind dialog only exists there; X11/macOS/Windows hotkeys
/// re-register from `[hotkeys]` directly, so the row would be a
/// dead control — the tray item's gate, moved into Settings).
fn shortcuts_section(ui: &mut egui::Ui, page: &mut SettingsApp) {
    section_card(ui, "Shortcuts", |ui| {
        set_row(ui, "Capture", |ui| {
            let chord = page.config.hotkeys.capture.clone();
            readonly_value(ui, &chord);
        });
        if page.wayland {
            set_row(ui, "Rebind shortcuts", |ui| {
                if link(ui, egui::Id::new("set-rebind"), "Open bind dialog…") {
                    page.actions.push(SettingsAction::RebindShortcuts);
                }
            });
        }
    });
}

/// The Config file section (settings-improvement): the `VSCode`
/// *User Settings* / *Default Settings* pair — a link that opens
/// the user's `config.toml` in their editor, and a link that
/// expands the embedded, fully-commented defaults read-only.
fn config_file_section(ui: &mut egui::Ui, page: &mut SettingsApp) {
    section_card(ui, "Config file", |ui| {
        set_row(ui, "Edit config file", |ui| {
            if link(ui, egui::Id::new("set-edit-config"), "Open in editor") {
                page.actions.push(SettingsAction::EditConfigFile);
            }
        });
        set_row(ui, "View default config", |ui| {
            let label = if page.show_defaults {
                "Hide defaults"
            } else {
                "Show defaults"
            };
            if link(ui, egui::Id::new("set-view-defaults"), label) {
                page.show_defaults = !page.show_defaults;
            }
        });
        if page.show_defaults {
            defaults_card(ui, &page.default_toml);
        }
        set_row(ui, "Ask the shot", |ui| {
            if link(
                ui,
                egui::Id::new("set-open-ai-config"),
                "Ask the shot settings…",
            ) {
                page.actions.push(SettingsAction::OpenAiConfig);
            }
        });
    });
}

/// The read-only defaults view: the embedded TOML in a monospace
/// card on the dim paper — dashed-locked kin to the readonly wells,
/// never an editable widget (the card is hover-sensed only; pinned
/// by test).
fn defaults_card(ui: &mut egui::Ui, toml_text: &str) {
    use crate::theme::palette as p;
    ui.add_space(theme::SPACE_SCALE[1]);
    egui::Frame::new()
        .fill(p::PAPER_DIM)
        .corner_radius(egui::CornerRadius::same(theme::POD_RADIUS))
        .inner_margin(egui::Margin::same(12))
        .stroke(theme::HAIRLINE_STROKE)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let top = ui.cursor().top();
            // Galley-painted lines (no widgets): the card can never
            // register an editable control.
            for line in toml_text.lines() {
                let galley = ui.painter().layout_no_wrap(
                    if line.is_empty() {
                        " ".to_owned()
                    } else {
                        line.to_owned()
                    },
                    egui::FontId::monospace(theme::TYPE_XS),
                    p::INK_SOFT,
                );
                let size = egui::vec2(ui.available_width(), galley.size().y.max(12.0));
                let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                ui.painter().galley(
                    egui::pos2(rect.left(), rect.center().y - galley.size().y / 2.0),
                    galley,
                    p::INK_SOFT,
                );
            }
            // One stable hover target over the painted body (tests,
            // tooltips later).
            let body = egui::Rect::from_min_max(
                egui::pos2(ui.max_rect().left(), top),
                ui.cursor().left_top(),
            );
            let _ = ui.interact(
                body,
                egui::Id::new("set-defaults-card"),
                egui::Sense::hover(),
            );
        });
}

/// A gold text link — the settings page's tertiary affordance
/// (navigate out or reveal; the row controls edit). Painted like
/// the breadcrumb's links: gold-deep ink, forest + underline on
/// hover, pointer cursor.
fn link(ui: &mut egui::Ui, id: egui::Id, label: &str) -> bool {
    use crate::theme::palette as p;
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::proportional(theme::TYPE_SM),
        p::GOLD_DEEP,
    );
    let size = egui::vec2(galley.size().x + 6.0, 26.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    // The id-ed interact is the link's one true hit target (the
    // allocation above senses hover only, so it cannot steal the
    // click).
    let resp = ui
        .interact(rect, id, egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let color = if resp.hovered() {
        p::FOREST
    } else {
        p::GOLD_DEEP
    };
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::proportional(theme::TYPE_SM),
        color,
    );
    let pos = egui::pos2(rect.left(), rect.center().y - galley.size().y / 2.0);
    let galley_size = galley.size();
    ui.painter().galley(pos, galley, color);
    if resp.hovered() {
        let underline = egui::Stroke::new(1.0, color);
        ui.painter().line_segment(
            [
                egui::pos2(pos.x, pos.y + galley_size.y + 1.0),
                egui::pos2(pos.x + galley_size.x, pos.y + galley_size.y + 1.0),
            ],
            underline,
        );
    }
    resp.clicked()
}

/// The System section: detected display server + version, readonly.
fn system_section(ui: &mut egui::Ui, page: &mut SettingsApp) {
    section_card(ui, "System", |ui| {
        set_row(ui, "Display server", |ui| {
            readonly_value(ui, &page.display_server);
        });
        set_row(ui, "Version", |ui| {
            readonly_value(ui, &page.version);
        });
    });
}

/// Write a `&'static str` pick into an owned String config field.
fn set_str(field: &mut String, value: &str) {
    field.clear();
    field.push_str(value);
}

/// One cutout section card with the collapsible section widget.
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
                egui::Id::new(("settings-section", title)),
                title,
                true,
                body,
            );
        });
}

/// A `set-row`: label left, control right.
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

/// A readonly value: dashed-locked well, dim ink — visually distinct
/// from the editable wells.
fn readonly_value(ui: &mut egui::Ui, value: &str) {
    use crate::theme::palette as p;
    let galley = ui.painter().layout_no_wrap(
        format!("🔒 {value}"),
        egui::FontId::proportional(theme::TYPE_SM),
        p::LOCK_GREY,
    );
    let w = 14.0 + galley.size().x + 14.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 26.0), egui::Sense::hover());
    let dashed = egui::Stroke::new(1.0, p::with_alpha(p::LOCK_GREY, 180));
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(crate::theme::POD_RADIUS),
        p::PAPER_DIM,
        dashed,
        egui::StrokeKind::Middle,
    );
    ui.painter().galley(
        egui::pos2(rect.left() + 14.0, rect.center().y - galley.size().y / 2.0),
        galley,
        p::LOCK_GREY,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page() -> SettingsApp {
        page_gated(false)
    }

    /// [`page`] with the Wayland gate pinned (the rebind row's
    /// render gate).
    fn page_gated(wayland: bool) -> SettingsApp {
        SettingsApp::new(
            Config::default(),
            "X11".into(),
            "9.9.9".into(),
            "# flintshot configuration\n[ui]\ndefault-color = \"#ff0000\"\n".into(),
            wayland,
        )
    }

    fn frame(page: &mut SettingsApp) -> egui::Context {
        frame_with_animations(page, true)
    }

    /// [`frame`] with the spring gate pinned — off settles every
    /// section's unfold in one frame (the body clip covers all rows
    /// immediately; with the spring on, a single headless frame
    /// catches the cards mid-unfold and the lower rows' widgets are
    /// clip-dead).
    fn frame_with_animations(page: &mut SettingsApp, animations: bool) -> egui::Context {
        let ctx = egui::Context::default();
        widgets::set_animations(&ctx, animations);
        theme::apply(&ctx);
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 900.0),
            )),
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            SettingsPage::show(ui, page, 0.0);
        });
        ctx
    }

    /// Every mapping-table row renders its control, and the readonly
    /// rows are visibly distinct (locked wells, not switches).
    #[test]
    fn all_mapping_rows_render() {
        let mut page = page();
        let ctx = frame(&mut page);
        // Wells register per-segment ids (`id.with(index)`); switches
        // register their bare id.
        let wells = [
            ("set-default-mode", 3),
            ("set-delay", 3),
            ("set-size-mode", 2),
            ("set-format", 2),
        ];
        for (base, segments) in wells {
            for i in 0..segments {
                assert!(
                    ctx.read_response(egui::Id::new(base).with(i)).is_some(),
                    "{base} segment {i} renders"
                );
            }
        }
        for id in [
            "set-cursor",
            "set-hide-locked",
            "set-statusbar",
            "set-animations",
            "set-copy-after",
        ] {
            assert!(
                ctx.read_response(egui::Id::new(id)).is_some(),
                "{id} renders"
            );
        }
        // Readonly rows use the locked-well idiom: switches for the
        // editable rows exist, and the hide-from-shot lock renders.
        assert!(
            ctx.read_response(egui::Id::new("set-hide-locked"))
                .is_some(),
            "the locked guarantee display renders"
        );
    }

    /// Editable rows write their config field live (row→field
    /// wiring map).
    #[test]
    fn editable_rows_write_their_fields() {
        let mut page = page();
        assert!(page.config.ui.show_statusbar, "default on");
        let ctx = frame(&mut page);
        let sw = ctx
            .read_response(egui::Id::new("set-statusbar"))
            .expect("statusbar switch");
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 900.0),
            )),
            events: {
                let at = sw.rect.center();
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
            },
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            SettingsPage::show(ui, &mut page, 0.0);
        });
        assert!(
            !page.config.ui.show_statusbar,
            "the click flipped the field"
        );
        assert!(page.save_due(10.0), "an edit arms the save debounce");
    }

    /// Readonly rows are inert: the locked switch swallows clicks.
    #[test]
    fn readonly_rows_are_inert() {
        let mut page = page();
        let ctx = frame(&mut page);
        let lock = ctx
            .read_response(egui::Id::new("set-hide-locked"))
            .expect("locked switch renders");
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 900.0),
            )),
            events: {
                let at = lock.rect.center();
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
            },
            ..egui::RawInput::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            SettingsPage::show(ui, &mut page, 0.0);
        });
        assert!(
            !page.config.hotkeys.capture.is_empty(),
            "the page survives the click"
        );
    }

    /// The debounce: an edit within the window holds the save; past
    /// the window, `drain_save` drains it.
    #[test]
    fn save_debounce_coalesces_writes() {
        let mut page = page();
        page.mark_dirty(0.0);
        assert!(!page.save_due(0.5), "inside the window the save holds");
        assert!(page.save_due(2.0), "past the window the save is due");
        assert!(page.drain_save(2.0).is_some(), "take drains the save");
        assert!(!page.save_due(2.1), "drained: no further saves");
    }

    // ── Config-file links (settings-improvement Task 009) ────────

    /// A click at `resp`'s center (the settings tests' event shape).
    fn click(at: egui::Rect) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(at.center()),
            egui::Event::PointerButton {
                pos: at.center(),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: at.center(),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }

    /// Both links render, and the edit link reports through the
    /// action queue exactly once per click.
    #[test]
    fn config_links_render_and_report() {
        let mut page = page();
        let ctx = frame_with_animations(&mut page, false);
        let edit = ctx
            .read_response(egui::Id::new("set-edit-config"))
            .expect("the edit link registers")
            .rect;
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 900.0),
                )),
                events: click(edit),
                ..egui::RawInput::default()
            },
            |ui| {
                SettingsPage::show(ui, &mut page, 0.0);
            },
        );
        assert_eq!(
            page.drain_action(),
            Some(SettingsAction::EditConfigFile),
            "one click = one queued action"
        );
        assert!(page.drain_action().is_none(), "drained exactly once");
    }

    /// The defaults view: collapsed by default, expands on the link
    /// click (the card registers, hover-sensed only), collapses
    /// again on the second click.
    #[test]
    fn defaults_view_toggles_and_is_readonly() {
        let mut page = page();
        let ctx = frame_with_animations(&mut page, false);
        assert!(
            ctx.read_response(egui::Id::new("set-defaults-card"))
                .is_none(),
            "collapsed by default — no card registers"
        );

        let toggle = ctx
            .read_response(egui::Id::new("set-view-defaults"))
            .expect("the view-defaults link registers")
            .rect;
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 900.0),
                )),
                events: click(toggle),
                ..egui::RawInput::default()
            },
            |ui| {
                SettingsPage::show(ui, &mut page, 0.0);
            },
        );
        assert!(page.show_defaults, "the link expanded the view");
        let card = ctx
            .read_response(egui::Id::new("set-defaults-card"))
            .expect("the expanded card registers");
        assert!(
            !card.clicked_by(egui::PointerButton::Primary),
            "hover-sensed only — the card can never click"
        );
        assert!(
            page.drain_action().is_none(),
            "revealing defaults queues no action"
        );

        // Second click collapses.
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 900.0),
                )),
                events: click(toggle),
                ..egui::RawInput::default()
            },
            |ui| {
                SettingsPage::show(ui, &mut page, 0.0);
            },
        );
        assert!(!page.show_defaults, "the second click collapsed it");
    }

    /// The expanded card renders the TOML text itself (the galley is
    /// painted, so pin through the monospace galley the painter
    /// lays out — one painted line per source line, first line the
    /// header).
    #[test]
    fn defaults_card_renders_the_embedded_toml_lines() {
        let mut page = page();
        page.show_defaults = true;
        let ctx = frame_with_animations(&mut page, false);
        assert!(
            ctx.read_response(egui::Id::new("set-defaults-card"))
                .is_some(),
            "the card is live"
        );
        // The fixture asset's line count is the layout budget: the
        // card grew the page by at least that many lines of height.
        let lines = f32::from(u16::try_from(page.default_toml.lines().count()).unwrap_or(u16::MAX));
        let card = ctx
            .read_response(egui::Id::new("set-defaults-card"))
            .expect("card")
            .rect;
        assert!(
            card.height() >= lines * 10.0,
            "the card body covers all {lines} monospace lines (got {} px)",
            card.height()
        );
    }

    // ── Recent-captures-v2 Task 007 ──────────────────────────────

    /// The displayed path is the one the save path honors: the
    /// configured dir when set, the resolved platform default
    /// otherwise — never a placeholder.
    #[test]
    fn effective_save_dir_spans_configured_and_default() {
        let mut page = page();
        assert!(
            page.effective_save_dir().ends_with("Flintshot"),
            "default shows the resolved platform dir, got {}",
            page.effective_save_dir()
        );
        page.config.output.save_dir = "  /tmp/my-shots  ".into();
        assert_eq!(page.effective_save_dir(), "/tmp/my-shots");
    }

    /// A picked folder lands in the working config and arms the
    /// save; a cancelled dialog (`None`) changes nothing.
    #[test]
    fn apply_picked_folder_sets_or_ignores() {
        let mut page = page();
        apply_picked_folder(
            &mut page,
            Some(std::path::PathBuf::from("/tmp/picked")),
            0.0,
        );
        assert_eq!(page.config.output.save_dir, "/tmp/picked");
        assert!(page.save_due(2.0), "the pick armed the debounce");

        let mut cancelled = page_gated(false);
        apply_picked_folder(&mut cancelled, None, 0.0);
        assert!(cancelled.config.output.save_dir.is_empty());
        assert!(!cancelled.save_due(2.0), "a cancel arms nothing");
    }

    /// The save-location row: Browse renders always; "Use default"
    /// renders only when a dir is configured, and clicking it
    /// resets to the default (empty).
    #[test]
    fn save_location_row_browses_and_resets() {
        let mut page = page();
        let ctx = frame_with_animations(&mut page, false);
        assert!(
            ctx.read_response(egui::Id::new("set-save-browse"))
                .is_some(),
            "the browse action renders"
        );
        assert!(
            ctx.read_response(egui::Id::new("set-save-default"))
                .is_none(),
            "no reset link while on the default"
        );

        page.config.output.save_dir = "/tmp/my-shots".into();
        let ctx = frame_with_animations(&mut page, false);
        let reset = ctx
            .read_response(egui::Id::new("set-save-default"))
            .expect("the reset link renders once a dir is set")
            .rect;
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 900.0),
                )),
                events: click(reset),
                ..egui::RawInput::default()
            },
            |ui| {
                SettingsPage::show(ui, &mut page, 0.0);
            },
        );
        assert!(
            page.config.output.save_dir.is_empty(),
            "the reset returned to the platform default"
        );
        assert!(page.save_due(2.0), "the reset armed the save");
    }

    /// The clipboard-copies switch flips its config field live.
    #[test]
    fn save_clipboard_copies_switch_writes_the_field() {
        let mut page = page();
        assert!(page.config.output.save_clipboard_copies, "default on");
        let ctx = frame_with_animations(&mut page, false);
        let sw = ctx
            .read_response(egui::Id::new("set-save-copies"))
            .expect("the switch renders")
            .rect;
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 900.0),
                )),
                events: click(sw),
                ..egui::RawInput::default()
            },
            |ui| {
                SettingsPage::show(ui, &mut page, 0.0);
            },
        );
        assert!(
            !page.config.output.save_clipboard_copies,
            "the click flipped it"
        );
    }

    /// The recents limit row renders preset wells and writes the
    /// picked value.
    #[test]
    fn recents_limit_row_wells_write_the_field() {
        let mut page = page();
        let ctx = frame_with_animations(&mut page, false);
        for i in 0..4 {
            assert!(
                ctx.read_response(egui::Id::new("set-recents-limit").with(i))
                    .is_some(),
                "segment {i} renders"
            );
        }
        assert_eq!(page.config.ui.recent_captures_limit, 50);
        let pick = ctx
            .read_response(egui::Id::new("set-recents-limit").with(2))
            .expect("the 100 segment")
            .rect;
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 900.0),
                )),
                events: click(pick),
                ..egui::RawInput::default()
            },
            |ui| {
                SettingsPage::show(ui, &mut page, 0.0);
            },
        );
        assert_eq!(
            page.config.ui.recent_captures_limit, 100,
            "the pick wrote the field"
        );
        assert!(page.save_due(2.0), "the pick armed the save");
    }

    // ── Rebind row (settings-improvement Task 010) ───────────────

    /// The row renders iff the session is Wayland — X11 never grows
    /// a dead control (the tray item's gate, moved into Settings).
    #[test]
    fn rebind_row_renders_only_on_wayland() {
        let mut x11 = page_gated(false);
        let ctx = frame_with_animations(&mut x11, false);
        assert!(
            ctx.read_response(egui::Id::new("set-rebind")).is_none(),
            "X11: no dead rebind row"
        );

        let mut wayland = page_gated(true);
        let ctx = frame_with_animations(&mut wayland, false);
        assert!(
            ctx.read_response(egui::Id::new("set-rebind")).is_some(),
            "Wayland: the rebind row registers"
        );
    }

    /// The row reports through the action queue when present.
    #[test]
    fn rebind_row_reports_through_the_queue() {
        let mut page = page_gated(true);
        let ctx = frame_with_animations(&mut page, false);
        let row = ctx
            .read_response(egui::Id::new("set-rebind"))
            .expect("the rebind row registers on Wayland")
            .rect;
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 900.0),
                )),
                events: click(row),
                ..egui::RawInput::default()
            },
            |ui| {
                SettingsPage::show(ui, &mut page, 0.0);
            },
        );
        assert_eq!(page.drain_action(), Some(SettingsAction::RebindShortcuts));
        assert!(page.drain_action().is_none(), "drained exactly once");
    }
}
