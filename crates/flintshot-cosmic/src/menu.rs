//! Pure popup-menu dispatch for the COSMIC applet.
//!
//! This module is the headless-testable mirror of the tray's
//! `action_for` (`flintshot-app::tray`): it maps a popup-menu click to
//! a daemon [`Request`] sent via [`crate::ipc::send_request`]. Keeping
//! the mapping pure lets the acceptance tests exercise every menu
//! item without a display or a running daemon; the libcosmic wiring
//! in [`crate::app`] just feeds button presses in and performs the
//! resulting actions.
//!
//! Mirrors the tray menu set (settings-improvement): one **Capture**
//! action, *Open last*, *Settings*, *Quit*. The former local
//! *Open config* item is gone — the settings page is the single
//! preferences surface, reached through the daemon. See ADR-0005
//! (applet is a pure IPC client).

use flintshot_core::ipc::Request;

/// A popup-menu click, before it is resolved to an [`Action`].
///
/// One variant per button in the popup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MenuMessage {
    /// *Capture* — the single Capture action: the OS's own selection
    /// UI where the backend supports it, else the editor flow.
    Capture,
    /// *Open last* — re-open the most recently saved capture.
    OpenLast,
    /// *Settings* — open (or focus) the app window on its settings
    /// page.
    Settings,
    /// *Quit* — tell the daemon to stop.
    Quit,
}

/// What a [`MenuMessage`] should trigger: a [`Request`] for the
/// daemon over IPC. Every menu item resolves to one — the local
/// open-config side-effect is gone with the settings swap.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Action {
    /// Send this request via [`crate::ipc::send_request`].
    Request(Request),
}

/// Map a popup-menu click to the action it triggers. Mirrors
/// `flintshot-app::tray::action_for` item-for-item. Every variant
/// resolves to a concrete [`Action`]; the match is exhaustive within
/// this crate, so adding a future variant is a compile error here
/// (forcing a conscious decision) rather than a silent `None`.
#[must_use]
pub fn action_for(message: MenuMessage) -> Action {
    let request = match message {
        MenuMessage::Capture => Request::Capture { delay_ms: 0 },
        MenuMessage::OpenLast => Request::OpenLast,
        MenuMessage::Settings => Request::OpenSettings,
        MenuMessage::Quit => Request::Quit,
    };
    Action::Request(request)
}

/// Does acting on this [`Request`] spawn a capture worker (anything
/// that freezes the screen)? Mirrors
/// `flintshot_app::worker::spawns_capture_worker`; the applet cannot
/// depend on the daemon's bin crate, so the match is replicated here
/// and pinned to the same request set by test. For exactly these the
/// applet destroys its popup and waits out the settle window in
/// `app::POPUP_CLOSE_SETTLE` before sending, so its own popup never
/// lands in the grabbed frame (ADR-0041).
#[must_use]
pub fn request_spawns_capture(request: &Request) -> bool {
    matches!(request, Request::Capture { .. })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── action_for: the menu-item → Action mapping ──────────────────

    #[test]
    fn capture_maps_to_capture_request() {
        assert!(matches!(
            action_for(MenuMessage::Capture),
            Action::Request(Request::Capture { delay_ms: 0 })
        ));
    }

    #[test]
    fn open_last_maps_to_request() {
        assert_eq!(
            action_for(MenuMessage::OpenLast),
            Action::Request(Request::OpenLast)
        );
    }

    /// settings-improvement: the *Settings* item sends the same
    /// `OpenSettings` request the tray sends — the daemon's
    /// broadcast + spawn opens (or focuses) the window on Settings.
    #[test]
    fn settings_maps_to_open_settings_request() {
        assert_eq!(
            action_for(MenuMessage::Settings),
            Action::Request(Request::OpenSettings)
        );
    }

    #[test]
    fn quit_maps_to_request() {
        assert_eq!(
            action_for(MenuMessage::Quit),
            Action::Request(Request::Quit)
        );
    }

    #[test]
    fn request_spawns_capture_is_true_only_for_captures() {
        let captures = [
            Request::Capture { delay_ms: 0 },
            Request::Capture { delay_ms: 500 },
        ];
        for request in &captures {
            assert!(
                request_spawns_capture(request),
                "{request:?} freezes the screen: the popup must close first"
            );
        }
        let others = [
            Request::OpenLast,
            Request::Quit,
            // Opening a window never grabs the screen.
            Request::OpenSettings,
        ];
        for request in &others {
            assert!(
                !request_spawns_capture(request),
                "{request:?} never grabs the screen: the popup stays for its status line"
            );
        }
    }
}
