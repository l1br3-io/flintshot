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

    /// The provider rejected an image content part — the configured
    /// model is text-only. Surfaced with a Configure deep-link,
    /// never retried blindly: the fix is a model change, not a
    /// resend.
    #[error("model rejected the image (text-only?): {message}")]
    VisionUnsupported {
        /// HTTP status the provider returned.
        status: u16,
        /// Provider-surfaced message, kept for the error card.
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
            // The vision check runs only in the generic arm: a 401
            // or 429 mentioning images keeps its more-specific
            // variant.
            _ if is_vision_rejection(&message) => Self::VisionUnsupported { status, message },
            _ => Self::Provider { status, message },
        }
    }
}

/// Terms marking a provider message as being about image/vision
/// input.
const VISION_TERMS: [&str; 4] = ["image", "vision", "modality", "multimodal"];

/// Phrases marking such a message as *rejecting* that input — a
/// bare image mention must not match ("image too large" is a payload
/// problem, not a capability statement).
const REJECTION_PHRASES: [&str; 5] = [
    "not supported",
    "only supported",
    "does not support",
    "invalid content type",
    "does not accept image",
];

/// Whether a provider error message says the model rejects
/// image/vision input. Lowercased-substring heuristic anchored on
/// the verified `OpenAI` wording ("Invalid content type. `image_url` is
/// only supported by certain models." — research §3.3) and tolerant
/// of rewordings; heuristic rot is safe — a miss degrades to today's
/// [`AiError::Provider`] card, never worse.
fn is_vision_rejection(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    VISION_TERMS.iter().any(|term| lower.contains(term))
        && REJECTION_PHRASES
            .iter()
            .any(|phrase| lower.contains(phrase))
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

/// One palette entry for the content-checking probe: the color word
/// the model must name and the RGB the challenge PNG is filled with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeColor {
    /// Lowercase English color word — the expected answer.
    pub name: &'static str,
    /// Solid fill for the challenge PNG.
    pub rgb: [u8; 3],
}

/// The probe palette: unambiguous primary color words, one distinct
/// RGB each. The challenge randomizes across it so a silent-ignore
/// model cannot pass by always guessing one fixed color.
pub const PROBE_PALETTE: [ProbeColor; 10] = [
    ProbeColor {
        name: "red",
        rgb: [237, 28, 36],
    },
    ProbeColor {
        name: "orange",
        rgb: [255, 127, 39],
    },
    ProbeColor {
        name: "yellow",
        rgb: [255, 221, 0],
    },
    ProbeColor {
        name: "green",
        rgb: [34, 177, 76],
    },
    ProbeColor {
        name: "blue",
        rgb: [0, 102, 255],
    },
    ProbeColor {
        name: "purple",
        rgb: [128, 0, 255],
    },
    ProbeColor {
        name: "pink",
        rgb: [255, 105, 180],
    },
    ProbeColor {
        name: "brown",
        rgb: [139, 69, 19],
    },
    ProbeColor {
        name: "black",
        rgb: [0, 0, 0],
    },
    ProbeColor {
        name: "white",
        rgb: [255, 255, 255],
    },
];

/// The probe prompt: one question, one-word answer shape, about the
/// image's content — the half that makes the probe capability-
/// checking rather than wire-shape-checking.
const PROBE_PROMPT: &str =
    "What is the dominant color of this image? Reply with only the color word.";

/// The content-checking probe challenge (v1): a solid-color PNG
/// paired with the color question, plus the expected answer a pass
/// is judged against. Replaces the wire-shape-only probe of
/// ask-the-shot v1 — a silent-ignore model (Ollama with a text-only
/// model, research §3.3) answered "ready" about an image it never
/// saw and passed; here it must name the color it was shown.
#[derive(Debug, Clone)]
pub struct ProbeChallenge {
    /// The challenge conversation: exactly one user message, text
    /// part first, inline PNG second (the transform path's shape).
    pub messages: Vec<ChatMessage>,
    /// Lowercase English color word the answer must contain.
    pub expected: &'static str,
}

/// Build the [`ProbeChallenge`] for one palette color.
#[must_use]
pub fn probe_challenge(color: ProbeColor) -> ProbeChallenge {
    ProbeChallenge {
        messages: vec![ChatMessage::User {
            content: vec![
                text_part(PROBE_PROMPT),
                image_part(png_data_url(&solid_color_png(color.rgb))),
            ],
        }],
        expected: color.name,
    }
}

/// The probe's pass predicate: the answer, lowercased and
/// punctuation-stripped, must *contain* the expected color word —
/// models answer "Red.", "The dominant color is red", "reddish". An
/// answer that never names the color means the model did not look at
/// the image (the silent-ignore failure this probe exists to catch).
#[must_use]
pub fn probe_answer_passes(answer: &str, expected: &str) -> bool {
    let normalize = |text: &str| -> String {
        text.to_ascii_lowercase()
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect()
    };
    normalize(answer).contains(&normalize(expected))
}

/// The seed-injectable challenge color picker: nanoseconds modulo
/// the palette. The bridge feeds `SystemTime` nanos; tests feed
/// chosen seeds. Deliberately not an RNG crate — this is a challenge
/// picker, not crypto, and the workspace takes no new deps for it.
#[must_use]
pub fn probe_color_for(nanos: u128) -> ProbeColor {
    PROBE_PALETTE[(nanos % PROBE_PALETTE.len() as u128) as usize]
}

/// Encode a 64×64 solid-color PNG in memory — small, uniform, and
/// unambiguous for a vision model; indistinguishable from noise for
/// a text-only one.
fn solid_color_png(rgb: [u8; 3]) -> Vec<u8> {
    let [r, g, b] = rgb;
    let image = image::RgbaImage::from_pixel(64, 64, image::Rgba([r, g, b, u8::MAX]));
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        // No failure modes: the cursor writes to memory and the
        // encoder input is well-formed RGBA8; the per-palette decode
        // test catches any encoder regression at test time.
        .expect("in-memory PNG encode cannot fail");
    png
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

    // --- vision-rejection classification (vision-validate T1) ---

    /// The verified `OpenAI` rejection body (research §3.3, seen on
    /// `gpt-4o-search-preview`): an `image_url` part reached a
    /// text-only model.
    #[test]
    fn openai_vision_rejection_400_classifies_to_vision_unsupported() {
        let err = AiError::from_status_and_body(
            400,
            r#"{"error": {"message": "Invalid content type. image_url is only supported by certain models."}}"#,
        );
        match err {
            AiError::VisionUnsupported { status, message } => {
                assert_eq!(status, 400);
                assert!(message.contains("image_url"), "kept: {message}");
            }
            other => panic!("expected VisionUnsupported, got {other:?}"),
        }
    }

    /// Benign image mentions stay `Provider`: "image too large" is a
    /// payload problem, not a capability statement; a generic 5xx
    /// matches nothing.
    #[test]
    fn benign_image_mentions_and_generic_errors_stay_provider() {
        for (status, message) in [
            (400u16, "image too large: the file exceeds the 20 MiB limit"),
            (500u16, "internal server error"),
        ] {
            match AiError::from_status_and_message(status, message.into()) {
                AiError::Provider {
                    status: s,
                    message: m,
                } => {
                    assert_eq!(s, status);
                    assert_eq!(m, message);
                }
                other => panic!("expected Provider for {message:?}, got {other:?}"),
            }
        }
    }

    /// Auth/rate-limit keep priority: a 401 mentioning image models
    /// is a key problem, not a capability verdict.
    #[test]
    fn vision_classification_never_steals_auth_or_rate_limited() {
        assert!(matches!(
            AiError::from_status_and_message(401, "invalid key for image models".into()),
            AiError::Auth(_)
        ));
        assert!(matches!(
            AiError::from_status_and_message(429, "image requests rate limited".into()),
            AiError::RateLimited(_)
        ));
    }

    /// Heuristic tolerance: rewordings providers may ship instead of
    /// the anchored `OpenAI` wording still classify — a miss only
    /// degrades to today's `Provider` card.
    #[test]
    fn reworded_rejection_phrases_classify_as_vision_unsupported() {
        for message in [
            "This model does not support image input",
            "vision input is not supported on this model",
            "multimodal content is only supported by certain models",
        ] {
            match AiError::from_status_and_message(400, message.into()) {
                AiError::VisionUnsupported { .. } => {}
                other => panic!("expected VisionUnsupported for {message:?}, got {other:?}"),
            }
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
    fn vision_probe_replacement_is_content_checking() {
        // Probe v1 pins: ≥8 unambiguous colors, distinct words.
        assert!(
            PROBE_PALETTE.len() >= 8,
            "palette must randomize across ≥8 colors"
        );
        let mut names: Vec<_> = PROBE_PALETTE.iter().map(|c| c.name).collect();
        names.sort_unstable();
        let distinct = names
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        assert_eq!(distinct, PROBE_PALETTE.len(), "color words are distinct");
    }

    /// Every palette entry's challenge PNG decodes as an image of
    /// exactly that color — a mangled generator fails CI here, not
    /// as a provider-side bad-request on the user's machine.
    #[test]
    fn every_palette_challenge_png_decodes_to_its_color() {
        use base64::Engine as _;
        for color in PROBE_PALETTE {
            let challenge = probe_challenge(color);
            assert_eq!(challenge.expected, color.name);
            let [ChatMessage::User { content }] = challenge.messages.as_slice() else {
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
            let decoded = image::load_from_memory(&bytes).expect("decodes as a PNG");
            let [r, g, b] = color.rgb;
            assert_eq!(
                decoded.to_rgba8().get_pixel(0, 0),
                &image::Rgba([r, g, b, u8::MAX]),
                "{} challenge PNG is solid {}",
                color.name,
                color.name
            );
        }
    }

    /// The prompt must ask about the image's content — the pass
    /// predicate is meaningless against a prompt the answer can
    /// satisfy without looking.
    #[test]
    fn probe_prompt_asks_for_the_dominant_color() {
        let challenge = probe_challenge(PROBE_PALETTE[0]);
        let ChatMessage::User { content } = &challenge.messages[0] else {
            panic!("user message");
        };
        let ContentPart::Text { text } = &content[0] else {
            panic!("text part first");
        };
        assert!(
            text.to_ascii_lowercase().contains("dominant color"),
            "prompt was: {text}"
        );
    }

    /// The pass predicate: contains, case- and punctuation-tolerant;
    /// a blind answer that never names the color fails.
    #[test]
    fn probe_answer_passes_on_color_word_in_any_shape() {
        assert!(probe_answer_passes("Red.", "red"));
        assert!(probe_answer_passes("The dominant color is RED", "red"));
        assert!(probe_answer_passes("reddish", "red"));
        assert!(!probe_answer_passes("The image shows a receipt", "red"));
        assert!(!probe_answer_passes("blue", "red"));
        assert!(!probe_answer_passes("", "red"));
    }

    /// The seed-injectable picker: deterministic per seed, and varied
    /// across seeds (a one-color distribution would let a blind
    /// model pass by always guessing the same word).
    #[test]
    fn probe_color_picker_is_deterministic_and_varies() {
        assert_eq!(probe_color_for(0), PROBE_PALETTE[0]);
        assert_eq!(probe_color_for(7), PROBE_PALETTE[7 % PROBE_PALETTE.len()]);
        let distinct: std::collections::BTreeSet<&str> =
            (0..1000u128).map(|n| probe_color_for(n).name).collect();
        assert!(
            distinct.len() > 1,
            "N draws must hit more than one color, got {}",
            distinct.len()
        );
    }
}
