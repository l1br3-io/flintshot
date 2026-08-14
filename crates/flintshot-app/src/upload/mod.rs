//! Upload backend registry + concrete implementations.
//!
//! [`resolve_backend`] maps a backend id (from CLI `--backend` or
//! `[upload] default-backend`) to an `Arc<dyn UploadBackend>`. Phase 4
//! ships [`ImgurBackend`] (see [`imgur`]); the registry is the single
//! place to add a new provider.

use std::sync::Arc;

use flintshot_core::{UploadBackend, UploadError};

pub mod imgur;

pub use imgur::ImgurBackend;

/// Resolve a backend id to its implementation.
///
/// # Errors
/// [`UploadError::UnknownBackend`] when `id` does not match a built-in
/// backend.
pub fn resolve_backend(id: &str) -> Result<Arc<dyn UploadBackend>, UploadError> {
    match id {
        "imgur" => Ok(Arc::new(ImgurBackend::new())),
        other => Err(UploadError::UnknownBackend(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_imgur_returns_backend() {
        let backend = resolve_backend("imgur").expect("imgur is built-in");
        assert_eq!(backend.id(), "imgur");
        assert_eq!(backend.name(), "Imgur");
    }

    #[test]
    fn resolve_unknown_backend_errors() {
        match resolve_backend("nope") {
            Err(UploadError::UnknownBackend(n)) => assert_eq!(n, "nope"),
            Err(other) => panic!("expected UnknownBackend, got {other:?}"),
            Ok(_) => panic!("expected error for unknown backend id"),
        }
    }
}
