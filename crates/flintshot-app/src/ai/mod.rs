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

    /// POST one chat-completions request and hand back the response
    /// the moment its headers arrive, before any body bytes are
    /// consumed.
    ///
    /// The contract: the caller sets `stream: Some(true)` on the
    /// request (this method sends the body verbatim); a non-2xx maps
    /// through [`AiError::from_status_and_body`] exactly like
    /// [`AiClient::chat`]; a 2xx inspects the content type —
    /// `text/event-stream` (parameters tolerated) yields
    /// [`ChatStreamStart::Sse`] whose `bytes_stream()` the worker
    /// owns until completion, stop, or drop; anything else yields
    /// [`ChatStreamStart::NotSse`] so the worker can fall back.
    ///
    /// # Errors
    /// [`AiError::Transport`] on connection or header-read failure;
    /// the [`AiError`] status mapping above on non-2xx.
    pub async fn chat_stream(&self, request: &AiRequest) -> Result<ChatStreamStart, AiError> {
        let mut outbound = self.http.post(chat_url(&self.base_url)).json(request);
        if let Some(auth) = bearer_header(&self.api_key) {
            outbound = outbound.header("Authorization", auth);
        }
        let response = outbound
            .send()
            .await
            .map_err(|e| AiError::Transport(e.to_string()))?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            let body = response
                .text()
                .await
                .map_err(|e| AiError::Transport(format!("reading error body: {e}")))?;
            return Err(AiError::from_status_and_body(status, &body));
        }
        let media_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        if media_type == "text/event-stream" {
            Ok(ChatStreamStart::Sse(response))
        } else {
            Ok(ChatStreamStart::NotSse(response))
        }
    }
}

/// Full request URL: `{base}/chat/completions` (trailing base-URL
/// slashes tolerated).
#[must_use]
pub fn chat_url(base_url: &str) -> String {
    format!("{}/chat/completions", base_url.trim_end_matches('/'))
}

/// What [`OpenAiCompatClient::chat_stream`] hands back once the
/// response headers are in — the headers-then-body split. Byte-level
/// errors *after* this point surface while draining the body stream
/// (the worker maps them to transport errors).
#[derive(Debug)]
#[non_exhaustive]
pub enum ChatStreamStart {
    /// 2xx + `text/event-stream`: the SSE body, owned by the caller.
    /// `bytes_stream()` yields the raw chunks; dropping the response
    /// closes the connection (the stop path's abort mechanism).
    Sse(reqwest::Response),
    /// 2xx with any other content type: the provider ignored
    /// `stream: true`. The worker falls back to non-streaming; the
    /// body rides along and is readable once (drain it, or parse it
    /// as a `chat.completion` and skip the retry round-trip).
    NotSse(reqwest::Response),
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
            stream: None,
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

    // --- chat_stream (Task 003) ---

    fn streaming_request() -> AiRequest {
        let mut request = vision_request();
        request.stream = Some(true);
        request
    }

    fn sse_body() -> String {
        use std::fmt::Write as _;
        let mut body = String::new();
        for text in ["Hel", "lo, ", "world"] {
            let _ = write!(
                body,
                "data: {{\"choices\":[{{\"delta\":{{\"content\":\"{text}\"}}}}]}}\n\n"
            );
        }
        body.push_str("data: [DONE]\n\n");
        body
    }

    #[tokio::test]
    async fn chat_stream_streams_sse_body_bytes() {
        use futures_util::StreamExt as _;

        let mut server = mockito::Server::new_async().await;
        server
            .mock("POST", "/chat/completions")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse_body())
            .create_async()
            .await;

        let client = OpenAiCompatClient::with_base_url_and_key(&server.url(), "sk-test");
        let start = client
            .chat_stream(&streaming_request())
            .await
            .expect("2xx SSE");
        let ChatStreamStart::Sse(response) = start else {
            panic!("text/event-stream must yield Sse, got {start:?}");
        };
        // Drain the owned byte stream — all chunks arrive in order.
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            bytes.extend_from_slice(&chunk.expect("chunk"));
        }
        let body = String::from_utf8(bytes).expect("utf-8 body");
        for text in ["Hel", "lo, ", "world", "[DONE]"] {
            assert!(body.contains(text), "body must carry {text:?}: {body:?}");
        }
    }

    #[tokio::test]
    async fn chat_stream_reports_not_sse_for_json_content_type() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("POST", "/chat/completions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"id":"chatcmpl-1","choices":[{"index":0,"message":{"role":"assistant","content":"whole"},"finish_reason":"stop"}]}"#,
            )
            .create_async()
            .await;

        let client = OpenAiCompatClient::with_base_url_and_key(&server.url(), "sk-test");
        let start = client.chat_stream(&streaming_request()).await.expect("2xx");
        let ChatStreamStart::NotSse(response) = start else {
            panic!("application/json must yield NotSse, got {start:?}");
        };
        // The body stays readable once — the fallback consumes it.
        let body = response.text().await.expect("body readable once");
        let parsed: flintshot_core::AiResponse =
            serde_json::from_str(&body).expect("non-streaming body parses");
        assert_eq!(parsed.first_text(), Some("whole"));
    }

    #[tokio::test]
    async fn chat_stream_maps_unauthorized_to_auth_error() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("POST", "/chat/completions")
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": {"message": "Incorrect API key provided"}}"#)
            .create_async()
            .await;

        let client = OpenAiCompatClient::with_base_url_and_key(&server.url(), "sk-bad");
        let err = client
            .chat_stream(&streaming_request())
            .await
            .expect_err("401 errors");
        assert!(
            matches!(err, AiError::Auth(ref m) if m == "Incorrect API key provided"),
            "expected Auth, got {err:?}"
        );
    }

    #[tokio::test]
    async fn chat_stream_maps_rate_limit_to_rate_limited_error() {
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
            .chat_stream(&streaming_request())
            .await
            .expect_err("429 errors");
        assert!(
            matches!(err, AiError::RateLimited(ref m) if m == "Rate limit reached"),
            "expected RateLimited, got {err:?}"
        );
    }

    #[tokio::test]
    async fn chat_stream_maps_server_error_to_provider_error() {
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
            .chat_stream(&streaming_request())
            .await
            .expect_err("500 errors");
        assert!(
            matches!(err, AiError::Provider { status: 500, ref message } if message == "Internal server error"),
            "expected Provider, got {err:?}"
        );
    }

    /// Regression: keyring v3 with no store feature compiled in
    /// silently falls back to an in-process *mock* store
    /// (`pub use mock as default`). Set/get "work" in-process, keys
    /// vanish on exit, and nothing ever reaches the OS keyring —
    /// AI keys were lost on every window close. This test fails if
    /// the workspace loses a real store feature for this platform.
    #[test]
    fn keyring_backs_entries_with_a_real_store_not_the_mock() {
        let entry = super::ai_key_entry("regression-check")
            .expect("keyring::Entry::new failed for this platform");
        let is_mock = entry
            .get_credential()
            .downcast_ref::<keyring::mock::MockCredential>()
            .is_some();
        assert!(
            !is_mock,
            "keyring fell back to its in-process mock store — keys \
             would vanish on exit; enable a store feature in the \
             workspace Cargo.toml"
        );
    }

    /// Full round-trip through the *real* OS secret store
    /// (set → get → delete), proving the store backend works at
    /// runtime — not just that a non-mock one is compiled in.
    /// Ignored by default because it touches the user's keyring;
    /// run explicitly on a desktop session:
    /// `cargo test -p flintshot-app -- --ignored keyring_round_trips`
    #[test]
    #[ignore = "touches the real OS keyring; run on a desktop session"]
    fn keyring_round_trips_through_the_real_store() {
        let entry = super::ai_key_entry("round-trip-check").expect("entry constructs");
        let _ = entry.delete_credential(); // clean slate, absent is fine

        entry
            .set_password("probe-value")
            .expect("the real store accepted the write");
        assert_eq!(
            entry
                .get_password()
                .expect("the real store served the read")
                .as_str(),
            "probe-value",
            "round-trip must return the stored value"
        );

        entry
            .delete_credential()
            .expect("cleanup deletes the entry");
        assert!(
            entry.get_password().is_err(),
            "the entry must be gone after cleanup"
        );
    }

    #[tokio::test]
    async fn keyless_stream_sends_no_authorization_header() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/chat/completions")
            .match_header("authorization", mockito::Matcher::Missing)
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse_body())
            .create_async()
            .await;

        let client = OpenAiCompatClient::with_base_url_and_key(&server.url(), "");
        let start = client.chat_stream(&streaming_request()).await.expect("ok");
        drop(start); // release the connection before the mock asserts
        mock.assert_async().await;
    }
}
