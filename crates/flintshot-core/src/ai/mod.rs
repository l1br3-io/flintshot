//! OpenAI-compatible chat wire types, the [`AiClient`] trait, and
//! provider presets for the Ask-the-shot loop.
//!
//! NOTE: the wire structs in this module serialize `snake_case` to
//! match the `OpenAI` chat-completions wire format — wire-format
//! compatibility, not a repo-convention override. The repo's
//! kebab-case Serde convention applies to *our* TOML/JSON files only.
//!
//! Mirrors the upload-backend split (ADR-0004 / ADR-0024): types and
//! trait live here; the reqwest implementation lives in
//! `flintshot-app`. No GUI toolkit types appear in this module.

pub mod stream;
pub mod transform;

pub use stream::{AiStreamChunk, SseDecoder, SseEvent};
pub use transform::{transform_store_path, AiTransform, TransformStore};

use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

/// One chat message in a vision-capable conversation.
///
/// The `OpenAI` wire shape is `{"role": ..., "content": <string or
/// parts-array>}` — a *duck-typed* body serde's tagged-enum attrs
/// can't express (adjacent tagging nests a `"content": {...}`
/// object providers reject). Serde therefore routes through the
/// private [`WireMessage`] struct; this enum stays the public API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(into = "WireMessage", from = "WireMessage")]
#[non_exhaustive]
pub enum ChatMessage {
    /// User turn; content may mix text and images.
    User {
        /// Ordered content parts of the message.
        content: Vec<ContentPart>,
    },
    /// Text-only assistant turn (what our providers return in v1).
    Assistant {
        /// The assistant's text.
        content: String,
    },
}

/// Internal wire form of one chat message (the actual `OpenAI` shape).
#[derive(Debug, Serialize, Deserialize)]
struct WireMessage {
    role: String,
    content: WireContent,
}

/// Message content is a bare string (assistant turns) or an array of
/// typed parts (vision user turns).
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum WireContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

impl From<ChatMessage> for WireMessage {
    fn from(message: ChatMessage) -> Self {
        match message {
            ChatMessage::User { content } => Self {
                role: "user".into(),
                content: WireContent::Parts(content),
            },
            ChatMessage::Assistant { content } => Self {
                role: "assistant".into(),
                content: WireContent::Text(content),
            },
        }
    }
}

impl From<WireMessage> for ChatMessage {
    fn from(wire: WireMessage) -> Self {
        match (wire.role.as_str(), wire.content) {
            ("assistant", WireContent::Text(text)) => Self::Assistant { content: text },
            // Providers never send assistant parts-arrays; if one
            // ever does, salvage the text rather than failing.
            ("assistant", WireContent::Parts(parts)) => Self::Assistant {
                content: join_text_parts(&parts),
            },
            (_, WireContent::Parts(parts)) => Self::User { content: parts },
            // A user turn with bare-string content (valid on the
            // wire) becomes a single text part.
            (_, WireContent::Text(text)) => Self::User {
                content: vec![ContentPart::Text { text }],
            },
        }
    }
}

/// Concatenate the text parts of a parts array (image parts are
/// non-textual; there is nothing to salvage from them).
fn join_text_parts(parts: &[ContentPart]) -> String {
    let mut out = String::new();
    for part in parts {
        if let ContentPart::Text { text } = part {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(text);
        }
    }
    out
}

/// A message content part (text, or an inline image data URL).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
#[non_exhaustive]
pub enum ContentPart {
    /// A text chunk.
    Text {
        /// The text itself.
        text: String,
    },
    /// An inline image (`data:image/png;base64,…` — built by the
    /// caller from `render::compose_final` output, longest edge
    /// capped at 2048 px before PNG encode to bound request size).
    ImageUrl {
        /// The image payload wrapper.
        image_url: ImageUrl,
    },
}

/// The `image_url` object of an image content part.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageUrl {
    /// `data:image/png;base64,…` or an `https://` URL.
    pub url: String,
}

/// Convenience constructor: a text content part.
#[must_use]
pub fn text_part(text: impl Into<String>) -> ContentPart {
    ContentPart::Text { text: text.into() }
}

/// Convenience constructor: an image content part from a data URL.
#[must_use]
pub fn image_part(data_url: impl Into<String>) -> ContentPart {
    ContentPart::ImageUrl {
        image_url: ImageUrl {
            url: data_url.into(),
        },
    }
}

/// A chat-completions request body.
///
/// Hand-checked against the `OpenAI` `POST /chat/completions` shape
/// (xAI and Gemini's OpenAI-compatible endpoint accept the same
/// fields; xAI additionally *requires* `max_tokens`, so the worker
/// always sets it).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiRequest {
    /// Model id, e.g. `"gpt-4o-mini"`.
    pub model: String,
    /// The conversation so far.
    pub messages: Vec<ChatMessage>,
    /// Generation cap. Sent only when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Request SSE token streaming (`"stream": true`). Skipped when
    /// `None` so the non-streaming golden request body stays
    /// byte-identical — only the streaming worker sets it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

/// One choice in a chat-completions response envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiChoice {
    /// Position of this choice in `choices` (we request one).
    #[serde(default)]
    pub index: u32,
    /// The generated message.
    pub message: AiResponseMessage,
    /// Why the model stopped (`"stop"`, `"length"`, ...).
    #[serde(default)]
    pub finish_reason: Option<String>,
}

/// The `message` object of a response choice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiResponseMessage {
    /// Always `"assistant"` in practice; optional on the wire.
    #[serde(default)]
    pub role: Option<String>,
    /// The generated text. Providers may return `null` (e.g. tool
    /// calls), so it parses as an option.
    #[serde(default)]
    pub content: Option<String>,
}

/// A chat-completions response envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiResponse {
    /// Provider-side completion id.
    #[serde(default)]
    pub id: Option<String>,
    /// Generated choices; we consume `choices[0]`.
    #[serde(default)]
    pub choices: Vec<AiChoice>,
}

impl AiResponse {
    /// The text of `choices[0].message.content`, if present.
    #[must_use]
    pub fn first_text(&self) -> Option<&str> {
        self.choices
            .first()
            .and_then(|choice| choice.message.content.as_deref())
    }
}

/// Provider error envelope: `{"error": {"message": "...", ...}}`.
///
/// All providers in scope (`OpenAI`, `xAI`, Gemini-compat) return this
/// shape on non-2xx statuses.
#[derive(Debug, Deserialize)]
struct AiErrorBody {
    error: AiErrorDetail,
}

#[derive(Debug, Deserialize)]
struct AiErrorDetail {
    message: String,
}

/// Errors returned by AI providers / transports.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AiError {
    /// HTTP/transport failure (DNS, TLS, timeout).
    #[error("transport error: {0}")]
    Transport(String),

    /// Authentication failure (missing, bad, or expired API key).
    #[error("auth error: {0}")]
    Auth(String),

    /// Rate limited (HTTP 429).
    #[error("rate limited: {0}")]
    RateLimited(String),

    /// The provider rejected the request (400, 5xx, ...).
    #[error("provider error ({status}): {message}")]
    Provider {
        /// HTTP status code.
        status: u16,
        /// Provider-surfaced error message.
        message: String,
    },

    /// A 2xx response with no usable text (`choices` empty or
    /// `content` null).
    #[error("empty response: {0}")]
    EmptyResponse(String),
}

impl AiError {
    /// Build the typed [`AiError`] for a non-2xx status from its
    /// response body.
    ///
    /// Parses the `{"error": {"message": ...}}` envelope all in-scope
    /// providers use; if the body isn't that shape, the raw body
    /// becomes the message (still surfaced, never swallowed). Status
    /// mapping: 401/403 ⇒ [`AiError::Auth`], 429 ⇒
    /// [`AiError::RateLimited`], everything else ⇒
    /// [`AiError::Provider`].
    #[must_use]
    pub fn from_status_and_body(status: u16, body: &str) -> Self {
        let message = serde_json::from_str::<AiErrorBody>(body)
            .map_or_else(|_| body.to_string(), |parsed| parsed.error.message);
        Self::from_status_and_message(status, message)
    }

    /// Map a status + already-extracted message to the typed variant
    /// (the mapping half of [`AiError::from_status_and_body`], split
    /// out for clients that parse the body themselves).
    #[must_use]
    pub fn from_status_and_message(status: u16, message: String) -> Self {
        match status {
            401 | 403 => Self::Auth(message),
            429 => Self::RateLimited(message),
            _ => Self::Provider { status, message },
        }
    }
}

/// Trait implemented by AI provider clients (the upload-backend
/// pattern: trait + types in core, impl in `flintshot-app`).
///
/// Clients are `Send + Sync` so a worker thread can own one; the
/// boxed-future return keeps the trait dyn-compatible. Futures are
/// only awaited on the request worker thread — never on the UI
/// thread.
pub trait AiClient: Send + Sync {
    /// Send one chat-completions request, await the full
    /// (non-streaming) response.
    ///
    /// # Errors
    /// See [`AiError`] variants.
    fn chat<'a>(
        &'a self,
        request: &'a AiRequest,
    ) -> Pin<Box<dyn Future<Output = Result<AiResponse, AiError>> + Send + 'a>>;
}

/// A provider preset: machine id, display name, default base URL,
/// and a vision-capable default model hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPreset {
    /// Stable machine id (`"openai"`) — the `[ai]` config value and
    /// keyring suffix.
    pub id: &'static str,
    /// Human-readable name for UI.
    pub name: &'static str,
    /// Chat-completions base URL (no trailing slash; the client
    /// appends `/chat/completions`). Empty for custom.
    pub base_url: &'static str,
    /// Suggested first model id (a hint, never a lock). Empty for
    /// custom.
    pub model_hint: &'static str,
}

const OPENAI: ProviderPreset = ProviderPreset {
    id: "openai",
    name: "OpenAI",
    base_url: "https://api.openai.com/v1",
    model_hint: "gpt-4o-mini",
};

const XAI: ProviderPreset = ProviderPreset {
    id: "xai",
    name: "xAI (Grok)",
    base_url: "https://api.x.ai/v1",
    model_hint: "grok-2-vision-1212",
};

const GEMINI: ProviderPreset = ProviderPreset {
    id: "gemini",
    name: "Gemini",
    base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
    model_hint: "gemini-2.0-flash",
};

const CUSTOM: ProviderPreset = ProviderPreset {
    id: "custom",
    name: "Custom (OpenAI-compatible)",
    base_url: "",
    model_hint: "",
};

impl ProviderPreset {
    /// All built-in presets, in UI display order.
    #[must_use]
    pub const fn all() -> &'static [ProviderPreset] {
        &[OPENAI, XAI, GEMINI, CUSTOM]
    }

    /// Look up a preset by machine id. Unknown ids (and `"custom"`)
    /// resolve to [`ProviderPreset::all`]'s custom entry — the same
    /// string-tolerant policy as `config.rs` fallbacks, so a typo in
    /// `[ai]` provider never breaks startup.
    #[must_use]
    pub fn for_id(id: &str) -> &'static Self {
        Self::all()
            .iter()
            .find(|preset| preset.id == id)
            .unwrap_or(&CUSTOM)
    }
}

/// Encode PNG bytes as a `data:image/png;base64,…` URL for the
/// `image_url` content part.
#[must_use]
pub fn png_data_url(png_bytes: &[u8]) -> String {
    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::STANDARD.encode(png_bytes);
    format!("data:image/png;base64,{encoded}")
}

/// A 1×1 opaque white PNG as a data URL — the smallest payload that
/// still makes a provider exercise its image-input path. The
/// `vision_probe` test decodes it, so a mangled constant fails CI
/// rather than every connection test.
const PROBE_IMAGE_URL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR4nGP4DwQACfsD/fteaysAAAAASUVORK5CYII=";

/// The connection-probe conversation: one user message pairing the
/// one-word-answer prompt with an inline PNG — the exact
/// `image_url` content-part shape the transform path sends. A
/// passing round-trip therefore proves the configured model accepts
/// image input, not merely text.
#[must_use]
pub fn vision_probe_messages() -> Vec<ChatMessage> {
    vec![ChatMessage::User {
        content: vec![
            text_part("Reply with the single word: ready"),
            image_part(PROBE_IMAGE_URL),
        ],
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    // Wire fixtures hand-checked against the OpenAI chat-completions
    // API reference (POST /v1/chat/completions); xAI and Gemini's
    // OpenAI-compatible endpoint accept the same shapes. Task 004's
    // client tests re-check these against live provider docs.

    #[test]
    fn vision_request_serializes_to_openai_shape() {
        let request = AiRequest {
            model: "gpt-4o-mini".into(),
            messages: vec![ChatMessage::User {
                content: vec![
                    text_part("Extract the text inside this image"),
                    image_part("data:image/png;base64,aGVsbG8="),
                ],
            }],
            max_tokens: Some(1024),
            stream: None,
        };
        let json = serde_json::to_value(&request).expect("serialize");
        // {
        //   "model": "gpt-4o-mini",
        //   "messages": [{
        //     "role": "user",
        //     "content": [
        //       {"type": "text", "text": "Extract the text inside this image"},
        //       {"type": "image_url", "image_url": {"url": "data:image/png;base64,aGVsbG8="}}
        //     ]
        //   }],
        //   "max_tokens": 1024
        // }
        assert_eq!(json["model"], "gpt-4o-mini");
        let msg = &json["messages"][0];
        assert_eq!(msg["role"], "user");
        assert_eq!(msg["content"][0]["type"], "text");
        assert_eq!(
            msg["content"][0]["text"],
            "Extract the text inside this image"
        );
        assert_eq!(msg["content"][1]["type"], "image_url");
        assert_eq!(
            msg["content"][1]["image_url"]["url"],
            "data:image/png;base64,aGVsbG8="
        );
        assert_eq!(json["max_tokens"], 1024);
    }

    #[test]
    fn assistant_message_serializes_as_plain_string_content() {
        // Follow-up turns send prior assistant text back as a string
        // content, the shape every provider accepts.
        let message = ChatMessage::Assistant {
            content: "Here is the text".into(),
        };
        let json = serde_json::to_value(&message).expect("serialize");
        assert_eq!(json["role"], "assistant");
        assert_eq!(json["content"], "Here is the text");
    }

    #[test]
    fn chat_messages_parse_from_wire_and_round_trip() {
        let user: ChatMessage = serde_json::from_str(
            r#"{"role": "user", "content": [
                {"type": "text", "text": "Extract the text"},
                {"type": "image_url", "image_url": {"url": "data:image/png;base64,aGVsbG8="}}
            ]}"#,
        )
        .expect("parse user");
        assert_eq!(
            user,
            ChatMessage::User {
                content: vec![
                    text_part("Extract the text"),
                    image_part("data:image/png;base64,aGVsbG8="),
                ]
            }
        );

        let assistant: ChatMessage =
            serde_json::from_str(r#"{"role": "assistant", "content": "done"}"#).expect("parse");
        assert_eq!(
            assistant,
            ChatMessage::Assistant {
                content: "done".into()
            }
        );

        for message in [user, assistant] {
            let json = serde_json::to_string(&message).expect("serialize");
            let back: ChatMessage = serde_json::from_str(&json).expect("round-trip");
            assert_eq!(back, message);
        }
    }

    #[test]
    fn max_tokens_omitted_when_none() {
        let request = AiRequest {
            model: "m".into(),
            messages: vec![],
            max_tokens: None,
            stream: None,
        };
        let json = serde_json::to_value(&request).expect("serialize");
        assert!(json.get("max_tokens").is_none());
    }

    #[test]
    fn stream_flag_serializes_only_when_set() {
        let base = AiRequest {
            model: "m".into(),
            messages: vec![],
            max_tokens: None,
            stream: None,
        };
        // None omits the key entirely — the non-streaming golden
        // request body stays byte-identical.
        let json = serde_json::to_value(&base).expect("serialize");
        assert!(json.get("stream").is_none());

        let streaming = AiRequest {
            stream: Some(true),
            ..base
        };
        let json = serde_json::to_value(&streaming).expect("serialize");
        assert_eq!(json["stream"], true);
    }

    #[test]
    fn response_envelope_round_trips() {
        // {
        //   "id": "chatcmpl-abc123",
        //   "choices": [{
        //     "index": 0,
        //     "message": {"role": "assistant", "content": "## Extracted\n- foo"},
        //     "finish_reason": "stop"
        //   }]
        // }
        let body = r###"{
            "id": "chatcmpl-abc123",
            "object": "chat.completion",
            "created": 1234567890,
            "model": "gpt-4o-mini",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "## Extracted\n- foo"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 100, "completion_tokens": 8, "total_tokens": 108}
        }"###;
        let parsed: AiResponse = serde_json::from_str(body).expect("parse envelope");
        assert_eq!(parsed.id.as_deref(), Some("chatcmpl-abc123"));
        assert_eq!(parsed.first_text(), Some("## Extracted\n- foo"));
        let round: AiResponse =
            serde_json::from_str(&serde_json::to_string(&parsed).expect("reserialize"))
                .expect("round-trip");
        assert_eq!(round, parsed);
    }

    #[test]
    fn response_with_null_content_parses_and_reports_empty() {
        let body =
            r#"{"choices": [{"index": 0, "message": {"role": "assistant", "content": null}}]}"#;
        let parsed: AiResponse = serde_json::from_str(body).expect("parse");
        assert_eq!(parsed.first_text(), None);
    }

    #[test]
    fn error_body_parses_into_typed_error_with_message() {
        // 401 surfaces as Auth with the provider's message.
        let err = AiError::from_status_and_body(
            401,
            r#"{"error": {"message": "Incorrect API key provided", "type": "invalid_request_error", "code": "invalid_api_key"}}"#,
        );
        match err {
            AiError::Auth(message) => assert_eq!(message, "Incorrect API key provided"),
            other => panic!("expected Auth, got {other:?}"),
        }

        // 429 surfaces as RateLimited.
        let err = AiError::from_status_and_body(
            429,
            r#"{"error": {"message": "Rate limit reached", "type": "rate_limit_error"}}"#,
        );
        match err {
            AiError::RateLimited(message) => assert_eq!(message, "Rate limit reached"),
            other => panic!("expected RateLimited, got {other:?}"),
        }

        // 5xx surfaces as Provider with the status retained.
        let err = AiError::from_status_and_body(
            500,
            r#"{"error": {"message": "Internal server error"}}"#,
        );
        match err {
            AiError::Provider { status, message } => {
                assert_eq!(status, 500);
                assert_eq!(message, "Internal server error");
            }
            other => panic!("expected Provider, got {other:?}"),
        }
    }

    #[test]
    fn non_json_error_body_falls_back_to_raw_text() {
        let err = AiError::from_status_and_body(502, "<html>Bad Gateway</html>");
        match err {
            AiError::Provider { status, message } => {
                assert_eq!(status, 502);
                assert_eq!(message, "<html>Bad Gateway</html>");
            }
            other => panic!("expected Provider, got {other:?}"),
        }
    }

    #[test]
    fn provider_presets_have_correct_base_urls() {
        assert_eq!(
            ProviderPreset::for_id("openai").base_url,
            "https://api.openai.com/v1"
        );
        assert_eq!(
            ProviderPreset::for_id("xai").base_url,
            "https://api.x.ai/v1"
        );
        assert_eq!(
            ProviderPreset::for_id("gemini").base_url,
            "https://generativelanguage.googleapis.com/v1beta/openai"
        );
        assert_eq!(ProviderPreset::for_id("custom").base_url, "");
    }

    #[test]
    fn unknown_provider_id_falls_back_to_custom() {
        let preset = ProviderPreset::for_id("anthrope"); // typo-tolerant
        assert_eq!(preset.id, "custom");
        assert_eq!(preset, ProviderPreset::for_id("custom"));
    }

    #[test]
    fn presets_cover_display_order() {
        let ids: Vec<&str> = ProviderPreset::all().iter().map(|p| p.id).collect();
        assert_eq!(ids, ["openai", "xai", "gemini", "custom"]);
    }

    #[test]
    fn png_data_url_prefixes_and_encodes() {
        // base64 of b"hello" is aGVsbG8=
        assert_eq!(png_data_url(b"hello"), "data:image/png;base64,aGVsbG8=");
    }

    #[test]
    fn vision_probe_pairs_prompt_with_a_real_inline_png() {
        use base64::Engine as _;
        let messages = vision_probe_messages();
        let [ChatMessage::User { content }] = messages.as_slice() else {
            panic!("the probe is exactly one user message");
        };
        // Same order as the transform path: text first, image second.
        assert!(matches!(content[0], ContentPart::Text { .. }));
        let ContentPart::ImageUrl { image_url } = &content[1] else {
            panic!("the second part carries the image");
        };
        let payload = image_url
            .url
            .strip_prefix("data:image/png;base64,")
            .expect("data URL prefix");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(payload)
            .expect("valid base64");
        // A mangled constant would make every connection test a
        // provider-side bad-request — hold it to a real decodable PNG.
        image::load_from_memory(&bytes).expect("decodes as a PNG");
    }
}
