//! Imgur upload backend — the first concrete [`UploadBackend`] impl.
//!
//! POSTs multipart form data to the Imgur v3 API (`POST /3/image`).
//! Supports anonymous mode (no auth header) and Client-ID authenticated
//! mode (`Authorization: Client-ID <id>` read from a [`keyring::Entry`]
//! via [`UploadContext::credentials`]).
//!
//! Response mapping:
//!
//! | HTTP status          | `UploadError` variant |
//! | -------------------- | --------------------- |
//! | 2xx                  | `Ok(UploadOutcome)`   |
//! | 401, 403             | `Auth`                |
//! | 429, other non-2xx   | `Backend`             |
//! | connect / body fault | `Transport`           |
//!
//! See ADR-0026 (Task 010) for the `reqwest` choice.
//!
//! [`UploadBackend`]: flintshot_core::UploadBackend

use std::future::Future;
use std::pin::Pin;

use flintshot_core::{ImageMime, UploadBackend, UploadContext, UploadError, UploadOutcome};
use serde::Deserialize;

/// Canonical Imgur v3 API base.
const IMGUR_API_BASE: &str = "https://api.imgur.com";

/// Imgur upload backend.
///
/// Construct with [`ImgurBackend::new`] (real API) or
/// [`ImgurBackend::with_base_url`] (test seam for mock servers).
pub struct ImgurBackend {
    http: reqwest::Client,
    base_url: String,
}

impl ImgurBackend {
    /// Create a backend targeting the real Imgur API.
    #[must_use]
    pub fn new() -> Self {
        Self::with_base_url(IMGUR_API_BASE)
    }

    /// Create a backend with a custom API base URL.
    ///
    /// The `/3/image` path is appended at request time; pass only the
    /// origin (e.g. `https://api.imgur.com` or a mock-server URL).
    #[must_use]
    pub fn with_base_url(base_url: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.to_string(),
        }
    }
}

impl Default for ImgurBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// Deserialized Imgur v3 response envelope. Fields are optional so a
/// single type covers both success (`data.link`, `data.deletehash`) and
/// error (`data.error`) shapes.
#[derive(Debug, Deserialize)]
struct ImgurResponse {
    data: Option<ImgurData>,
}

/// The `data` object inside an Imgur response.
#[derive(Debug, Deserialize)]
struct ImgurData {
    link: Option<String>,
    deletehash: Option<String>,
    error: Option<String>,
}

impl UploadBackend for ImgurBackend {
    fn id(&self) -> &'static str {
        "imgur"
    }

    fn name(&self) -> &'static str {
        "Imgur"
    }

    fn upload<'a>(
        &'a self,
        image_bytes: &'a [u8],
        mime: ImageMime,
        ctx: &'a UploadContext<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<UploadOutcome, UploadError>> + Send + 'a>> {
        Box::pin(async move {
            let url = format!("{}/3/image", self.base_url);

            let part = reqwest::multipart::Part::bytes(image_bytes.to_vec())
                .file_name(ctx.filename.unwrap_or("image").to_string())
                .mime_str(mime.as_str())
                .map_err(|e| UploadError::Transport(format!("invalid mime: {e}")))?;

            let form = reqwest::multipart::Form::new().part("image", part);
            let mut request = self.http.post(&url).multipart(form);

            if let Some(entry) = ctx.credentials {
                match entry.get_password() {
                    Ok(client_id) => {
                        request = request.header("Authorization", format!("Client-ID {client_id}"));
                    }
                    Err(e) => {
                        tracing::info!("keyring read failed; falling back to anonymous: {e}");
                    }
                }
            }

            let response = request
                .send()
                .await
                .map_err(|e| UploadError::Transport(e.to_string()))?;

            let status = response.status().as_u16();

            if response.status().is_success() {
                let body: ImgurResponse = response.json().await.map_err(|e| {
                    UploadError::Transport(format!("decoding success response: {e}"))
                })?;
                return parse_success(body, status);
            }

            let message = match response.json::<ImgurResponse>().await {
                Ok(ImgurResponse {
                    data:
                        Some(ImgurData {
                            error: Some(msg), ..
                        }),
                }) => msg,
                _ => format!("HTTP {status}"),
            };

            match status {
                401 | 403 => Err(UploadError::Auth(message)),
                _ => Err(UploadError::Backend { status, message }),
            }
        })
    }
}

/// Extract the outcome from a 2xx response body.
fn parse_success(body: ImgurResponse, status: u16) -> Result<UploadOutcome, UploadError> {
    let data = body.data.ok_or_else(|| UploadError::Backend {
        status,
        message: "response missing data object".to_string(),
    })?;
    let url = data.link.ok_or_else(|| UploadError::Backend {
        status,
        message: "response missing data.link".to_string(),
    })?;
    Ok(UploadOutcome::new(url, data.deletehash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flintshot_core::{ImageMime, UploadContext, UploadError};

    /// Stand-in image payload — mockito ignores the body, content is irrelevant.
    const PNG_BYTES: &[u8] = b"fake-png-bytes";

    fn anon_ctx() -> UploadContext<'static> {
        UploadContext {
            credentials: None,
            filename: Some("shot.png"),
        }
    }

    #[tokio::test]
    async fn anonymous_upload_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/3/image")
            .match_header("authorization", mockito::Matcher::Missing)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"data":{"link":"https://imgur.com/abc123","deletehash":"del42"},"success":true,"status":200}"#,
            )
            .create_async()
            .await;

        let backend = ImgurBackend::with_base_url(&server.url());
        let outcome = backend
            .upload(PNG_BYTES, ImageMime::Png, &anon_ctx())
            .await
            .expect("2xx should yield outcome");

        assert_eq!(outcome.url, "https://imgur.com/abc123");
        assert_eq!(outcome.delete_handle.as_deref(), Some("del42"));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn upload_without_deletehash() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("POST", "/3/image")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"data":{"link":"https://imgur.com/noDel"},"success":true,"status":200}"#)
            .create_async()
            .await;

        let backend = ImgurBackend::with_base_url(&server.url());
        let outcome = backend
            .upload(PNG_BYTES, ImageMime::Png, &anon_ctx())
            .await
            .expect("2xx should yield outcome");

        assert_eq!(outcome.url, "https://imgur.com/noDel");
        assert!(outcome.delete_handle.is_none());
    }

    #[tokio::test]
    async fn rate_limit_maps_to_backend_error() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("POST", "/3/image")
            .with_status(429)
            .with_header("content-type", "application/json")
            .with_body(r#"{"data":{"error":"Rate limit exceeded"},"success":false,"status":429}"#)
            .create_async()
            .await;

        let backend = ImgurBackend::with_base_url(&server.url());
        let err = backend
            .upload(PNG_BYTES, ImageMime::Png, &anon_ctx())
            .await
            .expect_err("429 should error");

        match err {
            UploadError::Backend { status, message } => {
                assert_eq!(status, 429);
                assert!(message.contains("Rate limit"), "message was: {message}");
            }
            other => panic!("expected Backend, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn unauthorized_maps_to_auth_error() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("POST", "/3/image")
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(r#"{"data":{"error":"Invalid Client-ID"},"success":false,"status":401}"#)
            .create_async()
            .await;

        let backend = ImgurBackend::with_base_url(&server.url());
        let err = backend
            .upload(PNG_BYTES, ImageMime::Png, &anon_ctx())
            .await
            .expect_err("401 should error");

        assert!(
            matches!(err, UploadError::Auth(ref m) if m.contains("Invalid Client-ID")),
            "expected Auth, got {err:?}"
        );
    }

    #[tokio::test]
    async fn forbidden_maps_to_auth_error() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("POST", "/3/image")
            .with_status(403)
            .with_header("content-type", "application/json")
            .with_body(r#"{"data":{"error":"Forbidden"},"success":false,"status":403}"#)
            .create_async()
            .await;

        let backend = ImgurBackend::with_base_url(&server.url());
        let err = backend
            .upload(PNG_BYTES, ImageMime::Png, &anon_ctx())
            .await
            .expect_err("403 should error");

        assert!(
            matches!(err, UploadError::Auth(_)),
            "expected Auth, got {err:?}"
        );
    }

    #[tokio::test]
    async fn transport_failure_maps_to_transport_error() {
        // Port 1 is guaranteed unbound → immediate connection refused.
        let backend = ImgurBackend::with_base_url("http://127.0.0.1:1");
        let err = backend
            .upload(PNG_BYTES, ImageMime::Png, &anon_ctx())
            .await
            .expect_err("connection refused should error");

        assert!(
            matches!(err, UploadError::Transport(_)),
            "expected Transport, got {err:?}"
        );
    }

    #[tokio::test]
    async fn malformed_success_body_maps_to_transport_error() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("POST", "/3/image")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("not json at all")
            .create_async()
            .await;

        let backend = ImgurBackend::with_base_url(&server.url());
        let err = backend
            .upload(PNG_BYTES, ImageMime::Png, &anon_ctx())
            .await
            .expect_err("malformed body should error");

        assert!(
            matches!(err, UploadError::Transport(_)),
            "expected Transport, got {err:?}"
        );
    }

    /// When `ctx.credentials` points at a keychain entry that was never
    /// set, `get_password()` returns an error and the backend falls back
    /// to anonymous mode. On headless CI this exercises the fallback
    /// path without needing a real keychain.
    #[tokio::test]
    async fn unset_credential_falls_back_to_anonymous() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/3/image")
            .match_header("authorization", mockito::Matcher::Missing)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"data":{"link":"https://imgur.com/anon"},"success":true,"status":200}"#)
            .create_async()
            .await;

        let entry =
            keyring::Entry::new("flintshot-test", "imgur-never-set").expect("Entry::new is inert");
        let ctx = UploadContext {
            credentials: Some(&entry),
            filename: None,
        };

        let backend = ImgurBackend::with_base_url(&server.url());
        let outcome = backend
            .upload(PNG_BYTES, ImageMime::Png, &ctx)
            .await
            .expect("fallback to anonymous should succeed");

        assert_eq!(outcome.url, "https://imgur.com/anon");
        mock.assert_async().await;
    }
}
