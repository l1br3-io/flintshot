//! Popup menu body for the COSMIC applet.
//!
//! [`popup_view`] builds the popup's [`Element`] tree: the capture/pin/
//! config/quit buttons (mirroring the Phase 2 tray), a live per-monitor
//! enumeration (re-called on each popup-open), and a status-line slot
//! that Task 004 fills from the subscriber stream.
//!
//! The buttons are enabled unconditionally here — Task 005 wires
//! [`ConnectionState`](crate::app::ConnectionState) into their `enabled`
//! flags for the daemon-absent UX. See `// TODO(005):` markers below.

use cosmic::iced::Length;
use cosmic::widget::{list_column, text};
use cosmic::Element;

use crate::app::{App, Message};

/// Build the popup body. Called each time the popup is rendered (fresh
/// monitor enumeration on every call — cheaper than the tray's
/// build-time enum, and correct under hot-plug within a session).
///
/// The returned [`Element`] is wrapped in `popup_container` by the
/// caller (the `app_popup` view closure in [`App::view`](crate::app::App)).
#[must_use]
pub fn popup_view(app: &App) -> Element<'_, Message> {
    // TODO(005): gate capture/pin buttons on ConnectionState::Connected;
    // show a hint + *Start daemon* when NoDaemon.
    let mut col = list_column()
        .add(list_column::button(text("Capture region")).on_press(Message::ClickRegion))
        .add(list_column::button(text("Capture fullscreen")).on_press(Message::ClickFullscreen));

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
                col = col
                    .add(list_column::button(text(label)).on_press(Message::ClickMonitor(m.index)));
            }
        }
        _ => {
            col = col.add(text("no monitors"));
        }
    }

    col = col
        .add(list_column::button(text("Pin last")).on_press(Message::ClickPinLast))
        .add(list_column::button(text("Open config")).on_press(Message::ClickOpenConfig))
        .add(list_column::button(text("Quit")).on_press(Message::ClickQuit));

    // TODO(004): wire to the live subscriber stream. Empty string → no
    // visible line this task.
    if !app.last_event_text().is_empty() {
        col = col.add(text(app.last_event_text()).width(Length::Fill).size(12));
    }

    col.into()
}
