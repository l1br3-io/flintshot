//! OpenAI-compatible chat client — the concrete [`AiClient`] impl.
//!
//! POSTs JSON to `{base_url}/chat/completions` with
//! `Authorization: Bearer <key>`; wire types and error mapping come
//! from `flintshot_core::ai` (the upload-backend split: trait in
//! core, HTTP impl here — ADR-0026 stack, reqwest + rustls).
//!
//! Provider coverage on one wire format:
//!
//! | Provider    | Base URL                                             | Auth                          |
//! | ----------- | ---------------------------------------------------- | ----------------------------- |
//! | `OpenAI`      | `https://api.openai.com/v1`                          | `Bearer`                      |
//! | `xAI` (Grok)  | `https://api.x.ai/v1`                                | `Bearer`                      |
//! | `Gemini`      | `https://generativelanguage.googleapis.com/v1beta/openai` | `Bearer` (see GOTCHA below) |
//! | custom      | any (`Ollama`, `LM Studio`, `OpenRouter`, ...)             | `Bearer`, or none when keyless |
//!
//! GOTCHA (Gemini): the OpenAI-compat endpoint also accepts the key
//! as a `?key=` query param, but plain `Authorization: Bearer` is
//! the documented header form — one header path covers all four
//! presets, so no per-provider special-casing.
//!
//! An empty key omits the header entirely: local servers (`Ollama`,
//! `LM Studio`) run keyless and some reject requests carrying an
//! `Authorization` header.

// The client surface is wired by the page bridge (app_window) and
// the worker module; the keyring helpers serve Task 010's config
// page. Items unused in some feature combinations stay allowed so
// the CI dead-code gate stays green without wiring consumers early.
#![allow(dead_code)]

pub mod worker;

use flintshot_core::{AiClient, AiError, AiRequest, AiResponse};
use std::future::Future;
use std::pin::Pin;

/// The OpenAI-compatible chat client.
///
/// Construct with [`OpenAiCompatClient::new`] (preset base URL +
/// resolved key) or [`OpenAiCompatClient::with_base_url_and_key`]
/// (test seam / custom endpoints).
#[derive(Debug, Clone)]
pub struct OpenAiCompatClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl OpenAiCompatClient {
    /// A client for an explicit base URL and key.
    ///
    /// The `/chat/completions` path is appended at request time;
    /// pass only the API root (e.g. `https://api.x.ai/v1` or a
    /// mock-server URL). An empty key sends no `Authorization`
    /// header (keyless local servers).
    #[must_use]
    pub fn with_base_url_and_key(base_url: &str, api_key: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
        }
    }

    /// A client from resolved config: base URL (override or preset)
    /// and the provider's keyring key (empty when none is stored).
    #[must_use]
    pub fn new(base_url: &str, api_key: &str) -> Self {
        Self::with_base_url_and_key(base_url, api_key)
    }
}

/// Full request URL: `{base}/chat/completions` (trailing base-URL
/// slashes tolerated).
#[must_use]
pub fn chat_url(base_url: &str) -> String {
    format!("{}/chat/completions", base_url.trim_end_matches('/'))
}

/// Header value for the key, or `None` when keyless. The key is
/// never logged; only this assembled header touches the wire.
#[must_use]
pub fn bearer_header(api_key: &str) -> Option<String> {
    (!api_key.is_empty()).then(|| format!("Bearer {api_key}"))
}

impl AiClient for OpenAiCompatClient {
    fn chat<'a>(
        &'a self,
        request: &'a AiRequest,
    ) -> Pin<Box<dyn Future<Output = Result<AiResponse, AiError>> + Send + 'a>> {
        Box::pin(async move {
            let mut outbound = self.http.post(chat_url(&self.base_url)).json(request);
            if let Some(auth) = bearer_header(&self.api_key) {
                outbound = outbound.header("Authorization", auth);
            }
            let response = outbound
                .send()
                .await
                .map_err(|e| AiError::Transport(e.to_string()))?;
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .map_err(|e| AiError::Transport(format!("reading response body: {e}")))?;
            if !(200..300).contains(&status) {
                return Err(AiError::from_status_and_body(status, &body));
            }
            let parsed: AiResponse = serde_json::from_str(&body)
                .map_err(|e| AiError::Transport(format!("decoding success response: {e}")))?;
            if parsed.first_text().is_none() {
                return Err(AiError::EmptyResponse(format!(
                    "HTTP {status} carried no assistant text"
                )));
            }
            Ok(parsed)
        })
    }
}

/// The keyring entry holding a provider's API key.
///
/// Same stance as `imgur:client-id` (ADR-0024): service
/// `"flintshot"`, account `ai:<provider>` — never a config file.
/// `None` when the OS has no keyring (headless CI).
#[must_use]
pub fn ai_key_entry(provider: &str) -> Option<keyring::Entry> {
    keyring::Entry::new("flintshot", &format!("ai:{provider}")).ok()
}

/// Read a provider's stored key, if any (`None` on missing entry or
/// no keyring — treated as "not configured").
#[must_use]
pub fn load_ai_key(provider: &str) -> Option<String> {
    let entry = ai_key_entry(provider)?;
    entry.get_password().ok()
}

/// Store (or overwrite) a provider's key.
///
/// # Errors
/// A human-readable message on keyring failure (the config page
/// shows it inline).
pub fn store_ai_key(provider: &str, key: &str) -> Result<(), String> {
    let entry = ai_key_entry(provider).ok_or_else(|| "no system keyring available".to_string())?;
    entry
        .set_password(key)
        .map_err(|e| format!("storing key in the system keyring: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flintshot_core::{image_part, text_part, ChatMessage};

    fn vision_request() -> AiRequest {
        AiRequest {
            model: "gpt-4o-mini".into(),
            messages: vec![ChatMessage::User {
                content: vec![
                    text_part("Extract the text inside this image"),
                    image_part("data:image/png;base64,aGVsbG8="),
                ],
            }],
            max_tokens: Some(1024),
        }
    }

    #[test]
    fn chat_url_joins_base_and_path() {
        assert_eq!(
            chat_url("https://api.x.ai/v1"),
            "https://api.x.ai/v1/chat/completions"
        );
        assert_eq!(
            chat_url("http://localhost:11434/v1/"),
            "http://localhost:11434/v1/chat/completions",
            "trailing slash tolerated"
        );
    }

    #[test]
    fn bearer_header_only_for_nonempty_key() {
        assert_eq!(bearer_header("sk-test"), Some("Bearer sk-test".to_string()));
        assert_eq!(bearer_header(""), None, "keyless local servers");
    }

    /// Golden request: the serialized JSON body matches the Task 001
    /// fixture byte-for-byte (field order is serde-declaration
    /// order, stable for a given core version).
    #[test]
    fn request_body_matches_golden_fixture() {
        let golden = r#"{"model":"gpt-4o-mini","messages":[{"role":"user","content":[{"type":"text","text":"Extract the text inside this image"},{"type":"image_url","image_url":{"url":"data:image/png;base64,aGVsbG8="}}]}],"max_tokens":1024}"#;
        assert_eq!(serde_json::to_string(&vision_request()).unwrap(), golden);
    }

    #[tokio::test]
    async fn chat_returns_assistant_text_for_well_formed_body() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("POST", "/chat/completions")
            .match_header("authorization", "Bearer sk-test")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r###"{"id":"chatcmpl-1","choices":[{"index":0,"message":{"role":"assistant","content":"## Extracted\n- foo"},"finish_reason":"stop"}]}"###,
            )
            .create_async()
            .await;

        let client = OpenAiCompatClient::with_base_url_and_key(&server.url(), "sk-test");
        let response = client.chat(&vision_request()).await.expect("2xx parses");
        assert_eq!(response.first_text(), Some("## Extracted\n- foo"));
    }

    #[tokio::test]
    async fn keyless_client_sends_no_authorization_header() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/chat/completions")
            .match_header("authorization", mockito::Matcher::Missing)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"choices":[{"index":0,"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}]}"#)
            .create_async()
            .await;

        let client = OpenAiCompatClient::with_base_url_and_key(&server.url(), "");
        client.chat(&vision_request()).await.expect("keyless ok");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn unauthorized_maps_to_auth_error() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("POST", "/chat/completions")
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": {"message": "Incorrect API key provided", "type": "invalid_request_error"}}"#)
            .create_async()
            .await;

        let client = OpenAiCompatClient::with_base_url_and_key(&server.url(), "sk-bad");
        let err = client
            .chat(&vision_request())
            .await
            .expect_err("401 errors");
        assert!(
            matches!(err, AiError::Auth(ref m) if m == "Incorrect API key provided"),
            "expected Auth, got {err:?}"
        );
    }

    #[tokio::test]
    async fn rate_limit_maps_to_rate_limited_error() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("POST", "/chat/completions")
            .with_status(429)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": {"message": "Rate limit reached"}}"#)
            .create_async()
            .await;

        let client = OpenAiCompatClient::with_base_url_and_key(&server.url(), "k");
        let err = client
            .chat(&vision_request())
            .await
            .expect_err("429 errors");
        assert!(
            matches!(err, AiError::RateLimited(ref m) if m == "Rate limit reached"),
            "expected RateLimited, got {err:?}"
        );
    }

    #[tokio::test]
    async fn server_error_maps_to_provider_error() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("POST", "/chat/completions")
            .with_status(500)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": {"message": "Internal server error"}}"#)
            .create_async()
            .await;

        let client = OpenAiCompatClient::with_base_url_and_key(&server.url(), "k");
        let err = client
            .chat(&vision_request())
            .await
            .expect_err("500 errors");
        assert!(
            matches!(err, AiError::Provider { status: 500, ref message } if message == "Internal server error"),
            "expected Provider, got {err:?}"
        );
    }

    #[tokio::test]
    async fn transport_failure_maps_to_transport_error() {
        // Port 1 is guaranteed unbound → immediate connection refused.
        let client = OpenAiCompatClient::with_base_url_and_key("http://127.0.0.1:1", "k");
        let err = client.chat(&vision_request()).await.expect_err("refused");
        assert!(
            matches!(err, AiError::Transport(_)),
            "expected Transport, got {err:?}"
        );
    }

    #[tokio::test]
    async fn two_hundred_with_no_text_maps_to_empty_response() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("POST", "/chat/completions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"choices":[]}"#)
            .create_async()
            .await;

        let client = OpenAiCompatClient::with_base_url_and_key(&server.url(), "k");
        let err = client.chat(&vision_request()).await.expect_err("empty");
        assert!(
            matches!(err, AiError::EmptyResponse(_)),
            "expected EmptyResponse, got {err:?}"
        );
    }
}
