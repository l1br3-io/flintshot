//! Toolkit lifecycle trait — "run the editor, return the outcome".
//!
//! Shells (egui, iced, etc.) implement [`EditorShell`]. The binary picks
//! the implementation via Cargo feature flag.

use std::path::PathBuf;

use crate::{editor::EditorState, error::Result};

/// What happened when the user closed the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EditorOutcome {
    /// User saved to disk; contains the chosen path.
    Saved(PathBuf),
    /// User copied the result to the clipboard.
    CopiedToClipboard,
    /// An upload was initiated: the editor spawned a `flintshot
    /// upload` worker and closed. The string is a placeholder (empty)
    /// — the editor exits before the upload completes, so the real URL
    /// is not known here; subscribers receive it via the worker's
    /// `Request::ReportUploaded` report to the daemon.
    Uploaded(String),
    /// User cancelled (`Esc`, window close, etc.).
    Cancelled,
    /// User asked for a fresh capture from inside the editor (the
    /// `+` chip / Ctrl+N, Win11-Snipping-Tool-style): the window has
    /// closed — so it cannot be in the new shot — and the caller
    /// should re-run the capture+editor cycle. Unsaved annotations
    /// are the *host's* to preserve: page-mode hosts stash them
    /// (the session-captures store) before acting; only hosts
    /// without a stash still lose them.
    NewCapture {
        /// Which capture kind to start next.
        mode: NewCaptureMode,
    },
}

/// What a "new capture" request targets (see
/// [`EditorOutcome::NewCapture`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum NewCaptureMode {
    /// Interactive region selection over a fresh freeze-frame.
    Region,
    /// Whole-monitor grab.
    Fullscreen,
}

/// Toolkit lifecycle: open the editor, block until the user is done,
/// return the outcome.
///
/// Implementations own the event loop and any toolkit-specific
/// resources (texture handles, theme). State mutations happen inside
/// the implementation by feeding input events into [`EditorState`].
pub trait EditorShell {
    /// Open the editor with `state` and block until the user finishes.
    ///
    /// # Errors
    /// Surfaces [`crate::FlintshotError`] for window-system, image, or
    /// IO failures.
    fn run_editor(&self, state: EditorState) -> Result<EditorOutcome>;
}
