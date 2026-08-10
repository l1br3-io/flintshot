//! Pure popup-menu dispatch for the COSMIC applet.
//!
//! This module is the headless-testable mirror of the tray's
//! `action_for` (`flintshot-app::tray`): it maps a popup-menu click to
//! either a daemon [`Request`] (sent via [`crate::ipc::send_request`])
//! or a local "open config" side-effect. Keeping the mapping pure lets
//! Task 7's acceptance test exercise every menu item without a display
//! or a running daemon; the libcosmic wiring in [`crate::app`] just
//! feeds button presses in and performs the resulting actions.
//!
//! Mirrors the tray menu set: *Capture region*, *Capture fullscreen*,
//! *Fullscreen on monitor →* (per-monitor submenu), *Pin last*,
//! *Open last*, *Open config*, *Quit*. See ADR-0005 (applet is a pure
//! IPC client) and the Phase 3 plan's "applet shells out to `$EDITOR`"
//! decision.

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

use flintshot_core::ipc::Request;
use flintshot_core::MonitorInfo;

/// A popup-menu click, before it is resolved to an [`Action`].
///
/// One variant per button in the popup. The per-monitor submenu carries
/// the monitor index; the applet builds the submenu entries from
/// [`monitor_entries`] and emits this variant on click.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MenuMessage {
    /// *Capture region* — interactive selection on the primary monitor.
    CaptureRegion,
    /// *Capture fullscreen* — primary monitor.
    CaptureFullscreen,
    /// *Fullscreen on monitor →* submenu, entry `index`.
    CaptureFullscreenMonitor(u32),
    /// *Pin last* — pin the most recently saved capture.
    PinLast,
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
/// `flintshot-app::tray::action_for` item-for-item. Every [`MenuMessage`]
/// variant resolves to a concrete [`Action`]; the match is exhaustive
/// within this crate, so adding a future variant is a compile error
/// here (forcing a conscious decision) rather than a silent `None`.
#[must_use]
pub fn action_for(message: &MenuMessage) -> Action {
    let request = match message {
        MenuMessage::CaptureRegion => Request::CaptureRegion {
            monitor: None,
            delay_ms: 0,
        },
        MenuMessage::CaptureFullscreen => Request::CaptureFullscreen { monitor: None },
        MenuMessage::CaptureFullscreenMonitor(index) => Request::CaptureFullscreen {
            monitor: Some(*index),
        },
        MenuMessage::PinLast => Request::PinLast,
        MenuMessage::OpenLast => Request::OpenLast,
        MenuMessage::Quit => Request::Quit,
        MenuMessage::OpenConfig => return Action::OpenConfig,
    };
    Action::Request(request)
}

/// Human label for a monitor entry in the *Fullscreen on monitor →*
/// submenu: `"<N> · <name>"`, or `"<N> · <name> (primary)"` for the
/// primary monitor. Mirrors `flintshot-app::tray`'s label format
/// (`tray.rs:117`) so the applet and tray read identically.
#[must_use]
pub fn monitor_label(monitor: &MonitorInfo) -> String {
    monitor_label_parts(monitor.index, &monitor.name, monitor.is_primary)
}

/// Pure label builder behind [`monitor_label`]. Factored out so the
/// format is unit-testable without constructing a [`MonitorInfo`] (which
/// is `#[non_exhaustive]` with geometry fields this crate can't set).
#[must_use]
fn monitor_label_parts(index: u32, name: &str, is_primary: bool) -> String {
    if is_primary {
        format!("{index} · {name} (primary)")
    } else {
        format!("{index} · {name}")
    }
}

/// All monitor entries for the submenu, as `(index, label)` pairs.
/// Returns an empty vector if `flintshot_core::monitors()` errors or
/// reports no monitors — the popup then shows a disabled "no monitors"
/// placeholder instead.
#[must_use]
pub fn monitor_entries() -> Vec<(u32, String)> {
    match flintshot_core::monitors() {
        Ok(list) if !list.is_empty() => list.iter().map(|m| (m.index, monitor_label(m))).collect(),
        _ => Vec::new(),
    }
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
    fn capture_region_maps_to_default_monitor_request() {
        let Action::Request(Request::CaptureRegion { monitor, delay_ms }) =
            action_for(&MenuMessage::CaptureRegion)
        else {
            panic!("expected CaptureRegion request");
        };
        assert!(monitor.is_none(), "popup region uses the default monitor");
        assert_eq!(delay_ms, 0);
    }

    #[test]
    fn capture_fullscreen_maps_to_default_monitor_request() {
        assert!(matches!(
            action_for(&MenuMessage::CaptureFullscreen),
            Action::Request(Request::CaptureFullscreen { monitor: None })
        ));
    }

    #[test]
    fn capture_fullscreen_monitor_carries_the_index() {
        assert_eq!(
            action_for(&MenuMessage::CaptureFullscreenMonitor(3)),
            Action::Request(Request::CaptureFullscreen { monitor: Some(3) })
        );
    }

    #[test]
    fn pin_last_maps_to_request() {
        assert_eq!(
            action_for(&MenuMessage::PinLast),
            Action::Request(Request::PinLast)
        );
    }

    #[test]
    fn open_last_maps_to_request() {
        assert_eq!(
            action_for(&MenuMessage::OpenLast),
            Action::Request(Request::OpenLast)
        );
    }

    #[test]
    fn open_config_maps_directly() {
        assert_eq!(action_for(&MenuMessage::OpenConfig), Action::OpenConfig);
    }

    #[test]
    fn quit_maps_to_request() {
        assert_eq!(
            action_for(&MenuMessage::Quit),
            Action::Request(Request::Quit)
        );
    }

    // ── monitor_label_parts: submenu text (pure, no MonitorInfo) ────

    #[test]
    fn monitor_label_non_primary_is_index_dot_name() {
        assert_eq!(monitor_label_parts(2, "HDMI-A-1", false), "2 · HDMI-A-1");
    }

    #[test]
    fn monitor_label_primary_appends_primary_marker() {
        assert_eq!(monitor_label_parts(0, "eDP-1", true), "0 · eDP-1 (primary)");
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
