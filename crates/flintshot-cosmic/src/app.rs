//! Applet model: state, messages, and pure decision logic.
//!
//! This module is the applet's "brain." It holds the [`App`] struct shape
//! (the fields the libcosmic `Application` impl in Task 003 will consume),
//! the [`Message`] vocabulary (clicks, forwarded IPC events, daemon
//! lifecycle), and the **pure** associated functions that map clicks →
//! [`Request`]s, [`Event`]s → status-line text, and drive the
//! [`ConnectionState`] machine.
//!
//! The pure helpers ([`App::request_for`], [`App::status_text`],
//! [`App::next_state`]) are associated functions (no `self`) so they are
//! unit-tested headlessly without constructing a [`cosmic::app::Core`] or
//! running a libcosmic event loop. Task 003 adds the `cosmic::Application`
//! impl that wires these into the UI; Task 004 feeds `Message::IpcEvent`
//! from a subscriber `Subscription`; Task 005 wires `ConnectionState`
//! into the popup's enabled flags.

use cosmic::app::{Action as AppAction, Core, Task};
use cosmic::iced::window::Id;
use cosmic::iced::Rectangle;
use cosmic::surface::action::{app_popup, destroy_popup, LiveSettings};
use cosmic::surface::Action as SurfaceAction;
use cosmic::Action as CosmicAction;
use cosmic::Element;
use flintshot_core::ipc::{Event, Request};
use std::process::Command;

/// Applet-wide connection state. Drives which actions the popup enables
/// (Task 005): [`NoDaemon`](Self::NoDaemon) disables capture/pin and shows
/// the *Start daemon* button; [`Connected`](Self::Connected) shows the
/// full menu + live status line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    /// No daemon answered the probe — capture/pin actions are disabled,
    /// the popup shows a hint + *Start daemon*.
    NoDaemon,
    /// The daemon is reachable — the full popup is enabled and the
    /// subscriber stream feeds the status line.
    Connected,
}

/// One UI or IPC event the applet reacts to.
///
/// Click variants originate from the popup menu (Task 003). [`IpcEvent`]
/// is forwarded by the background subscriber (Task 004).
/// [`DaemonGone`]/[`DaemonUp`] are lifecycle signals from the subscriber
/// connect/read path. [`PopupClosed`] is fired by libcosmic when the
/// popup window is dismissed. [`Surface`] carries popup open/close
/// actions for the libcosmic surface lifecycle.
///
/// [`IpcEvent`]: Message::IpcEvent
/// [`DaemonGone`]: Message::DaemonGone
/// [`DaemonUp`]: Message::DaemonUp
/// [`PopupClosed`]: Message::PopupClosed
/// [`Surface`]: Message::Surface
#[non_exhaustive]
#[derive(Clone, Debug)]
pub enum Message {
    /// *Capture region* clicked.
    ClickRegion,
    /// *Capture fullscreen* clicked.
    ClickFullscreen,
    /// *Fullscreen on monitor N* clicked.
    ClickMonitor(u32),
    /// *Pin last* clicked (Phase 3).
    ClickPinLast,
    /// *Open config* clicked (opens `config.toml` in `$EDITOR`).
    ClickOpenConfig,
    /// *Quit* clicked (sends `Request::Quit` to the daemon).
    ClickQuit,
    /// *Start daemon* clicked (spawns `flintshot daemon` detached).
    #[allow(
        dead_code,
        reason = "constructed by Task 005's Start-daemon button in popup_view"
    )]
    ClickStartDaemon,
    /// The popup window was dismissed (libcosmic lifecycle).
    PopupClosed(Id),
    /// A forwarded daemon [`Event`] from the subscriber stream.
    #[allow(
        dead_code,
        reason = "constructed by Task 004's subscriber Subscription"
    )]
    IpcEvent(Event),
    /// The subscriber lost the daemon connection.
    #[allow(
        dead_code,
        reason = "constructed by Task 004's subscriber on connect/read failure"
    )]
    DaemonGone,
    /// A probe / start-daemon succeeded — the daemon is reachable.
    #[allow(
        dead_code,
        reason = "constructed by Task 004/005 on successful connect/probe"
    )]
    DaemonUp,
    /// A libcosmic surface lifecycle action (popup open/close). Produced
    /// by the panel button and consumed by `update` to drive
    /// `cosmic::app::Action::Surface`.
    Surface(SurfaceAction),
}

/// The libcosmic applet application model.
///
/// Fields are private; the `cosmic::Application` impl (Task 003) lives in
/// this crate and accesses them directly. The pure decision logic is on
/// associated functions ([`request_for`](Self::request_for),
/// [`status_text`](Self::status_text), [`next_state`](Self::next_state))
/// so it tests without a [`Core`] or a display.
#[allow(
    clippy::module_name_repetitions,
    reason = "`App` is the libcosmic Application model name; the module groups app-model code"
)]
pub struct App {
    /// libcosmic core state (theme, nav-bar, window bookkeeping).
    core: Core,
    /// The popup's window id when open, `None` when closed.
    popup: Option<Id>,
    /// Daemon reachability — drives the popup's enabled actions.
    connection: ConnectionState,
    /// Human-readable text rendered into the popup's status line.
    last_event_text: String,
}

impl App {
    /// Pure: which [`Request`] (if any) a click maps to.
    ///
    // RATIONALE: this mirrors `tray::action_for`
    // (`crates/flintshot-app/src/tray.rs:39`) — the two surfaces (tray +
    // applet) map the same user intents to the same daemon requests, so
    // a capture initiated from the applet reaches the worker table via
    // the exact same path as a tray click or a hotkey. Keeping the
    // mapping on `App` makes the call sites in Task 003 read as
    // `App::request_for(&msg)`, which documents ownership.
    #[must_use]
    pub fn request_for(msg: &Message) -> Option<Request> {
        match msg {
            Message::ClickRegion => Some(Request::CaptureRegion {
                monitor: None,
                delay_ms: 0,
            }),
            Message::ClickFullscreen => Some(Request::CaptureFullscreen { monitor: None }),
            Message::ClickMonitor(i) => Some(Request::CaptureFullscreen { monitor: Some(*i) }),
            Message::ClickPinLast => Some(Request::PinLast),
            Message::ClickQuit => Some(Request::Quit),
            // OpenConfig is handled locally (Task 003 spawns $EDITOR);
            // StartDaemon is handled via DaemonSpawner (Task 005); IPC
            // events, lifecycle signals, and surface actions are not requests.
            Message::ClickOpenConfig
            | Message::ClickStartDaemon
            | Message::PopupClosed(_)
            | Message::IpcEvent(_)
            | Message::DaemonGone
            | Message::DaemonUp
            | Message::Surface(_) => None,
        }
    }

    /// Pure: render an [`Event`] as the popup's status-line text.
    #[must_use]
    pub fn status_text(event: &Event) -> String {
        match event {
            Event::CaptureStarted => "capturing…".to_string(),
            Event::EditorOpened => "editor open".to_string(),
            Event::Saved { path } => {
                format!("saved: {}", path.display())
            }
            Event::Pinned { id } => format!("pinned #{id}"),
            Event::Error { code, message } => {
                format!("error: {code}: {message}")
            }
            Event::Uploaded { url, .. } => format!("uploaded: {url}"),
            // Event is #[non_exhaustive]; future variants render generically
            // so the applet degrades gracefully instead of panicking.
            _ => format!("{event:?}"),
        }
    }

    /// Pure: advance the [`ConnectionState`] machine.
    ///
    /// [`DaemonUp`](Message::DaemonUp) → [`Connected`](ConnectionState::Connected)
    /// from any state; [`DaemonGone`](Message::DaemonGone) →
    /// [`NoDaemon`](ConnectionState::NoDaemon) from any state; all other
    /// messages leave the state unchanged.
    #[must_use]
    pub fn next_state(state: ConnectionState, msg: &Message) -> ConnectionState {
        match msg {
            Message::DaemonUp => ConnectionState::Connected,
            Message::DaemonGone => ConnectionState::NoDaemon,
            _ => state,
        }
    }

    /// Convenience: apply `msg` to `self`. Advances the connection state
    /// via [`next_state`](Self::next_state); for
    /// [`IpcEvent`](Message::IpcEvent) also updates
    /// [`last_event_text`](App::last_event_text) via
    /// [`status_text`](Self::status_text). No I/O — pure glue.
    pub fn apply(&mut self, msg: &Message) {
        if let Message::IpcEvent(event) = msg {
            self.last_event_text = Self::status_text(event);
        }
        self.connection = Self::next_state(self.connection, msg);
    }

    /// The current status-line text (rendered into the popup by
    /// [`menu::popup_view`](crate::menu::popup_view)). Empty until the
    /// subscriber stream delivers the first [`Event`] (Task 004).
    #[must_use]
    pub fn last_event_text(&self) -> &str {
        &self.last_event_text
    }
}

/// Reverse-DNS app id. Must match the `.desktop` filename (Task 006)
/// exactly — `cosmic-panel` correlates a running applet with its config
/// slot via this string.
const APP_ID: &str = "io.github.anomalyco.Flintshot";

/// Open `config.toml` in `$EDITOR` (fallback `$VISUAL`); log the path
/// if neither is set.
///
/// RATIONALE: this replicates the daemon's `open_config_file`
/// (`crates/flintshot-app/src/daemon.rs:237`) because the daemon's
/// helper is in-process and unreachable from the applet — a bin cannot
/// call another bin's private function. The spawn is fire-and-forget
/// (detached child); the daemon process is unaffected.
fn open_config_file() -> color_eyre::eyre::Result<()> {
    let path = flintshot_core::config_path()
        .ok_or_else(|| color_eyre::eyre::eyre!("no platform config_dir available"))?;
    if let Some(editor) = std::env::var_os("EDITOR").or_else(|| std::env::var_os("VISUAL")) {
        Command::new(&editor).arg(&path).spawn().map_err(|e| {
            color_eyre::eyre::eyre!("launching {editor:?} on {}: {e}", path.display())
        })?;
        tracing::info!(editor = ?editor, path = %path.display(), "opened config");
    } else {
        tracing::info!(path = %path.display(), "no $EDITOR/$VISUAL; config path logged");
    }
    Ok(())
}

/// Build a fire-and-forget background [`Task`] that sends `req` to the
/// daemon.
///
/// RATIONALE: the UI thread never blocks on the socket; `send_request`
/// runs on the executor and any error is logged. The daemon's reaction
/// arrives via the subscriber stream (Task 004), not a response — the
/// tray uses the same fire-and-forget shape.
fn send_request_task(req: Request) -> Task<Message> {
    cosmic::iced::Task::future(async move {
        if let Err(e) = flintshot_ipc::send_request(None, &req).await {
            tracing::warn!(error = %e, "applet IPC send failed");
        }
    })
    .discard()
}

impl cosmic::Application for App {
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = APP_ID;

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let app = App {
            core,
            popup: None,
            // Provisional: optimistic Connected so buttons are enabled on
            // first paint when a daemon is running. Task 005 flips this to
            // NoDaemon-aware (probe on startup).
            connection: ConnectionState::Connected,
            last_event_text: String::new(),
        };
        (app, Task::none())
    }

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match &message {
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(id) {
                    self.popup = None;
                }
                Task::none()
            }
            Message::Surface(action) => {
                cosmic::task::message(CosmicAction::Cosmic(AppAction::Surface(action.clone())))
            }
            Message::ClickOpenConfig => {
                if let Err(e) = open_config_file() {
                    tracing::warn!(error = %e, "opening config from applet");
                }
                Task::none()
            }
            Message::ClickStartDaemon => {
                // TODO(005): spawn via DaemonSpawner + one-shot re-probe.
                Task::none()
            }
            Message::IpcEvent(_) | Message::DaemonGone | Message::DaemonUp => {
                self.apply(&message);
                Task::none()
            }
            // ClickRegion / ClickFullscreen / ClickMonitor(_) / ClickPinLast
            // / ClickQuit — dispatch via background IPC Task.
            _ => match App::request_for(&message) {
                Some(req) => send_request_task(req),
                None => Task::none(),
            },
        }
    }

    #[allow(
        clippy::cast_possible_truncation,
        reason = "popup positioner anchor rect uses i32; the f32 values are screen-space pixel coordinates from libcosmic's on_press_with_rectangle — mirrors examples/applet/src/window.rs:116-121 verbatim"
    )]
    fn view(&self) -> Element<'_, Message> {
        let have_popup = self.popup;
        let btn = self
            .core
            .applet
            .icon_button("camera-photo-symbolic")
            // TODO: replace with a real flintshot icon asset (Task 006 / polish).
            .on_press_with_rectangle(move |offset, bounds| {
                if let Some(id) = have_popup {
                    Message::Surface(destroy_popup(id))
                } else {
                    Message::Surface(app_popup::<App>(
                        |_| LiveSettings::default(),
                        move |state: &mut App| {
                            let new_id = Id::unique();
                            state.popup = Some(new_id);
                            let main_window = state.core.main_window_id().unwrap_or(Id::RESERVED);
                            let mut popup_settings = state.core.applet.get_popup_settings(
                                main_window,
                                new_id,
                                None,
                                None,
                                None,
                            );
                            popup_settings.positioner.anchor_rect = Rectangle {
                                x: (bounds.x - offset.x) as i32,
                                y: (bounds.y - offset.y) as i32,
                                width: bounds.width as i32,
                                height: bounds.height as i32,
                            };
                            popup_settings
                        },
                        Some(Box::new(move |state: &App| {
                            let content = crate::menu::popup_view(state);
                            Element::from(state.core.applet.popup_container(content))
                                .map(CosmicAction::App)
                        })),
                    ))
                }
            });

        Element::from(self.core.applet.applet_tooltip::<Message>(
            btn,
            "flintshot",
            self.popup.is_some(),
            Message::Surface,
            None,
        ))
    }

    fn view_window(&self, _id: Id) -> Element<'_, Message> {
        // GOTCHA: popup content is built inside the app_popup closure's
        // third argument in view() — the example at revision ef162b8
        // confirms this pattern (its view_window returns "oops").
        // Delegating here too so the default trait impl's panic never fires.
        crate::menu::popup_view(self)
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

#[cfg(test)]
impl App {
    /// Test-only constructor: build an `App` with the given state fields
    /// and a default [`Core`] (no display required).
    fn for_test(connection: ConnectionState, last_event_text: &str) -> Self {
        Self {
            core: Core::default(),
            popup: None,
            connection,
            last_event_text: last_event_text.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // ── request_for ─────────────────────────────────────────────────

    #[test]
    fn click_region_maps_to_capture_region() {
        let Some(Request::CaptureRegion { monitor, delay_ms }) =
            App::request_for(&Message::ClickRegion)
        else {
            panic!("expected CaptureRegion");
        };
        assert!(monitor.is_none(), "applet region uses the default monitor");
        assert_eq!(delay_ms, 0);
    }

    #[test]
    fn click_fullscreen_maps_to_capture_fullscreen_default_monitor() {
        assert!(matches!(
            App::request_for(&Message::ClickFullscreen),
            Some(Request::CaptureFullscreen { monitor: None })
        ));
    }

    #[test]
    fn click_monitor_maps_to_capture_fullscreen_with_index() {
        let Some(Request::CaptureFullscreen { monitor }) =
            App::request_for(&Message::ClickMonitor(3))
        else {
            panic!("expected CaptureFullscreen");
        };
        assert_eq!(monitor, Some(3));
    }

    #[test]
    fn click_pin_last_maps_to_pin_last() {
        assert!(matches!(
            App::request_for(&Message::ClickPinLast),
            Some(Request::PinLast)
        ));
    }

    #[test]
    fn click_quit_maps_to_quit() {
        assert!(matches!(
            App::request_for(&Message::ClickQuit),
            Some(Request::Quit)
        ));
    }

    #[test]
    fn non_action_clicks_map_to_none() {
        assert_eq!(App::request_for(&Message::ClickOpenConfig), None);
        assert_eq!(App::request_for(&Message::ClickStartDaemon), None);
    }

    #[test]
    fn surface_action_maps_to_none() {
        let action = destroy_popup(Id::unique());
        assert_eq!(
            App::request_for(&Message::Surface(action)),
            None,
            "Surface actions are framework routing, not daemon requests"
        );
    }

    #[test]
    fn ipc_and_lifecycle_messages_map_to_none() {
        assert_eq!(
            App::request_for(&Message::IpcEvent(Event::CaptureStarted)),
            None
        );
        assert_eq!(App::request_for(&Message::DaemonGone), None);
        assert_eq!(App::request_for(&Message::DaemonUp), None);
    }

    // ── status_text ─────────────────────────────────────────────────

    #[test]
    fn status_capture_started() {
        let text = App::status_text(&Event::CaptureStarted);
        assert_eq!(text, "capturing…");
    }

    #[test]
    fn status_editor_opened() {
        let text = App::status_text(&Event::EditorOpened);
        assert_eq!(text, "editor open");
    }

    #[test]
    fn status_saved_contains_path() {
        let text = App::status_text(&Event::Saved {
            path: PathBuf::from("/tmp/shot.png"),
        });
        assert!(
            text.contains("/tmp/shot.png"),
            "status must contain the path: {text}"
        );
        assert!(
            text.starts_with("saved:"),
            "status must start with 'saved:': {text}"
        );
    }

    #[test]
    fn status_pinned_shows_id() {
        let text = App::status_text(&Event::Pinned { id: 42 });
        assert_eq!(text, "pinned #42");
    }

    #[test]
    fn status_error_surfaces_code_and_message() {
        let text = App::status_text(&Event::Error {
            code: "capture-failed".into(),
            message: "xcap returned no frames".into(),
        });
        assert!(!text.is_empty(), "error status must be non-empty");
        assert!(
            text.contains("capture-failed"),
            "status must contain the code: {text}"
        );
        assert!(
            text.contains("xcap returned no frames"),
            "status should contain the message: {text}"
        );
    }

    #[test]
    fn status_uploaded_shows_url() {
        let text = App::status_text(&Event::Uploaded {
            url: "https://i/x".into(),
            delete_handle: None,
        });
        assert!(
            text.contains("https://i/x"),
            "status must contain the url: {text}"
        );
    }

    #[test]
    fn status_all_variants_are_non_empty() {
        let cases = [
            Event::CaptureStarted,
            Event::EditorOpened,
            Event::Saved {
                path: PathBuf::from("/x.png"),
            },
            Event::Error {
                code: "c".into(),
                message: "m".into(),
            },
            Event::Pinned { id: 0 },
            Event::Uploaded {
                url: "u".into(),
                delete_handle: None,
            },
        ];
        for event in &cases {
            let text = App::status_text(event);
            assert!(!text.is_empty(), "status for {event:?} must be non-empty");
        }
    }

    // ── next_state ──────────────────────────────────────────────────

    #[test]
    fn daemon_up_transitions_to_connected_from_no_daemon() {
        assert_eq!(
            App::next_state(ConnectionState::NoDaemon, &Message::DaemonUp),
            ConnectionState::Connected
        );
    }

    #[test]
    fn daemon_up_is_idempotent_when_already_connected() {
        assert_eq!(
            App::next_state(ConnectionState::Connected, &Message::DaemonUp),
            ConnectionState::Connected
        );
    }

    #[test]
    fn daemon_gone_transitions_to_no_daemon_from_connected() {
        assert_eq!(
            App::next_state(ConnectionState::Connected, &Message::DaemonGone),
            ConnectionState::NoDaemon
        );
    }

    #[test]
    fn daemon_gone_is_idempotent_when_already_no_daemon() {
        assert_eq!(
            App::next_state(ConnectionState::NoDaemon, &Message::DaemonGone),
            ConnectionState::NoDaemon
        );
    }

    #[test]
    fn clicks_and_ipc_events_do_not_change_state() {
        for state in [ConnectionState::NoDaemon, ConnectionState::Connected] {
            let msgs = [
                Message::ClickRegion,
                Message::ClickFullscreen,
                Message::ClickMonitor(0),
                Message::ClickPinLast,
                Message::ClickOpenConfig,
                Message::ClickQuit,
                Message::ClickStartDaemon,
                Message::IpcEvent(Event::CaptureStarted),
            ];
            for msg in &msgs {
                assert_eq!(
                    App::next_state(state, msg),
                    state,
                    "state should be unchanged under {msg:?} from {state:?}"
                );
            }
        }
    }

    // ── apply ───────────────────────────────────────────────────────

    #[test]
    fn apply_ipc_event_updates_status_text() {
        let mut app = App::for_test(ConnectionState::Connected, "");
        app.apply(&Message::IpcEvent(Event::CaptureStarted));
        assert_eq!(app.last_event_text, "capturing…");
        assert_eq!(app.connection, ConnectionState::Connected);
    }

    #[test]
    fn apply_daemon_gone_transitions_to_no_daemon() {
        let mut app = App::for_test(ConnectionState::Connected, "editor open");
        app.apply(&Message::DaemonGone);
        assert_eq!(app.connection, ConnectionState::NoDaemon);
        // Status text is preserved — DaemonGone doesn't carry an event.
        assert_eq!(app.last_event_text, "editor open");
    }

    #[test]
    fn apply_daemon_up_does_not_clobber_status_text() {
        let mut app = App::for_test(ConnectionState::NoDaemon, "saved: /x.png");
        app.apply(&Message::DaemonUp);
        assert_eq!(app.connection, ConnectionState::Connected);
        assert_eq!(app.last_event_text, "saved: /x.png");
    }
}
