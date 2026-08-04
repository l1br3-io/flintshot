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
    /// User pinned the composed image to the desktop; contains the
    /// pin-cache path the editor wrote (not the user's save path). The
    /// pin persists as its own process — the editor spawns
    /// `flintshot pin <path>` detached and exits, leaving the pin window
    /// on screen. See ADR-0023 (pin-as-process).
    Pinned(PathBuf),
    /// User copied the result to the clipboard.
    CopiedToClipboard,
    /// User uploaded and got back a URL.
    Uploaded(String),
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
    /// in that case. Phase 3's callers (`flintshot pin <path>` and the
    /// editor's Pin button) both pass a default `Rect`; the Phase 5
    /// live overlay will pass a real screen position so a pin opens
    /// where the capture was taken.
    ///
    /// # Errors
    /// Surfaces [`crate::FlintshotError`] for window-system or image failures.
    fn run_pin(&self, image: image::RgbaImage, rect: Rect) -> Result<()>;
}
