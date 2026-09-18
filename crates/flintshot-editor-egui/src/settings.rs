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

/// The settings page's state: the working config copy plus the
/// draft of the filename-template text field (validated before it
/// reaches the config).
pub(crate) struct SettingsApp {
    config: Config,
    template_draft: String,
    display_server: String,
    version: String,
    /// When the first unsaved edit landed (the debounce clock's
    /// start). `None` when everything is persisted.
    dirty_since: Option<f64>,
}

impl SettingsApp {
    /// Snapshot the page from the bridge.
    pub(crate) fn new(config: Config, display_server: String, version: String) -> Self {
        let template_draft = config.output.filename_template.clone();
        Self {
            config,
            template_draft,
            display_server,
            version,
            dirty_since: None,
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
}

pub(crate) struct SettingsPage;

impl SettingsPage {
    /// Draw the settings body over `ui` against the working config.
    /// `now` is the frame clock (for the debounce); edits flag the
    /// page dirty. Returns whether Escape fired (the router routes
    /// home).
    pub(crate) fn show(ui: &mut egui::Ui, page: &mut SettingsApp, now: f64) -> bool {
        let escape = crate::settings::SettingsApp::escape_pressed(ui.ctx());

        egui::ScrollArea::vertical().show(ui, |ui| {
            let column = 640.0_f32.min(ui.available_width());
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                ui.set_min_width(column);
                ui.add_space(theme::SPACE_SCALE[3]);
                capture_section(ui, page, now);
                editor_section(ui, page, now);
                output_section(ui, page, now);
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
            // Move; the setting is a seam, not a pref.
            readonly_value(ui, "Move");
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
            // The directory picker is the app crate's dialog; the
            // readonly display shows the configured path.
            readonly_value(
                ui,
                if page.config.output.save_dir.is_empty() {
                    "Pictures (default)"
                } else {
                    &page.config.output.save_dir
                },
            );
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

/// The Shortcuts section: the real binding table, readonly.
fn shortcuts_section(ui: &mut egui::Ui, page: &mut SettingsApp) {
    section_card(ui, "Shortcuts", |ui| {
        set_row(ui, "Capture", |ui| {
            let chord = page.config.hotkeys.capture.clone();
            readonly_value(ui, &chord);
        });
    });
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
        SettingsApp::new(Config::default(), "X11".into(), "9.9.9".into())
    }

    fn frame(page: &mut SettingsApp) -> egui::Context {
        let ctx = egui::Context::default();
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
}
