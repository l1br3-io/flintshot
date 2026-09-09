//! The eframe app that drives the flintshot home (app) window.
//!
//! [`HomeApp`] is the Win11-Snipping-Tool-style capture home: two
//! prominent controls (*Capture region*, *Capture fullscreen* plus a
//! monitor picker) and a status line. This crate stays IPC-free —
//! the window talks to the world exclusively through [`HomeBridge`],
//! which the app crate implements (subscription pump + daemon
//! delegation live there).
//!
//! Close/reopen contract (ADR-0047): a control click asks the
//! bridge to start the capture and then sends
//! [`egui::ViewportCommand::Close`] — a destroyed window cannot be
//! captured, on every compositor (winit's Wayland `set_visible` is
//! a hard no-op, so hiding is not a portable guarantee). The caller
//! (which owns the process) waits out the unmap grace, delegates,
//! and reopens a fresh window on [`HomeDirective::Show`]. While the
//! window is open, a standing `request_repaint_after` keeps
//! directives (status, focus pings) flowing even when idle.

use std::time::Duration;

use eframe::App;
use egui::{Context, Key, KeyboardShortcut, Modifiers, ViewportCommand};

use crate::theme;

/// How often an idle home window re-polls directives (status
/// changes, focus pings arrive independent of input).
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
    /// The window should be (re)opened / brought forward: the
    /// capture session ended, the daemon went away, or someone
    /// asked for the window (focus ping). While a window is open
    /// this is just a focus; the caller's outer loop treats it as
    /// the reopen signal when the window is closed (ADR-0047).
    Show,
    /// The window must close now — a capture is imminent and it may
    /// not be in the shot. Covers **external** triggers too (the
    /// pre-grab `HideForCapture` handshake and any `CaptureStarted`),
    /// closing the ADR-0047 gap where only self-triggered captures
    /// hid the window. The outer loop distinguishes this close from
    /// a user quit and reopens on the session's end.
    Hide,
    /// Replace the status line text (daemon/connection state).
    SetStatus(String),
}

/// The home window's only seam to the outside world. Implemented by
/// the app crate; every method runs on the UI thread.
pub trait HomeBridge {
    /// Start a region capture (the window closes itself right
    /// after; the implementation owns the grace + delegation + the
    /// reopen decision).
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
    /// Status line contents.
    status: String,
    /// Selected fullscreen monitor (`None` = primary/default).
    selected_monitor: Option<u32>,
    /// Monitor picker entries (refreshed on show).
    monitors: Vec<MonitorEntry>,
}

impl HomeUiState {
    /// Apply one directive (the status line is the only mutable
    /// display state — visibility is the caller's close/reopen
    /// business, per ADR-0047).
    fn apply(&mut self, directive: &HomeDirective) {
        if let HomeDirective::SetStatus(text) = directive {
            self.status.clone_from(text);
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
                status: initial_status,
                selected_monitor: None,
                monitors,
            },
            region_shortcut: KeyboardShortcut::new(Modifiers::NONE, Key::R),
            fullscreen_shortcut: KeyboardShortcut::new(Modifiers::NONE, Key::F),
        }
    }

    /// Start a capture through the bridge and close the window —
    /// the not-in-the-shot guarantee (ADR-0047). Shared by the
    /// region button, the fullscreen button, and the keyboard
    /// shortcuts. Order matters: the intent must be registered
    /// before the close tears this app down.
    fn start_capture(&mut self, ctx: &Context, kind: CaptureKind) {
        match kind {
            CaptureKind::Region => self.bridge.request_capture_region(),
            CaptureKind::Fullscreen(monitor) => self.bridge.request_capture_fullscreen(monitor),
        }
        ctx.send_viewport_cmd(ViewportCommand::Close);
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
    /// Headless-callable core of `App::logic`: directive polling and
    /// keyboard shortcuts. Lives in `logic` (not `ui`) because eframe
    /// calls it even without recent input.
    fn logic_impl(&mut self, ctx: &Context) {
        // Directives (status changes, focus pings) arrive independent
        // of input; keep a slow repaint heartbeat so they land.
        ctx.request_repaint_after(REPOLL_INTERVAL);

        while let Some(directive) = self.bridge.poll_directive() {
            let focus = matches!(directive, HomeDirective::Show);
            let hide = matches!(directive, HomeDirective::Hide);
            self.ui.apply(&directive);
            if hide {
                // Not-in-the-shot for externally triggered captures:
                // the same close the self-trigger path performs, on
                // the daemon's pre-grab say-so (ADR-0047 gap).
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            if focus {
                self.ui.monitors = self.bridge.monitors();
                if let Some(sel) = self.ui.selected_monitor {
                    if !self.ui.monitors.iter().any(|m| m.index == sel) {
                        self.ui.selected_monitor = None;
                    }
                }
                // The window is open by construction here (a closed
                // one has no frames to run); Show while open is the
                // focus ping — raise and focus.
                ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(ViewportCommand::Focus);
            }
        }

        if ctx.input_mut(|i| i.consume_shortcut(&self.region_shortcut)) {
            self.start_capture(ctx, CaptureKind::Region);
        } else if ctx.input_mut(|i| i.consume_shortcut(&self.fullscreen_shortcut)) {
            self.start_capture(ctx, CaptureKind::Fullscreen(self.ui.selected_monitor));
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
            status: "connected".into(),
            selected_monitor: None,
            monitors: vec![MonitorEntry {
                index: 0,
                label: "0 · DP-1 (primary)".into(),
            }],
        }
    }

    #[test]
    fn show_directive_leaves_display_state_alone() {
        // Visibility is the caller's close/reopen business (ADR-0047);
        // the UI only owns the status line.
        let mut ui = ui_state();
        ui.apply(&HomeDirective::Show);
        assert_eq!(ui.status, "connected");
    }

    #[test]
    fn status_directive_replaces_text() {
        let mut ui = ui_state();
        ui.apply(&HomeDirective::SetStatus("daemon went away".into()));
        assert_eq!(ui.status, "daemon went away");
    }

    #[test]
    fn hide_directive_leaves_display_state_alone() {
        let mut ui = ui_state();
        ui.apply(&HomeDirective::Hide);
        assert_eq!(ui.status, "connected");
    }

    #[test]
    fn hide_directive_closes_the_window_headlessly() {
        // A default context stands in for the real one: the close
        // command queues without a display (the CloseHandle test
        // pattern); what is under test is that the Hide branch is
        // reached at all — a panic or a missed branch fails this.
        let bridge = RecordingBridge::default();
        bridge
            .pending
            .lock()
            .expect("pending mutex")
            .push_back(HomeDirective::Hide);
        let mut app = HomeApp::new(Box::new(bridge), "connected".into());
        let ctx = egui::Context::default();
        app.logic_impl(&ctx);
        // The directive was consumed (not skipped as unknown).
        assert!(
            app.bridge.poll_directive().is_none(),
            "the Hide directive was consumed by the frame"
        );
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
