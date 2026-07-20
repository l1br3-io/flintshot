//! Shared error type for `flintshot-core`.
//!
//! Libraries return [`FlintshotError`] via [`Result`]; the binary shells
//! (`flintshot-app`, `flintshot-cosmic`) convert these into
//! `color_eyre::Report` at their boundaries.

use std::path::PathBuf;

use thiserror::Error;

use crate::geom::Rect;

/// Top-level error type for all fallible core operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FlintshotError {
    /// Image decode/encode/resample failure, surfaced from the `image` crate.
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),

    /// An I/O error from the filesystem or a socket.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Config file failed to load, parse, or serialize. `path` is the
    /// file we were operating on (empty when the failure is not tied
    /// to a specific file, e.g. an in-memory serialization).
    #[error("config error at {path}: {detail}")]
    Config {
        /// File whose load/save failed. Empty for in-memory failures.
        path: PathBuf,
        /// Lower-level error description.
        detail: String,
    },

    /// Screen capture failed (xcap returned an error, monitor selection
    /// was out of range, or the platform denied permission). `detail`
    /// is the lower-level message verbatim — on macOS permission
    /// denials, it contains the substring `"permission"` so callers
    /// can detect that case and guide the user. (Field is named
    /// `detail` rather than `source` because `thiserror` treats a
    /// field literally named `source` as the error-chain source and
    /// requires it to be `dyn Error`; we want a plain `String` for
    /// easy substring matching.)
    #[error("capture error: {detail}")]
    Capture {
        /// Lower-level error message from `xcap` or our monitor picker.
        detail: String,
    },

    /// A rectangle passed to [`crate::capture::capture_region`] (or any
    /// other rect-consuming API) was degenerate: zero-area or with
    /// corners in the wrong order. Carries the offending rect so the
    /// caller can include it in user-facing diagnostics.
    #[error("invalid rect ({reason}): {rect:?}")]
    InvalidRect {
        /// The rect that was rejected.
        rect: Rect,
        /// Short fixed keyword: `"empty"` for zero-area,
        /// `"negative"` for inverted corners.
        reason: &'static str,
    },

    /// The editor shell (window system, GPU surface, or runtime) failed
    /// to start or sustained a fatal error mid-session. `detail` carries
    /// the lower-level message verbatim. Used for `eframe::Error` and any
    /// future toolkit-side failures (Phase 3 pin window, etc.).
    ///
    /// Named `detail` rather than `source` for the same reason as
    /// [`FlintshotError::Capture`]: `thiserror` treats a field literally
    /// named `source` as the error-chain source and requires it to be
    /// `dyn Error`, but we want a plain `String` for cheap display.
    #[error("editor shell error: {detail}")]
    Shell {
        /// Lower-level error message from the shell or window system.
        detail: String,
    },

    /// A catch-all for operations not yet wired up. Phase 0 uses this in
    /// stubs; later phases replace it with specific variants.
    #[error("not implemented yet: {0}")]
    NotImplemented(&'static str),
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, FlintshotError>;
