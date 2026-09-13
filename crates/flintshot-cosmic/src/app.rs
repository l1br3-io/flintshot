//! COSMIC panel applet: panel button + popup menu.
//!
//! This is the Task 7 UI layer, extended in Task 8 with a live event
//! stream. It builds a [`cosmic::Application`] that shows a panel
//! button; clicking it opens a popup mirroring the tray menu —
//! *Capture region*, *Capture fullscreen*, per-monitor entries,
//! *Open last*, *Open config*, *Quit*. Menu clicks are resolved
//! by the pure [`crate::menu`] dispatch (unit-tested headless) and
//! performed here as side-effects: daemon [`Request`]s go out via
//! [`crate::ipc::send_request`], and *Open config* shells out to
//! `$EDITOR`.
//!
//! Task 8 layers the live feedback: a persistent [`subscribe`]
//! connection reads the daemon's [`Event`] stream and updates a status
//! line in the popup (*Idle* / *Capturing…* / *Editor opened* / *Saved
//! <basename>* / error). When the daemon is absent, the
//! popup shows *"daemon not running"* and disables action buttons. A
//! background [`subscription`] drives the reconnect loop so a daemon
//! restart doesn't require restarting the applet.
//!
//! The applet lifecycle (popup open/close, surface actions) follows
//! libcosmic's canonical applet template — see the `applet` example in
//! the libcosmic tree.
//!
//! [`subscribe`]: crate::ipc::subscribe
//! [`Event`]: flintshot_core::ipc::Event
//! [`subscription`]: cosmic::Application::subscription

use cosmic::app::{Core, Task};
use cosmic::iced::core::window;
use cosmic::iced::futures::SinkExt;
use cosmic::iced::window::Id;
use cosmic::iced::{Length, Rectangle, Subscription, Vector};
use cosmic::surface::action::{app_popup, destroy_popup};
use cosmic::widget::{divider, text};
use cosmic::{applet, Action as CosmicAction, Element};

use crate::ipc::{self, status_for_event, DaemonState};
use crate::menu::{self, MenuMessage};

/// Reverse-DNS app id (COSMIC/xdg convention).
const APP_ID: &str = "dev.flintshot.FlintshotCosmic";

/// Freedesktop icon name for the panel button. `camera-photo` is widely
/// installed; COSMIC falls back gracefully if it's missing. The polish
/// sweep can swap in a branded mark.
const PANEL_ICON: &str = "camera-photo";

/// Reconnect interval: how long to wait between probe attempts when the
/// daemon is absent or the event stream has dropped. Configurable in a
/// future polish pass; 2 s is responsive without busy-looping.
const RECONNECT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);

/// Settle window between destroying the popup for a capture and
/// sending the request: the popup's unmap round trip + COSMIC's close
/// animation must finish before the daemon's worker grabs, or the
/// applet's own popup lands in the shot. The IPC request carries no
/// menu-origin marker, so the daemon applies only its 250 ms icon
/// grace (ADR-0037's open edge) — the applet self-serves the menu
/// grace here instead. With worker spawn + the icon grace,
/// click-to-grab lands ≈ the tray path's ~600 ms (ADR-0041).
const POPUP_CLOSE_SETTLE: std::time::Duration = std::time::Duration::from_millis(250);

/// The applet state: a COSMIC [`Core`] plus the current popup id (if
/// the popup is open), the live status line, and whether the daemon is
/// reachable. The panel button toggles `popup`.
pub struct App {
    core: Core,
    popup: Option<Id>,
    /// Current status line text (e.g. *"Idle"*, *"Capturing…"*). Set
    /// from the event stream; rendered at the top of the popup.
    status: String,
    /// Whether the daemon is reachable. When `false`, the popup shows
    /// *"daemon not running"* and disables daemon-dependent buttons.
    daemon_up: bool,
}

/// Messages handled by [`App`]. Menu actions ([`Message::Menu`]) are
/// forwarded to the pure [`menu::action_for`] resolver; the rest is
/// libcosmic lifecycle (popup close, surface action, async completion)
/// or event-stream updates from the background subscription.
#[derive(Clone, Debug)]
pub enum Message {
    /// A popup menu item was clicked.
    Menu(MenuMessage),
    /// A popup window requested close (libcosmic lifecycle).
    PopupClosed(Id),
    /// A libcosmic surface action (popup open/close plumbing).
    Surface(cosmic::surface::Action),
    /// An async IPC send completed (fire-and-forget; errors are logged
    /// in the future before this is emitted).
    Sent,
    /// A new status line from the event stream (e.g. *"Capturing…"*,
    /// *"Saved shot.png"*, *"Error: no display"*).
    Status(String),
    /// The daemon was lost (probe failed or the stream dropped). The
    /// popup switches to the daemon-absent display.
    DaemonLost,
    /// The daemon reappeared and the subscription reconnected. The
    /// popup switches back to the live display with *"Idle"* status.
    DaemonBack,
}

impl cosmic::Application for App {
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Message>) {
        // Start optimistic-but-conservative: the subscription probes on
        // its first tick and emits DaemonBack/DaemonLost within
        // milliseconds. The popup isn't visible until the user clicks
        // the panel button, so the initial state is never seen.
        (
            Self {
                core,
                popup: None,
                status: "Idle".to_string(),
                daemon_up: false,
            },
            Task::none(),
        )
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Menu(menu_msg) => match menu::action_for(&menu_msg) {
                menu::Action::Request(req) => {
                    // A capture is about to freeze the screen: destroy
                    // the popup at the click and let the settle window
                    // cover its unmap + close animation before the
                    // request even reaches the daemon — otherwise the
                    // applet's own popup lands in the grabbed frame
                    // (ADR-0041). Other requests keep the popup open:
                    // its live status line is the feedback for
                    // open/quit.
                    let close = if menu::request_spawns_capture(&req) {
                        self.popup
                            .take()
                            .map(|id| cosmic::task::message(Message::Surface(destroy_popup(id))))
                    } else {
                        None
                    };
                    let settle = close.is_some();
                    let send = cosmic::task::future(async move {
                        if settle {
                            tokio::time::sleep(POPUP_CLOSE_SETTLE).await;
                        }
                        // Fire-and-forget: clone the Request, send it on
                        // the runtime, log failures, yield a no-op when
                        // done. The socket-path override is None →
                        // platform default.
                        if let Err(e) = crate::ipc::send_request(None, &req).await {
                            tracing::warn!(error = %e, "applet: sending IPC request");
                        }
                        Message::Sent
                    });
                    return match close {
                        Some(close) => cosmic::task::batch([close, send]),
                        None => send,
                    };
                }
                menu::Action::OpenConfig => {
                    if let Err(e) = menu::open_config() {
                        tracing::warn!(error = %e, "applet: opening config");
                    }
                }
            },
            Message::PopupClosed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                }
            }
            Message::Surface(a) => {
                return cosmic::task::message(CosmicAction::Cosmic(cosmic::app::Action::Surface(
                    a,
                )));
            }
            Message::Sent => {}
            Message::Status(s) => {
                self.status = s;
            }
            Message::DaemonLost => {
                self.daemon_up = false;
            }
            Message::DaemonBack => {
                self.daemon_up = true;
                self.status = "Idle".to_string();
            }
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::run(ipc_event_subscription)
    }

    fn view(&self) -> Element<'_, Message> {
        let have_popup = self.popup;
        let btn = self
            .core
            .applet
            .icon_button(PANEL_ICON)
            .on_press_with_rectangle(move |offset: Vector, bounds: Rectangle| {
                if let Some(id) = have_popup {
                    Message::Surface(destroy_popup(id))
                } else {
                    Message::Surface(app_popup::<App>(
                        |_| cosmic::surface::action::LiveSettings::default(),
                        move |state: &mut App| {
                            let new_id = Id::unique();
                            state.popup = Some(new_id);
                            let mut popup_settings = state.core.applet.get_popup_settings(
                                state.core.main_window_id().unwrap_or(Id::RESERVED),
                                new_id,
                                None,
                                None,
                                None,
                            );
                            popup_settings.positioner.anchor_rect =
                                rect_from_bounds(offset, bounds);
                            popup_settings
                        },
                        Some(Box::new(move |state: &App| {
                            state.popup_content().map(CosmicAction::App)
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
        self.popup_content()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(applet::style())
    }
}

impl App {
    /// Build the popup body: a status line at the top, then a column of
    /// menu buttons with dividers between the capture / monitor /
    /// system groups. Mirrors the tray's menu structure item-for-item;
    /// each button emits a [`MenuMessage`] that [`menu::action_for`]
    /// resolves.
    ///
    /// When the daemon is absent (`daemon_up == false`), the status line
    /// is replaced with a *"daemon not running"* message and all
    /// daemon-dependent buttons are rendered without `on_press`
    /// (disabled). *Open config* stays enabled — it's a local action.
    fn popup_content(&self) -> Element<'_, Message> {
        let monitors = menu::monitor_entries();
        let up = self.daemon_up;

        let mut col = cosmic::widget::Column::new().push(status_line(&self.status, up));

        col = col
            .push(divider::horizontal::default())
            .push(action_btn("Capture region", MenuMessage::CaptureRegion, up))
            .push(action_btn(
                "Capture fullscreen",
                MenuMessage::CaptureFullscreen,
                up,
            ))
            .push(divider::horizontal::default());

        if monitors.is_empty() {
            col = col.push(disabled_item("no monitors"));
        } else {
            for (index, label) in monitors {
                col = col.push(action_btn(
                    label,
                    MenuMessage::CaptureFullscreenMonitor(index),
                    up,
                ));
            }
        }

        col = col
            .push(divider::horizontal::default())
            .push(action_btn("Open last", MenuMessage::OpenLast, up))
            .push(divider::horizontal::default())
            .push(action_btn("Open config", MenuMessage::OpenConfig, true))
            .push(action_btn("Quit", MenuMessage::Quit, up))
            .width(Length::Fill);

        Element::from(self.core.applet.popup_container(col))
    }
}

/// The popup status line. When the daemon is up, shows the current
/// status string (*Idle* / *Capturing…* / *Saved shot.png* / …). Error
/// statuses are tinted red so they stand out. When the daemon is down,
/// shows a *"daemon not running"* hint instead.
fn status_line<'a>(status: &str, daemon_up: bool) -> Element<'a, Message> {
    if daemon_up {
        let is_error = status.starts_with("Error:");
        let txt = text(status.to_string());
        if is_error {
            // Red tint for errors. cosmic::theme::Text implements
            // From<Color>, so .class() takes a color directly.
            txt.class(cosmic::iced::Color::from_rgb(0.86, 0.27, 0.27))
        } else {
            txt
        }
        .into()
    } else {
        text("daemon not running — start with `flintshot daemon`").into()
    }
}

/// A popup menu row that is clickable when `enabled` and non-interactive
/// when not. Wraps [`menu_btn`] / [`disabled_btn`] so [`popup_content`]
/// can pass the daemon-up flag without branching at every call site.
fn action_btn<'a>(
    label: impl Into<String>,
    msg: MenuMessage,
    enabled: bool,
) -> Element<'a, Message> {
    if enabled {
        menu_btn(label, msg)
    } else {
        disabled_btn(label)
    }
}

/// A single clickable popup menu row: `menu_button`-styled, presses emit
/// `msg`. Uses `applet::menu_button` for the COSMIC popup look.
fn menu_btn<'a>(label: impl Into<String>, msg: MenuMessage) -> Element<'a, Message> {
    applet::menu_button(text(label.into()))
        .on_press(Message::Menu(msg))
        .into()
}

/// A disabled popup menu row: `menu_button`-styled but without
/// `on_press`, so it renders greyed-out and produces no message. Used
/// for daemon-dependent actions when the daemon is absent.
fn disabled_btn<'a>(label: impl Into<String>) -> Element<'a, Message> {
    applet::menu_button(text(label.into())).into()
}

/// A disabled (non-interactive) menu row, used for the "no monitors"
/// placeholder. Visually matches [`menu_btn`] but produces no message.
fn disabled_item<'a>(label: impl Into<String>) -> Element<'a, Message> {
    applet::padded_control(text(label.into())).into()
}

/// Build the popup's `anchor_rect` from the button's offset + bounds.
/// `bounds` is `Rectangle<f32>` (iced default); the positioner wants
/// `Rectangle<i32>`. Screen coordinates are small positive f32s (low
/// thousands at most), so truncation to i32 is lossless in practice —
/// mirrors libcosmic's own applet examples.
#[allow(clippy::cast_possible_truncation)]
fn rect_from_bounds(offset: Vector, bounds: Rectangle) -> cosmic::iced::Rectangle<i32> {
    cosmic::iced::Rectangle {
        x: (bounds.x - offset.x) as i32,
        y: (bounds.y - offset.y) as i32,
        width: bounds.width as i32,
        height: bounds.height as i32,
    }
}

// ── event stream subscription ───────────────────────────────────────

/// Build the [`Subscription`] that drives the IPC event stream. Runs
/// [`ipc_event_loop`] in a background task on the applet's tokio
/// runtime; emitted [`Message`]s cross to the UI thread via iced's
/// subscription channel.
fn ipc_event_subscription() -> impl cosmic::iced::futures::Stream<Item = Message> {
    cosmic::iced::stream::channel(16, ipc_event_loop)
}

/// The persistent event-stream loop. Runs for the applet's lifetime:
///
/// 1. **Probe** the daemon. If `NotRunning`, emit [`DaemonLost`], sleep
///    `RECONNECT_INTERVAL`, retry.
/// 2. **Subscribe**. If the daemon accepts, emit [`DaemonBack`] and
///    enter the read loop. If subscribe fails (daemon stopped between
///    probe and connect), emit [`DaemonLost`] and retry after the
///    interval.
/// 3. **Read events**. Each [`Event`] frame maps to a [`Status`]
///    message via [`status_for_event`]. On stream-lost (`Ok(None)`) or
///    decode fault (`Err`), emit [`DaemonLost`] and break to the outer
///    loop for re-probe.
///
/// The daemon's broadcast channel handles `Lagged` subscribers
/// server-side (`ipc_server.rs:296`-area: logged + skipped), so the
/// client only ever sees well-formed `Event` frames or a clean stream
/// end — never a `Lagged` frame, never a panic.
///
/// Channel-send errors (receiver dropped = applet shutting down) are
/// silently ignored; the loop ends when the future is dropped.
///
/// [`DaemonLost`]: Message::DaemonLost
/// [`DaemonBack`]: Message::DaemonBack
/// [`Status`]: Message::Status
/// [`Event`]: flintshot_core::ipc::Event
async fn ipc_event_loop(mut sender: cosmic::iced::futures::channel::mpsc::Sender<Message>) {
    loop {
        match ipc::probe_daemon(None).await {
            DaemonState::NotRunning => {
                tracing::debug!("daemon absent; will re-probe");
                ignore_send(&mut sender, Message::DaemonLost).await;
                tokio::time::sleep(RECONNECT_INTERVAL).await;
                continue;
            }
            DaemonState::Running => {}
        }

        let mut stream = match ipc::subscribe(None).await {
            Ok(s) => {
                ignore_send(&mut sender, Message::DaemonBack).await;
                s
            }
            Err(e) => {
                tracing::debug!(error = %e, "subscribe failed after probe; will re-probe");
                ignore_send(&mut sender, Message::DaemonLost).await;
                tokio::time::sleep(RECONNECT_INTERVAL).await;
                continue;
            }
        };

        loop {
            match stream.next_event().await {
                Ok(Some(event)) => {
                    let status = status_for_event(&event);
                    tracing::debug!(status = %status, "event stream");
                    ignore_send(&mut sender, Message::Status(status)).await;
                }
                Ok(None) => {
                    tracing::debug!("event stream lost; will re-probe");
                    ignore_send(&mut sender, Message::DaemonLost).await;
                    tokio::time::sleep(RECONNECT_INTERVAL).await;
                    break;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "event stream decode fault; dropping");
                    ignore_send(&mut sender, Message::DaemonLost).await;
                    tokio::time::sleep(RECONNECT_INTERVAL).await;
                    break;
                }
            }
        }
    }
}

/// Send `msg` on `sender`, silently ignoring a send error (which means
/// the receiver — the applet runtime — has been dropped on shutdown).
async fn ignore_send(
    sender: &mut cosmic::iced::futures::channel::mpsc::Sender<Message>,
    msg: Message,
) {
    if sender.send(msg).await.is_err() {
        tracing::debug!("subscription channel closed; applet shutting down");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A headless `App`: `update` only touches `popup`/`status` and
    /// builds lazy `Task`s (the send future — settle sleep included —
    /// is never polled when the returned task is dropped unrun), so
    /// no display or runtime is needed.
    fn test_app(popup: Option<Id>) -> App {
        App {
            core: Core::default(),
            popup,
            status: "Idle".to_string(),
            daemon_up: true,
        }
    }

    #[test]
    fn capture_menu_click_closes_the_popup() {
        // The popup must be destroyed at the click — before the
        // (settled) request leaves — or it lands in the grab
        // (ADR-0041).
        let mut app = test_app(Some(Id::unique()));
        let _ = cosmic::Application::update(&mut app, Message::Menu(MenuMessage::CaptureRegion));
        assert!(app.popup.is_none(), "capture click must destroy the popup");
    }

    #[test]
    fn per_monitor_capture_click_closes_the_popup() {
        let mut app = test_app(Some(Id::unique()));
        let _ = cosmic::Application::update(
            &mut app,
            Message::Menu(MenuMessage::CaptureFullscreenMonitor(1)),
        );
        assert!(app.popup.is_none());
    }

    #[test]
    fn non_capture_click_keeps_the_popup() {
        // The popup's live status line is the feedback for
        // open/quit; only captures destroy it.
        let mut app = test_app(Some(Id::unique()));
        let _ = cosmic::Application::update(&mut app, Message::Menu(MenuMessage::OpenLast));
        assert!(app.popup.is_some());
    }

    #[test]
    fn capture_click_without_a_popup_is_inert() {
        let mut app = test_app(None);
        let _ =
            cosmic::Application::update(&mut app, Message::Menu(MenuMessage::CaptureFullscreen));
        assert!(app.popup.is_none());
    }
}
