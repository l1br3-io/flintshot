//! Shared error type for `flintshot-core`.
//!
//! Libraries return [`FlintshotError`] via [`Result`]; the binary shells
//! (`flintshot-app`, `flintshot-cosmic`) convert these into
//! `color_eyre::Report` at their boundaries.

use thiserror::Error;

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

    /// A catch-all for operations not yet wired up. Phase 0 uses this in
    /// stubs; later phases replace it with specific variants.
    #[error("not implemented yet: {0}")]
    NotImplemented(&'static str),
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, FlintshotError>;
