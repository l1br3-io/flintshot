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
    /// User uploaded and got back a URL.
    Uploaded(String),
    /// User pinned the result to the desktop (a `flintshot pin` window
    /// was spawned from the editor). The pin worker reads the image
    /// from a temp file; the editor closes immediately after spawning.
    Pinned,
    /// User cancelled (`Esc`, window close, etc.).
    Cancelled,
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
    /// Blocks until the user closes the pin.
    ///
    /// # Errors
    /// Surfaces [`crate::FlintshotError`] for window-system or image failures.
    fn run_pin(&self, image: image::RgbaImage, rect: Rect) -> Result<()>;
}
