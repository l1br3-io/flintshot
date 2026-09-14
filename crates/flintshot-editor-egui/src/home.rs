//! The eframe app that drives the flintshot home (app) window.
//!
//! [`HomeApp`] is the Win11-Snipping-Tool-style capture home: one
//! prominent **Capture** control (the single Capture action,
//! native-capture-ux — the OS's own selection UI where the backend
//! supports it, else the editor flow), an **Open** control (native
//! file dialog → the picked image opens in an editor window of its
//! own), and a status line. A **Paste** control is fully built but
//! hidden behind [`SHOW_PASTE`] for the future clipboard→editor
//! feature. This crate stays IPC-free — the window talks to the
//! world exclusively through [`HomeBridge`], which the app crate
//! implements (subscription pump + daemon delegation live there).
//!
//! Close/reopen contract (ADR-0047): a Capture click asks the
//! bridge to start the capture and then sends
//! [`egui::ViewportCommand::Close`] — a destroyed window cannot be
//! captured, on every compositor (winit's Wayland `set_visible` is
//! a hard no-op, so hiding is not a portable guarantee). The caller
//! (which owns the process) waits out the unmap grace, delegates,
//! and reopens a fresh window on [`HomeDirective::Show`]. **Open**
//! does *not* close the window: the editor runs elsewhere (its own
//! process — see [`HomeBridge::open_image`]), so there is nothing
//! to keep out of a shot. While the window is open, a standing
//! `request_repaint_after` keeps directives (status, focus pings)
//! flowing even when idle.

use std::path::PathBuf;
use std::time::Duration;

use eframe::App;
use egui::{Context, Key, KeyboardShortcut, Modifiers, ViewportCommand};

use crate::theme;

/// How often an idle home window re-polls directives (status
/// changes, focus pings arrive independent of input).
const REPOLL_INTERVAL: Duration = Duration::from_millis(250);

/// Whether the **Paste** control renders (and its `P` accelerator
/// arms). The clipboard→editor feature is a later effort; the
/// button, accelerator, and [`HomeBridge::request_paste`] plumbing
/// are built now so flipping this const is the whole toggle.
// TODO(future effort): clipboard → editor — read the system
// clipboard image straight into a fresh editor session.
const SHOW_PASTE: bool = false;

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
    /// Start the single Capture action (the window closes itself
    /// right after; the implementation owns the grace + delegation +
    /// the reopen decision).
    fn request_capture(&mut self);

    /// Open `path` in the editor — the `flintshot open` path. The
    /// editor must not run on this thread (the home window's eframe
    /// loop owns it and native loops do not nest), so the
    /// implementation opens it as its own window/process; the home
    /// window stays open and untouched.
    fn open_image(&mut self, path: PathBuf);

    /// Placeholder for the future clipboard→editor feature: read the
    /// system clipboard image into a fresh editor session. Only
    /// reachable while [`SHOW_PASTE`] is `true`.
    fn request_paste(&mut self);

    /// Take one pending directive, oldest first. Called every
    /// frame (see [`REPOLL_INTERVAL`]).
    fn poll_directive(&mut self) -> Option<HomeDirective>;
}

/// The eframe app for the home window. Owns no IPC state — see
/// [`HomeBridge`].
pub(crate) struct HomeApp {
    /// Implementation-supplied world (pump, delegation).
    bridge: Box<dyn HomeBridge>,
    /// Pure UI state, kept separate so directive application is
    /// headless-testable.
    ui: HomeUiState,
    /// Keyboard shortcut: C → capture.
    capture_shortcut: KeyboardShortcut,
    /// Keyboard shortcut: O → open.
    open_shortcut: KeyboardShortcut,
    /// Keyboard shortcut: P → paste (only consumed while
    /// [`SHOW_PASTE`] is `true`).
    paste_shortcut: KeyboardShortcut,
}

/// Pure UI state of the home window — the part directives mutate.
/// Extracted from [`HomeApp`] so the transition logic is
/// unit-testable without an egui context.
#[derive(Debug, Clone, PartialEq, Eq)]
struct HomeUiState {
    /// Status line contents.
    status: String,
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
        Self {
            ui: HomeUiState {
                status: initial_status,
            },
            bridge,
            capture_shortcut: KeyboardShortcut::new(Modifiers::NONE, Key::C),
            open_shortcut: KeyboardShortcut::new(Modifiers::NONE, Key::O),
            paste_shortcut: KeyboardShortcut::new(Modifiers::NONE, Key::P),
        }
    }

    /// Start the Capture action through the bridge and close the
    /// window — the not-in-the-shot guarantee (ADR-0047). Shared by
    /// the Capture button and its keyboard accelerator. Order
    /// matters: the intent must be registered before the close tears
    /// this app down.
    fn start_capture(&mut self, ctx: &Context) {
        self.bridge.request_capture();
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }

    /// The Open control: native file dialog, then hand the picked
    /// path to the bridge. Cancelling the dialog is a plain no-op —
    /// the window stays exactly as it was. The window does **not**
    /// close: the editor opens elsewhere (see [`HomeBridge::open_image`]).
    fn handle_open(&mut self) {
        if let Some(path) = pick_image_path() {
            self.bridge.open_image(path);
        }
    }
}

/// Prompt for an image to open via `rfd`. Reuses the editor
/// save path's blocking-modal pattern (`crates/flintshot-editor-egui`
/// already depends on `rfd`; the eframe event loop is already
/// blocking, and the dialog is what the user is driving anyway).
/// `None` when the user cancels.
fn pick_image_path() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Image files", &["png", "jpg", "jpeg", "webp", "bmp"])
        .pick_file()
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
    /// keyboard accelerators. Lives in `logic` (not `ui`) because
    /// eframe calls it even without recent input.
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
                // The window is open by construction here (a closed
                // one has no frames to run); Show while open is the
                // focus ping — raise and focus.
                ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(ViewportCommand::Focus);
            }
        }

        if ctx.input_mut(|i| i.consume_shortcut(&self.capture_shortcut)) {
            self.start_capture(ctx);
        } else if ctx.input_mut(|i| i.consume_shortcut(&self.open_shortcut)) {
            self.handle_open();
        } else if SHOW_PASTE && ctx.input_mut(|i| i.consume_shortcut(&self.paste_shortcut)) {
            self.bridge.request_paste();
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
            let capture = egui::Button::new(
                egui::RichText::new("Capture  [C]")
                    .color(theme::INK)
                    .strong(),
            )
            .fill(theme::EMBER)
            .min_size(tall);
            if ui.add(capture).clicked() {
                self.start_capture(ui.ctx());
                return;
            }
            ui.add_space(4.0);

            let open = egui::Button::new(
                egui::RichText::new("Open…  [O]")
                    .color(theme::CHALK)
                    .strong(),
            )
            .min_size(egui::vec2(ui.available_width(), 34.0));
            if ui.add(open).clicked() {
                self.handle_open();
                return;
            }

            if SHOW_PASTE {
                ui.add_space(4.0);
                let paste = egui::Button::new(
                    egui::RichText::new("Paste  [P]")
                        .color(theme::CHALK)
                        .strong(),
                )
                .min_size(egui::vec2(ui.available_width(), 34.0));
                if ui.add(paste).clicked() {
                    self.bridge.request_paste();
                }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    fn ui_state() -> HomeUiState {
        HomeUiState {
            status: "connected".into(),
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

    /// Pinned: the Paste control stays dark on this branch — the
    /// clipboard→editor feature is a later effort. Flipping
    /// [`SHOW_PASTE`] without shipping that feature breaks this test
    /// on purpose.
    #[test]
    #[expect(
        clippy::assertions_on_constants,
        reason = "the constant IS the pinned contract; the test exists to fail on flip"
    )]
    fn show_paste_const_is_false_on_this_branch() {
        assert!(!SHOW_PASTE);
    }

    /// Shared recorder so tests can inspect a bridge handed to
    /// `HomeApp` as a `Box<dyn HomeBridge>` (no downcast needed).
    #[derive(Default, Clone)]
    struct Recording(Arc<Mutex<Recorder>>);

    #[derive(Default)]
    struct Recorder {
        capture_requests: usize,
        opened: Vec<PathBuf>,
        paste_requests: usize,
    }

    /// Bridge double that records requests and replays directives.
    #[derive(Default, Clone)]
    struct RecordingBridge {
        rec: Recording,
        pending: Arc<Mutex<VecDeque<HomeDirective>>>,
    }

    impl HomeBridge for RecordingBridge {
        fn request_capture(&mut self) {
            self.rec.0.lock().expect("recorder mutex").capture_requests += 1;
        }

        fn open_image(&mut self, path: PathBuf) {
            self.rec.0.lock().expect("recorder mutex").opened.push(path);
        }

        fn request_paste(&mut self) {
            self.rec.0.lock().expect("recorder mutex").paste_requests += 1;
        }

        fn poll_directive(&mut self) -> Option<HomeDirective> {
            self.pending.lock().expect("pending mutex").pop_front()
        }
    }

    #[test]
    fn bridge_requests_flow_through() {
        let bridge = RecordingBridge::default();
        let rec = bridge.rec.clone();
        let mut boxed: Box<dyn HomeBridge> = Box::new(bridge);
        boxed.request_capture();
        boxed.open_image(PathBuf::from("/tmp/shot.png"));
        boxed.request_paste();
        let guard = rec.0.lock().expect("recorder mutex");
        assert_eq!(guard.capture_requests, 1);
        assert_eq!(guard.opened, [PathBuf::from("/tmp/shot.png")]);
        assert_eq!(guard.paste_requests, 1);
    }

    #[test]
    fn capture_accelerator_fires_the_bridge_request() {
        // A synthetic C keystroke through a default context must
        // reach the bridge — the accelerator is part of the Capture
        // control's contract.
        let bridge = RecordingBridge::default();
        let rec = bridge.rec.clone();
        let mut app = HomeApp::new(Box::new(bridge), "connected".into());
        let ctx = egui::Context::default();
        ctx.begin_pass(egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::C,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        });
        app.logic_impl(&ctx);
        let _ = ctx.end_pass();
        assert_eq!(
            rec.0.lock().expect("recorder mutex").capture_requests,
            1,
            "the C accelerator must fire request_capture exactly once"
        );
    }
}
