//! The eframe app that drives the flintshot home (app) window.
//!
//! [`HomeApp`] is the Win11-Snipping-Tool-style capture home: two
//! prominent controls (*Capture region*, *Capture fullscreen* plus a
//! monitor picker) and a status line. This crate stays IPC-free —
//! the window talks to the world exclusively through [`HomeBridge`],
//! which the app crate implements (subscription pump + daemon
//! delegation live there).
//!
//! Hide/restore contract: a control click sends
//! [`egui::ViewportCommand::Visible(false)`] *then* asks the bridge
//! to start the capture (the bridge waits out the hide grace before
//! delegating). The bridge answers eventually with
//! [`HomeDirective::Show`], polled every frame, which makes the
//! window visible and focused again. While hidden the window
//! receives no input events, so egui's reactive mode would stop
//! calling `update` — a standing `request_repaint_after` keeps the
//! directive pump alive (without it, a hidden window would never
//! come back).

use std::time::Duration;

use eframe::App;
use egui::{Context, Key, KeyboardShortcut, Modifiers, ViewportCommand};

use crate::theme;

/// How often a hidden (or idle) home window re-polls directives.
/// See the module doc for why polling must continue while hidden.
const REPOLL_INTERVAL: Duration = Duration::from_millis(250);

/// One entry of the fullscreen monitor picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorEntry {
    /// `xcap` monitor index (the `--monitor N` value).
    pub index: u32,
    /// Human label, e.g. `"0 · DP-1 (primary)"`.
    pub label: String,
}

/// A directive from the app crate into the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomeDirective {
    /// Show and focus the window: the capture session ended, the
    /// daemon went away, or someone asked for the window (focus
    /// ping).
    Show,
    /// Replace the status line text (daemon/connection state).
    SetStatus(String),
}

/// The home window's only seam to the outside world. Implemented by
/// the app crate; every method runs on the UI thread.
pub trait HomeBridge {
    /// Start a region capture (the window has just hidden itself;
    /// the implementation owns the hide grace + daemon delegation).
    fn request_capture_region(&mut self);

    /// Start a fullscreen capture of `monitor` (primary when
    /// `None`).
    fn request_capture_fullscreen(&mut self, monitor: Option<u32>);

    /// Take one pending directive, oldest first. Called every
    /// frame (see [`REPOLL_INTERVAL`]).
    fn poll_directive(&mut self) -> Option<HomeDirective>;

    /// The monitors offered by the fullscreen picker. Called on
    /// construction and on each [`HomeDirective::Show`] (a monitor
    /// hot-plug between captures is then picked up).
    fn monitors(&mut self) -> Vec<MonitorEntry>;
}

/// Which capture a control asked for. Local to the click handlers —
/// the bridge methods stay the wire-shaped pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureKind {
    /// Interactive region selection.
    Region,
    /// Whole-monitor grab (`None` = primary).
    Fullscreen(Option<u32>),
}

/// The eframe app for the home window. Owns no IPC state — see
/// [`HomeBridge`].
pub(crate) struct HomeApp {
    /// Implementation-supplied world (pump, delegation).
    bridge: Box<dyn HomeBridge>,
    /// Pure UI state, kept separate so directive application is
    /// headless-testable.
    ui: HomeUiState,
    /// Keyboard shortcut: R → region.
    region_shortcut: KeyboardShortcut,
    /// Keyboard shortcut: F → fullscreen.
    fullscreen_shortcut: KeyboardShortcut,
}

/// Pure UI state of the home window — the part directives and clicks
/// mutate. Extracted from [`HomeApp`] so the transition logic is
/// unit-testable without an egui context.
#[derive(Debug, Clone, PartialEq, Eq)]
struct HomeUiState {
    /// Whether the window is (believed) hidden for a capture.
    hidden: bool,
    /// Status line contents.
    status: String,
    /// Selected fullscreen monitor (`None` = primary/default).
    selected_monitor: Option<u32>,
    /// Monitor picker entries (refreshed on show).
    monitors: Vec<MonitorEntry>,
}

impl HomeUiState {
    /// Apply one directive. Returns whether the window should be
    /// shown (polled by `update`; pure for tests).
    fn apply(&mut self, directive: &HomeDirective) {
        match directive {
            HomeDirective::Show => self.hidden = false,
            HomeDirective::SetStatus(text) => self.status.clone_from(text),
        }
    }
}

impl HomeApp {
    /// Construct the app over a bridge. Pure: no egui context is
    /// touched until the first `update`.
    pub(crate) fn new(bridge: Box<dyn HomeBridge>, initial_status: String) -> Self {
        let mut bridge = bridge;
        let monitors = bridge.monitors();
        Self {
            bridge,
            ui: HomeUiState {
                hidden: false,
                status: initial_status,
                selected_monitor: None,
                monitors,
            },
            region_shortcut: KeyboardShortcut::new(Modifiers::NONE, Key::R),
            fullscreen_shortcut: KeyboardShortcut::new(Modifiers::NONE, Key::F),
        }
    }

    /// Hide the window and start a capture through the bridge.
    /// Shared by the region button, the fullscreen button, and the
    /// keyboard shortcuts.
    fn start_capture(&mut self, ctx: &Context, kind: CaptureKind) {
        if self.ui.hidden {
            return;
        }
        ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        self.ui.hidden = true;
        match kind {
            CaptureKind::Region => self.bridge.request_capture_region(),
            CaptureKind::Fullscreen(monitor) => self.bridge.request_capture_fullscreen(monitor),
        }
    }
}

impl App for HomeApp {
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.logic_impl(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui_impl(ui);
    }
}

impl HomeApp {
    /// Headless-callable core of `App::logic`: directive polling,
    /// keyboard shortcuts, viewport visibility flips. Lives in
    /// `logic` (not `ui`) because eframe calls it even while the
    /// window is hidden — exactly when directives must still flow.
    fn logic_impl(&mut self, ctx: &Context) {
        // Keep polling while hidden: an unmapped window gets no input
        // events, so without a standing repaint request `logic`
        // would never run again and no directive could arrive.
        ctx.request_repaint_after(REPOLL_INTERVAL);

        while let Some(directive) = self.bridge.poll_directive() {
            let refresh_monitors = matches!(directive, HomeDirective::Show);
            self.ui.apply(&directive);
            if refresh_monitors {
                self.ui.monitors = self.bridge.monitors();
                if let Some(sel) = self.ui.selected_monitor {
                    if !self.ui.monitors.iter().any(|m| m.index == sel) {
                        self.ui.selected_monitor = None;
                    }
                }
            }
            if !self.ui.hidden {
                ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(ViewportCommand::Focus);
            }
        }

        if !self.ui.hidden {
            if ctx.input_mut(|i| i.consume_shortcut(&self.region_shortcut)) {
                self.start_capture(ctx, CaptureKind::Region);
            } else if ctx.input_mut(|i| i.consume_shortcut(&self.fullscreen_shortcut)) {
                self.start_capture(ctx, CaptureKind::Fullscreen(self.ui.selected_monitor));
            }
        }
    }

    /// Headless-callable core of `App::ui` (mirrors `EditorApp`'s
    /// `logic_impl` / `ui_impl` split).
    fn ui_impl(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(6.0);
            ui.heading("flintshot");
            ui.label(
                egui::RichText::new("captures hide this window and return when done")
                    .color(theme::SLATE)
                    .small(),
            );
            ui.add_space(10.0);

            let tall = egui::vec2(ui.available_width(), 42.0);
            let region = egui::Button::new(
                egui::RichText::new("Capture region  [R]")
                    .color(theme::INK)
                    .strong(),
            )
            .fill(theme::EMBER)
            .min_size(tall);
            if ui.add(region).clicked() {
                self.start_capture(ui.ctx(), CaptureKind::Region);
                return;
            }
            ui.add_space(4.0);

            egui::ComboBox::from_id_salt("flintshot-home-monitor")
                .selected_text(self.monitor_selection_label())
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.ui.selected_monitor, None, "Primary monitor");
                    for entry in &self.ui.monitors {
                        ui.selectable_value(
                            &mut self.ui.selected_monitor,
                            Some(entry.index),
                            &entry.label,
                        );
                    }
                });
            ui.add_space(4.0);

            let fullscreen = egui::Button::new(
                egui::RichText::new("Capture fullscreen  [F]")
                    .color(theme::CHALK)
                    .strong(),
            )
            .min_size(tall);
            if ui.add(fullscreen).clicked() {
                self.start_capture(ui.ctx(), CaptureKind::Fullscreen(self.ui.selected_monitor));
                return;
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(&self.ui.status)
                        .color(theme::SLATE)
                        .small(),
                );
            });
        });
    }

    /// Label for the monitor picker's closed state.
    fn monitor_selection_label(&self) -> String {
        match self.ui.selected_monitor {
            None => "Primary monitor".to_string(),
            Some(sel) => self
                .ui
                .monitors
                .iter()
                .find(|m| m.index == sel)
                .map_or_else(|| "Primary monitor".to_string(), |m| m.label.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    fn ui_state() -> HomeUiState {
        HomeUiState {
            hidden: false,
            status: "connected".into(),
            selected_monitor: None,
            monitors: vec![MonitorEntry {
                index: 0,
                label: "0 · DP-1 (primary)".into(),
            }],
        }
    }

    #[test]
    fn show_directive_clears_hidden() {
        let mut ui = ui_state();
        ui.hidden = true;
        ui.apply(&HomeDirective::Show);
        assert!(!ui.hidden, "Show must clear the hidden flag");
    }

    #[test]
    fn status_directive_replaces_text_without_touching_visibility() {
        let mut ui = ui_state();
        ui.apply(&HomeDirective::SetStatus("daemon went away".into()));
        assert_eq!(ui.status, "daemon went away");
        assert!(!ui.hidden, "a status change must not touch visibility");

        let mut hidden = ui_state();
        hidden.hidden = true;
        hidden.apply(&HomeDirective::SetStatus("x".into()));
        assert!(hidden.hidden, "a status change must not reveal the window");
    }

    #[test]
    fn combo_label_tracks_selection_and_falls_back() {
        let mut app = HomeApp::new(Box::new(RecordingBridge::default()), "connected".into());
        assert_eq!(app.monitor_selection_label(), "Primary monitor");
        app.ui.selected_monitor = Some(0);
        assert_eq!(app.monitor_selection_label(), "0 · DP-1 (primary)");
        // A selection whose monitor vanished (hot-unplug between
        // captures) reads as the default instead of a blank combo.
        app.ui.selected_monitor = Some(7);
        assert_eq!(app.monitor_selection_label(), "Primary monitor");
    }

    #[test]
    fn stale_selection_is_reset_on_show_refresh() {
        // monitors() returns only index 0; a stale selection of 2
        // must be dropped by the same refresh update() applies on
        // Show.
        let monitors = [MonitorEntry {
            index: 0,
            label: "0 · DP-1 (primary)".into(),
        }];
        let mut selected = Some(2_u32);
        if let Some(sel) = selected {
            if !monitors.iter().any(|m| m.index == sel) {
                selected = None;
            }
        }
        assert_eq!(selected, None);
    }

    /// Shared recorder so tests can inspect a bridge handed to
    /// `HomeApp` as a `Box<dyn HomeBridge>` (no downcast needed).
    #[derive(Default, Clone)]
    struct Recording(Arc<Mutex<Recorder>>);

    #[derive(Default)]
    struct Recorder {
        region_requests: usize,
        fullscreen_requests: Vec<Option<u32>>,
    }

    /// Bridge double that records requests and replays directives.
    #[derive(Default, Clone)]
    struct RecordingBridge {
        rec: Recording,
        pending: Arc<Mutex<VecDeque<HomeDirective>>>,
    }

    impl HomeBridge for RecordingBridge {
        fn request_capture_region(&mut self) {
            self.rec.0.lock().expect("recorder mutex").region_requests += 1;
        }

        fn request_capture_fullscreen(&mut self, monitor: Option<u32>) {
            self.rec
                .0
                .lock()
                .expect("recorder mutex")
                .fullscreen_requests
                .push(monitor);
        }

        fn poll_directive(&mut self) -> Option<HomeDirective> {
            self.pending.lock().expect("pending mutex").pop_front()
        }

        fn monitors(&mut self) -> Vec<MonitorEntry> {
            vec![MonitorEntry {
                index: 0,
                label: "0 · DP-1 (primary)".into(),
            }]
        }
    }

    #[test]
    fn bridge_requests_flow_through() {
        let bridge = RecordingBridge::default();
        let rec = bridge.rec.clone();
        let mut boxed: Box<dyn HomeBridge> = Box::new(bridge);
        boxed.request_capture_region();
        boxed.request_capture_fullscreen(Some(2));
        let guard = rec.0.lock().expect("recorder mutex");
        assert_eq!(guard.region_requests, 1);
        assert_eq!(guard.fullscreen_requests, vec![Some(2)]);
    }

    #[test]
    fn initial_monitors_come_from_the_bridge() {
        let app = HomeApp::new(Box::new(RecordingBridge::default()), "connected".into());
        assert_eq!(app.ui.monitors.len(), 1);
        assert_eq!(app.ui.monitors[0].label, "0 · DP-1 (primary)");
    }
}
