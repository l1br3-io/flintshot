//! Pure popup-menu dispatch for the COSMIC applet.
//!
//! This module is the headless-testable mirror of the tray's
//! `action_for` (`flintshot-app::tray`): it maps a popup-menu click to
//! either a daemon [`Request`] (sent via [`crate::ipc::send_request`])
//! or a local "open config" side-effect. Keeping the mapping pure lets
//! the acceptance tests exercise every menu item without a display
//! or a running daemon; the libcosmic wiring in [`crate::app`] just
//! feeds button presses in and performs the resulting actions.
//!
//! Mirrors the tray menu set (native-capture-ux): one **Capture**
//! action, *Open last*, *Open config*, *Quit*. See ADR-0005 (applet
//! is a pure IPC client) and the Phase 3 plan's "applet shells out to
//! `$EDITOR`" decision."

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

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
    /// *Open config* — edit `config.toml` in `$EDITOR`.
    OpenConfig,
    /// *Quit* — tell the daemon to stop.
    Quit,
}

/// What a [`MenuMessage`] should trigger: send a [`Request`] to the
/// daemon over IPC, or perform a local side-effect.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Action {
    /// Send this request via [`crate::ipc::send_request`].
    Request(Request),
    /// Open `config.toml` in `$EDITOR` (see [`open_config`]).
    OpenConfig,
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
        MenuMessage::Quit => Request::Quit,
        MenuMessage::OpenConfig => return Action::OpenConfig,
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

/// Build the `$EDITOR <path>` command for opening the config file.
/// Returns `None` when `editor` is empty (no `EDITOR`/`VISUAL` set);
/// the caller prints the path instead. Factored out of [`open_config`]
/// so the command shape is unit-testable without spawning a process.
#[must_use]
pub fn editor_command(path: &Path, editor: OsString) -> Option<Command> {
    if editor.is_empty() {
        return None;
    }
    let mut cmd = Command::new(editor);
    cmd.arg(path);
    Some(cmd)
}

/// Open `config.toml` in `$EDITOR` (fallback `$VISUAL`); print the path
/// if neither is set. Mirrors `flintshot-app::daemon::open_config_file`
/// (`daemon.rs:254`) ~10 LOC — the applet is a pure IPC client and
/// intentionally does *not* add an IPC variant for this (see plan §Q5).
///
/// # Errors
/// - `dirs::config_dir()` returns `None` on this platform.
/// - `$EDITOR`/`$VISUAL` is set but the editor fails to spawn.
pub fn open_config() -> color_eyre::eyre::Result<()> {
    let path = flintshot_core::config_path()
        .ok_or_else(|| color_eyre::eyre::eyre!("no platform config_dir available"))?;
    let editor = std::env::var_os("EDITOR").or_else(|| std::env::var_os("VISUAL"));
    if let Some(mut cmd) = editor_command(&path, editor.unwrap_or_default()) {
        let program = cmd.get_program().to_os_string();
        cmd.spawn().map_err(|e| {
            color_eyre::eyre::eyre!("launching {program:?} on {}: {e}", path.display())
        })?;
        tracing::info!(path = %path.display(), "opened config via $EDITOR");
    } else {
        println!("{}", path.display());
        tracing::info!(path = %path.display(), "no $EDITOR/$VISUAL; printed config path");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::PathBuf;

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

    #[test]
    fn open_config_maps_directly() {
        assert_eq!(action_for(MenuMessage::OpenConfig), Action::OpenConfig);
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
        let others = [Request::OpenLast, Request::Quit];
        for request in &others {
            assert!(
                !request_spawns_capture(request),
                "{request:?} never grabs the screen: the popup stays for its status line"
            );
        }
    }

    // ── editor_command: pure command builder ────────────────────────

    #[test]
    fn editor_command_builds_editor_with_path_arg() {
        let path = PathBuf::from("/home/u/.config/flintshot/config.toml");
        let cmd =
            editor_command(&path, OsString::from("nano")).expect("editor set → Some(Command)");
        assert_eq!(cmd.get_program(), OsString::from("nano"));
        let args: Vec<OsString> = cmd.get_args().map(OsString::from).collect();
        assert_eq!(args, [OsString::from(path.as_os_str())]);
    }

    #[test]
    fn editor_command_returns_none_for_empty_editor() {
        let path = PathBuf::from("/x/config.toml");
        assert!(editor_command(&path, OsString::new()).is_none());
    }
}
