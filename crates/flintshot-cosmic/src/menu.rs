//! Popup menu body for the COSMIC applet.
//!
//! [`popup_view`] builds the popup's [`Element`] tree: the capture/pin/
//! config/quit buttons (mirroring the Phase 2 tray), a live per-monitor
//! enumeration (re-called on each popup-open), and a status-line slot
//! fed by the subscriber stream ([`crate::ipc`]).
//!
//! The buttons' enabled state is gated on
//! [`ConnectionState`](crate::app::ConnectionState): when the daemon is
//! unreachable ([`NoDaemon`](crate::app::ConnectionState::NoDaemon))
//! every capture/pin/config/quit action is rendered disabled (a cosmic
//! `widget::button` with no `on_press` is disabled by default — see
//! libcosmic `button/widget.rs` at the locked revision) and a single
//! *Start daemon* button + a "daemon not running" hint take their place.

use cosmic::iced::Length;
use cosmic::widget::{list_column, text};
use cosmic::Element;

use crate::app::{App, ConnectionState, Message};

/// Build the popup body. Called each time the popup is rendered (fresh
/// monitor enumeration on every call — cheaper than the tray's
/// build-time enum, and correct under hot-plug within a session).
///
/// The returned [`Element`] is wrapped in `popup_container` by the
/// caller (the `app_popup` view closure in [`App::view`](crate::app::App)).
#[must_use]
pub fn popup_view(app: &App) -> Element<'_, Message> {
    // Capture/pin/config/quit actions are only meaningful when the daemon
    // answers. When NoDaemon, the buttons are rendered without an
    // `on_press` — cosmic's `widget::button` treats that as its disabled
    // state (greyed, non-interactive), so the user sees the full menu
    // shape but can't fire a doomed IPC request.
    let connected = app.connection() == ConnectionState::Connected;

    let mut col = list_column()
        .add(
            list_column::button(text("Capture region"))
                .on_press_maybe(connected.then_some(Message::ClickRegion)),
        )
        .add(
            list_column::button(text("Capture fullscreen"))
                .on_press_maybe(connected.then_some(Message::ClickFullscreen)),
        );

    // Per-monitor entries: enumerate at popup-open time (fresher than
    // the tray's build-time snapshot). A disabled "no monitors" row
    // mirrors the tray's fallback.
    match flintshot_core::monitors() {
        Ok(list) if !list.is_empty() => {
            for m in list {
                let label = if m.is_primary {
                    format!("Monitor {} · {} (primary)", m.index, m.name)
                } else {
                    format!("Monitor {} · {}", m.index, m.name)
                };
                col = col.add(
                    list_column::button(text(label))
                        .on_press_maybe(connected.then_some(Message::ClickMonitor(m.index))),
                );
            }
        }
        _ => {
            col = col.add(text("no monitors"));
        }
    }

    col = col
        .add(
            list_column::button(text("Pin last"))
                .on_press_maybe(connected.then_some(Message::ClickPinLast)),
        )
        .add(
            list_column::button(text("Open config"))
                .on_press_maybe(connected.then_some(Message::ClickOpenConfig)),
        )
        .add(
            list_column::button(text("Quit"))
                .on_press_maybe(connected.then_some(Message::ClickQuit)),
        );

    // NoDaemon: the single recovery affordance. The hint + the *Start
    // daemon* button (the only enabled action in this state) replace the
    // dead capture surface. Clicking Start daemon spawns the daemon; the
    // subscriber's backoff loop reconnects and emits DaemonUp, flipping
    // back to Connected without an applet restart.
    if !connected {
        col = col
            .add(text("daemon not running").width(Length::Fill).size(12))
            .add(list_column::button(text("Start daemon")).on_press(Message::ClickStartDaemon));
    }

    // Live status line: populated by the subscriber stream (Task 004);
    // also carries *Start daemon* outcomes ("starting daemon…" / spawn
    // errors). Empty until the first message lands — no visible line.
    if !app.last_event_text().is_empty() {
        col = col.add(text(app.last_event_text()).width(Length::Fill).size(12));
    }

    col.into()
}
