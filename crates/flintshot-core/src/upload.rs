//! Pluggable upload backends.
//!
//! The [`UploadBackend`] trait is defined in Phase 0 (see ADR-0004) but
//! the first concrete implementation (`ImgurBackend`) ships in Phase 4.
//! Credentials, progress, and deletion are first-class so we don't have
//! to rewrite the trait when the second backend arrives.

use serde::{Deserialize, Serialize};

/// Supported image MIME types for upload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ImageMime {
    /// `image/png`
    Png,
    /// `image/jpeg`
    Jpeg,
    /// `image/webp`
    Webp,
}

impl ImageMime {
    /// The canonical MIME type string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
        }
    }
}

/// Per-upload runtime context: where credentials live, what filename to
/// suggest, and a channel for reporting progress.
///
/// Phase 0 defines the shape; Phase 4 wires real `keyring::Entry` and
/// `tokio::sync::watch::Sender`. Until then the fields use stub types
/// so the trait compiles without pulling those deps into core.
pub struct UploadContext<'a> {
    /// Opaque credential-store handle. Phase 4 replaces this with
    /// `&'a keyring::Entry`.
    pub credentials: CredentialHandle<'a>,
    /// Optional filename hint for the upload.
    pub filename: Option<&'a str>,
    /// Progress reporter in `[0.0, 1.0]`. Phase 4 wires a `watch::Sender`.
    pub progress: ProgressSink<'a>,
}

/// Placeholder for a `keyring::Entry` reference. Phase 4 swaps in the
/// real type; for Phase 0 it carries a backend identifier string only.
#[derive(Debug, Clone, Copy)]
pub struct CredentialHandle<'a> {
    /// Backend identifier this entry belongs to (`"imgur"`, etc.).
    pub backend: &'a str,
}

/// Placeholder for a progress channel. Phase 4 replaces this with a
/// `&'a mut tokio::sync::watch::Sender<f32>`.
#[derive(Debug)]
pub struct ProgressSink<'a> {
    /// Last reported progress fraction in `[0.0, 1.0]`.
    pub last: &'a mut f32,
}

/// Successful upload result.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UploadOutcome {
    /// Public URL of the uploaded image.
    pub url: String,
    /// Backend-specific handle that allows later deletion (e.g. Imgur
    /// delete-hash). `None` if the backend doesn't support deletion.
    pub delete_handle: Option<String>,
}

/// Errors returned by upload backends.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum UploadError {
    /// HTTP/transport failure.
    #[error("transport error: {0}")]
    Transport(String),

    /// Authentication failure (expired token, bad credentials).
    #[error("auth error: {0}")]
    Auth(String),

    /// The backend rejected the upload (rate limit, bad request, etc.).
    #[error("backend rejected upload ({status}): {message}")]
    Backend {
        /// HTTP status code (or 0 if not applicable).
        status: u16,
        /// Backend-specific error message.
        message: String,
    },

    /// The selected backend is unknown / not built.
    #[error("unknown backend: {0}")]
    UnknownBackend(String),
}

/// Trait implemented by each upload provider (Imgur, 0x0, SFTP, ...).
///
/// Backends are `Send + Sync` so they can live behind an `Arc<dyn>` in
/// the daemon. Phase 4 ships `ImgurBackend`.
pub trait UploadBackend: Send + Sync {
    /// Stable machine identifier (`"imgur"`). Used in CLI flags and config.
    fn id(&self) -> &'static str;

    /// Human-readable name (`"Imgur"`). Used in UI.
    fn name(&self) -> &'static str;

    /// Upload the encoded image bytes.
    ///
    /// # Errors
    /// See [`UploadError`] variants.
    fn upload(
        &self,
        image_bytes: &[u8],
        mime: ImageMime,
        ctx: &UploadContext<'_>,
    ) -> impl std::future::Future<Output = Result<UploadOutcome, UploadError>> + Send;

    /// Delete a previously-uploaded resource, if the backend supports it.
    ///
    /// Default implementation returns `Ok(())` so backends that don't
    /// support deletion can omit the method.
    ///
    /// # Errors
    /// See [`UploadError`] variants.
    // NOTE: `path` is unused in Phase 0 (no impls); kept on the signature
    // so future implementors have a clear contract without a revision.
    fn delete(
        &self,
        _handle: &str,
    ) -> impl std::future::Future<Output = Result<(), UploadError>> + Send {
        async { Ok(()) }
    }
}
