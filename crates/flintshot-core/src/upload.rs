//! Pluggable upload backends.
//!
//! The [`UploadBackend`] trait is defined in Phase 0 (see ADR-0004). The
//! first concrete implementation (`ImgurBackend`) ships in Phase 4.
//! Credentials are a direct `keyring::Entry` reference (not a trait
//! object — see ADR-0024); deletion is on the trait but unused in
//! Phase 4.

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

/// Per-upload runtime context: where credentials live and what filename
/// to suggest.
///
/// `credentials` is a direct `Option<&keyring::Entry>` reference —
/// `None` for anonymous mode (no keychain lookup), `Some(entry)` when
/// the caller resolved a credential for this backend. `keyring` is a
/// core dep because it appears as a parameter type here; it's not a GUI
/// toolkit (it's a secrets handle), so `no_gui_types_in_core.rs` stays
/// green. See ADR-0024.
pub struct UploadContext<'a> {
    /// OS keychain entry the backend reads credentials from. `None`
    /// for anonymous mode.
    pub credentials: Option<&'a keyring::Entry>,
    /// Optional filename hint sent to the backend.
    pub filename: Option<&'a str>,
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
    fn delete(
        &self,
        _handle: &str,
    ) -> impl std::future::Future<Output = Result<(), UploadError>> + Send {
        async { Ok(()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_context_anonymous_has_no_credentials() {
        let ctx = UploadContext {
            credentials: None,
            filename: Some("shot.png"),
        };
        assert!(ctx.credentials.is_none());
        assert_eq!(ctx.filename, Some("shot.png"));
    }

    #[test]
    fn upload_context_accepts_keyring_entry_reference() {
        let entry = keyring::Entry::new("flintshot", "imgur:client-id")
            .expect("Entry::new creates a handle without touching the keychain");
        let ctx = UploadContext {
            credentials: Some(&entry),
            filename: None,
        };
        assert!(ctx.credentials.is_some());
        assert!(ctx.filename.is_none());
    }
}
