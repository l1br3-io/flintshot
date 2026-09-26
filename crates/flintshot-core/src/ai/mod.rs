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

pub mod transform;

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
    /// SSE streaming flag. Sent only when set — the non-streaming
    /// Test-connection probe omits it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    /// Streaming options; meaningful only together with
    /// `stream: true` (per the `OpenAI` chat/create reference the
    /// field must not appear otherwise).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<AiStreamOptions>,
}

/// The `stream_options` object of a streaming request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiStreamOptions {
    /// Ask the provider to stream a final usage-only chunk before
    /// `[DONE]`. All four in-scope providers support it.
    pub include_usage: bool,
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

/// Token-usage accounting, on a completion envelope or (with
/// `include_usage`) a stream's final chunk.
///
/// xAI attaches `usage` to *every* streaming chunk, cumulative —
/// parse each and let the last non-null one win.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiUsage {
    /// Tokens in the prompt side.
    pub prompt_tokens: u64,
    /// Tokens generated.
    pub completion_tokens: u64,
    /// `prompt_tokens + completion_tokens`.
    pub total_tokens: u64,
}

/// The `delta` of a streaming choice — the assistant message as it
/// grows. Unknown members (xAI's `reasoning_content`, tool-call
/// fields) are ignored rather than rejected.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AiStreamDelta {
    /// `"assistant"` on the first chunk, absent after.
    #[serde(default)]
    pub role: Option<String>,
    /// The text delta; `None` on role-only and terminal chunks.
    #[serde(default)]
    pub content: Option<String>,
}

/// One choice of a `chat.completion.chunk`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiStreamChoice {
    /// Position of this choice in `choices` (we request one).
    #[serde(default)]
    pub index: u32,
    /// The growing message.
    pub delta: AiStreamDelta,
    /// `None` on content chunks; `"stop"`/`"length"`/… on the
    /// terminal chunk, whose `delta` is empty.
    #[serde(default)]
    pub finish_reason: Option<String>,
}

/// One SSE `data:` payload of a streaming chat completion
/// (`object: "chat.completion.chunk"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiStreamChunk {
    /// Provider-side completion id.
    #[serde(default)]
    pub id: Option<String>,
    /// Growing choices; empty on the usage-only final chunk that
    /// `include_usage` requests.
    #[serde(default)]
    pub choices: Vec<AiStreamChoice>,
    /// Usage accounting — `None` on content chunks (`OpenAI`), on
    /// every chunk (xAI, cumulative), or on the final chunk.
    #[serde(default)]
    pub usage: Option<AiUsage>,
}

/// One parsed streaming event (the worker's payload after parsing).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum AiStreamEvent {
    /// A content delta to append to the streaming turn.
    Token(String),
    /// Usage accounting (xAI sends it on every chunk, cumulative —
    /// the last one wins; parsed for tolerance, not yet displayed).
    Usage(AiUsage),
    /// The provider finished (`[DONE]` or a clean stream close —
    /// both are valid ends).
    Done,
}

/// Parse one SSE `data:` payload. `data: [DONE]` → `Done`; a
/// usage-only chunk (empty `choices`) → `Usage`; a content chunk →
/// `Token` when it carries text, else `None` (role-only chunks,
/// keepalives, unknown fields are skipped without failing).
///
/// Malformed or unknown payloads return `None` — a data line is
/// never a fatal point in the stream loop.
#[must_use]
pub fn parse_stream_data_line(payload: &str) -> Option<AiStreamEvent> {
    if payload == "[DONE]" {
        return Some(AiStreamEvent::Done);
    }
    let chunk: AiStreamChunk = serde_json::from_str(payload).ok()?;
    if chunk.choices.is_empty() {
        return chunk.usage.map(AiStreamEvent::Usage);
    }
    chunk
        .choices
        .into_iter()
        .next()?
        .delta
        .content
        .filter(|text| !text.is_empty())
        .map(AiStreamEvent::Token)
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

/// The four built-in starter transforms — code, not data.
///
/// They live here (never in `ai-transforms.toml`) so a fresh profile
/// has working one-press transforms, they can't be deleted by a
/// store edit, and using one mutates nothing on disk: the MRU rides
/// `[ai] recent-transforms` instead. Ids are `builtin-*` and are
/// API-stable — the dialog preselect and the MRU key persist them.
#[must_use]
pub fn builtin_transforms() -> &'static [AiTransform] {
    static BUILTINS: std::sync::OnceLock<Vec<AiTransform>> = std::sync::OnceLock::new();
    BUILTINS.get_or_init(|| {
        let starter = |id: &str, name: &str, prompt: &str| AiTransform {
            id: id.to_string(),
            name: name.to_string(),
            prompt: prompt.to_string(),
            builtin: true,
        };
        vec![
            starter(
                "builtin-extract-text",
                "Extract text",
                "Extract the text inside this image, preserving its layout.",
            ),
            starter(
                "builtin-summarize",
                "Summarize",
                "Summarize this screenshot in a few bullet points.",
            ),
            starter(
                "builtin-explain-error",
                "Explain this error",
                "Explain the error in this screenshot: what caused it and how to fix it.",
            ),
            starter(
                "builtin-draft-reply",
                "Draft a reply",
                "Draft a short reply to the message conversation in this screenshot.",
            ),
        ]
    })
}

/// Merge user transforms + built-ins MRU-first for the dialog list.
///
/// Order: ids from `recent_transforms` that still exist (recent
/// order, stale ids filtered out), then the remaining user
/// transforms (store order), then the remaining built-ins.
#[must_use]
pub fn merge_transforms_mru_first(user: &[AiTransform], recent_ids: &[String]) -> Vec<AiTransform> {
    let mut remainder: Vec<AiTransform> = user
        .iter()
        .chain(builtin_transforms().iter())
        .cloned()
        .collect();
    let mut ordered = Vec::with_capacity(remainder.len());
    for id in recent_ids {
        if let Some(position) = remainder.iter().position(|t| &t.id == id) {
            ordered.push(remainder.remove(position));
        }
    }
    ordered.append(&mut remainder);
    ordered
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
            stream_options: None,
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
            stream_options: None,
        };
        let json = serde_json::to_value(&request).expect("serialize");
        assert!(json.get("max_tokens").is_none());
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

    // Streaming fixtures hand-checked against the SSE wire shapes in
    // docs/research/2026-09-26-phase7-adaptation-research.md §1
    // (OpenAI CreateChatCompletionStreamResponse schema, xAI's
    // streaming guide divergences, Gemini/Ollama compat pages).

    #[test]
    fn streaming_request_serializes_stream_fields() {
        // OpenAI chat/create: stream_options must only be set when
        // streaming. The worker always sets both together.
        let request = AiRequest {
            model: "gpt-4o-mini".into(),
            messages: vec![ChatMessage::User {
                content: vec![text_part("summarize")],
            }],
            max_tokens: Some(1024),
            stream: Some(true),
            stream_options: Some(AiStreamOptions {
                include_usage: true,
            }),
        };
        let json = serde_json::to_value(&request).expect("serialize");
        // {
        //   ...,
        //   "stream": true,
        //   "stream_options": {"include_usage": true}
        // }
        assert_eq!(json["stream"], true);
        assert_eq!(json["stream_options"]["include_usage"], true);
    }

    #[test]
    fn stream_fields_omitted_when_none() {
        // The non-streaming Test-connection probe sends neither field.
        let request = AiRequest {
            model: "m".into(),
            messages: vec![],
            max_tokens: None,
            stream: None,
            stream_options: None,
        };
        let json = serde_json::to_value(&request).expect("serialize");
        assert!(json.get("stream").is_none());
        assert!(json.get("stream_options").is_none());
    }

    #[test]
    fn baseline_content_chunk_parses_and_yields_token() {
        // OpenAI baseline content chunk (object chat.completion.chunk):
        // {
        //   "id": "chatcmpl-abc",
        //   "object": "chat.completion.chunk",
        //   "created": 1234567890,
        //   "model": "gpt-4o-mini",
        //   "choices": [{
        //     "index": 0,
        //     "delta": {"content": "Hel"},
        //     "finish_reason": null
        //   }],
        //   "usage": null
        // }
        let payload = r#"{
            "id": "chatcmpl-abc",
            "object": "chat.completion.chunk",
            "created": 1234567890,
            "model": "gpt-4o-mini",
            "choices": [{
                "index": 0,
                "delta": {"content": "Hel"},
                "finish_reason": null
            }],
            "usage": null
        }"#;
        let chunk: AiStreamChunk = serde_json::from_str(payload).expect("parse chunk");
        let [choice] = chunk.choices.as_slice() else {
            panic!("one choice");
        };
        assert_eq!(choice.index, 0);
        assert_eq!(choice.delta.content.as_deref(), Some("Hel"));
        assert_eq!(choice.finish_reason, None);
        assert_eq!(chunk.usage, None);

        assert_eq!(
            parse_stream_data_line(payload),
            Some(AiStreamEvent::Token("Hel".into()))
        );
    }

    #[test]
    fn role_only_and_terminal_chunks_yield_none() {
        // First chunk carries only the role (no text yet) — skipped.
        assert_eq!(
            parse_stream_data_line(
                r#"{"choices": [{"index": 0, "delta": {"role": "assistant"}, "finish_reason": null}]}"#
            ),
            None
        );
        // Terminal chunk: empty delta + finish_reason set — the text
        // is done; [DONE] (or a clean close) ends the stream, not this.
        assert_eq!(
            parse_stream_data_line(
                r#"{"choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]}"#
            ),
            None
        );
        // Empty-string content carries no text — skipped.
        assert_eq!(
            parse_stream_data_line(
                r#"{"choices": [{"index": 0, "delta": {"content": ""}, "finish_reason": null}]}"#
            ),
            None
        );
    }

    #[test]
    fn usage_only_chunk_yields_usage() {
        // OpenAI with include_usage: the final usage chunk arrives
        // before [DONE]; its choices is ALWAYS an empty array.
        // {
        //   "choices": [],
        //   "usage": {"prompt_tokens": 100, "completion_tokens": 20, "total_tokens": 120}
        // }
        let event = parse_stream_data_line(
            r#"{"choices": [], "usage": {"prompt_tokens": 100, "completion_tokens": 20, "total_tokens": 120}}"#,
        );
        assert_eq!(
            event,
            Some(AiStreamEvent::Usage(AiUsage {
                prompt_tokens: 100,
                completion_tokens: 20,
                total_tokens: 120,
            }))
        );
    }

    #[test]
    fn empty_choices_with_null_usage_yields_none() {
        // A chunk with no choices and no usage (xAI-style padding or a
        // provider keepalive) is skipped, not an error.
        assert_eq!(
            parse_stream_data_line(r#"{"choices": [], "usage": null}"#),
            None
        );
    }

    #[test]
    fn done_sentinel_yields_done() {
        assert_eq!(parse_stream_data_line("[DONE]"), Some(AiStreamEvent::Done));
    }

    #[test]
    fn xai_chunk_with_usage_and_reasoning_content_tolerated() {
        // xAI divergences (streaming guide): usage on EVERY chunk
        // (cumulative — the last one would win) and
        // delta.reasoning_content on reasoning models. Content wins the
        // event; the extras parse without failing. Ollama sends the
        // same cumulative-usage shape at stream end.
        let payload = r#"{
            "id": "chunk-1",
            "object": "chat.completion.chunk",
            "choices": [{
                "index": 0,
                "delta": {"content": "wor", "reasoning_content": "internal deliberation"},
                "finish_reason": null
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        }"#;
        let chunk: AiStreamChunk = serde_json::from_str(payload).expect("parse");
        assert_eq!(chunk.usage.map(|u| u.total_tokens), Some(15));
        assert_eq!(
            parse_stream_data_line(payload),
            Some(AiStreamEvent::Token("wor".into()))
        );
    }

    #[test]
    fn malformed_line_yields_none_never_panics() {
        // A malformed or unknown payload is skipped — the parser must
        // never be a fatal point in the stream loop.
        assert_eq!(parse_stream_data_line("not json"), None);
        assert_eq!(parse_stream_data_line(r#"{"unexpected": true}"#), None);
        assert_eq!(parse_stream_data_line(""), None);
    }

    #[test]
    fn builtin_transforms_are_four_with_stable_ids_and_flag() {
        let builtins = builtin_transforms();
        let ids: Vec<&str> = builtins.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "builtin-extract-text",
                "builtin-summarize",
                "builtin-explain-error",
                "builtin-draft-reply",
            ],
            "ids are API surface — stable across releases"
        );
        assert!(
            builtins.iter().all(|t| t.builtin),
            "every starter carries the builtin flag"
        );
        assert!(builtins
            .iter()
            .all(|t| !t.name.is_empty() && !t.prompt.is_empty()));
    }

    #[test]
    fn merge_orders_mru_first_then_user_then_builtins_and_filters_stale() {
        let user = |id: &str, name: &str| AiTransform {
            id: id.into(),
            name: name.into(),
            prompt: format!("prompt for {name}"),
            builtin: false,
        };
        let merged = merge_transforms_mru_first(
            &[user("u1", "One"), user("u2", "Two")],
            &[
                "u2".into(),
                "builtin-extract-text".into(),
                "gone-with-the-wind".into(),
            ],
        );
        let ids: Vec<&str> = merged.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "u2",                   // MRU head, most recent
                "builtin-extract-text", // MRU second
                "u1",                   // remaining user (store order)
                "builtin-summarize",    // remaining built-ins
                "builtin-explain-error",
                "builtin-draft-reply",
            ],
            "stale MRU id ('gone-with-the-wind') is filtered out"
        );
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
