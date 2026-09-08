//! Toolkit lifecycle trait — "run the editor (or pin window), return the outcome".
//!
//! Shells (egui, iced, etc.) implement [`EditorShell`]. The binary picks
//! the implementation via Cargo feature flag.

use std::path::PathBuf;

use crate::{editor::EditorState, error::Result, geom::Rect};

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
    /// User pinned the result to the desktop (a `flintshot pin` window
    /// was spawned from the editor). The pin worker reads the image
    /// from a temp file; the editor closes immediately after spawning.
    Pinned,
    /// User cancelled (`Esc`, window close, etc.).
    Cancelled,
    /// User asked for a fresh capture from inside the editor (the
    /// `+` chip / Ctrl+N, Win11-Snipping-Tool-style): the window has
    /// closed — so it cannot be in the new shot — and the caller
    /// should re-run the capture+editor cycle. Unsaved annotations
    /// are discarded, Win11-style.
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

/// Toolkit lifecycle: open the editor (or a pin window), block until the
/// user is done, return the outcome.
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

    /// Open a pin-to-desktop window showing `image` at `rect`.
    ///
    /// Blocks until the user closes the pin (via `Esc`, the WM close
    /// button, or process termination). The pin is a borderless
    /// always-on-top floating window sized to the image; the user can
    /// drag it around the desktop. Multiple pins coexist as separate
    /// processes (the caller spawns one `run_pin` call per pin).
    ///
    /// **`rect` semantics:** `rect`'s dimensions are advisory (the
    /// image's own dimensions are the source of truth for window
    /// size); `rect.min` carries the desired screen position when the
    /// caller knows it. A default `Rect` (`min == (0, 0)`) signals "let
    /// the WM place the window" — the implementation skips positioning
    /// in that case. Both current callers (`flintshot pin <path>` and the
    /// editor's Pin button) pass a default `Rect`; the Phase 5
    /// live overlay will pass a real screen position so a pin opens
    /// where the capture was taken.
    ///
    /// # Errors
    /// Surfaces [`crate::FlintshotError`] for window-system or image failures.
    fn run_pin(&self, image: image::RgbaImage, rect: Rect) -> Result<()>;
}
