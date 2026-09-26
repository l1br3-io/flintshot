//! The Ask-the-shot request worker — thread + runtime glue.
//!
//! The reqwest future is **only** awaited here: each request spawns a
//! plain [`std::thread`] that builds a current-thread tokio runtime
//! and block-ons the [`AiClient`] call. The result travels back over
//! [`std::sync::mpsc`]; the UI thread polls it via the page bridge
//! (with a repaint timer while pending) — never `block_on` on the UI
//! thread, never a cross-thread `Context` clone.
//!
//! Closing the editor while pending is safe: the receiver drops, the
//! sender's failure is ignored, the thread exits on its own.

use std::sync::mpsc;

use flintshot_core::{AiClient, AiError, AiRequest, ChatMessage, Config, ProviderPreset};

use super::OpenAiCompatClient;

/// The cap for `max_tokens`. xAI requires the field; everyone else
/// accepts it. Generous enough for OCR of a dense screenshot.
const MAX_TOKENS: u32 = 1024;

/// A resolved, ready-to-use provider configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAi {
    /// Chat-completions base URL (no trailing slash).
    pub base_url: String,
    /// Model id for the wire.
    pub model: String,
    /// The provider's API key; empty = keyless (local servers).
    pub api_key: String,
}

/// Resolve `[ai]` config + keyring into a ready request context.
///
/// `Err` carries one-line actionable copy (the dialog's Configure
/// gate and the immediate-failure event share it). Keyless is valid
/// **only** for custom endpoints: a hosted preset without a stored
/// key is not configured.
pub fn resolve_ai(config: &Config) -> Result<ResolvedAi, String> {
    let preset: &'static ProviderPreset = config.ai.preset();
    if config.ai.model.trim().is_empty() {
        return Err("No model set — pick one in AI settings".into());
    }
    let base_url = config.ai.effective_base_url();
    if base_url.is_empty() {
        return Err("No provider endpoint — set the base URL in AI settings".into());
    }
    let api_key = super::load_ai_key(&config.ai.provider).unwrap_or_default();
    if preset.id != "custom" && api_key.is_empty() {
        return Err(format!(
            "No {} API key stored — add it in AI settings",
            preset.name
        ));
    }
    Ok(ResolvedAi {
        base_url: base_url.to_string(),
        model: config.ai.model.trim().to_string(),
        api_key,
    })
}

/// Whether Run is offered (the dialog's configured gate).
#[must_use]
pub fn ai_configured(config: &Config) -> bool {
    resolve_ai(config).is_ok()
}

/// Spawn the request worker: a `std::thread` with its own
/// current-thread tokio runtime that block-ons the chat call and
/// sends exactly one result. Sender failure (receiver dropped — the
/// editor closed) is ignored by construction: `send` returns `Err`
/// and the thread simply exits.
pub fn spawn_chat_worker(
    resolved: ResolvedAi,
    messages: Vec<ChatMessage>,
) -> mpsc::Receiver<Result<String, AiError>> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("flintshot-ai-request".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = tx.send(Err(AiError::Transport(format!(
                        "building the request runtime: {e}"
                    ))));
                    return;
                }
            };
            let request = AiRequest {
                model: resolved.model,
                messages,
                max_tokens: Some(MAX_TOKENS),
                stream: None,
                stream_options: None,
            };
            let client =
                OpenAiCompatClient::with_base_url_and_key(&resolved.base_url, &resolved.api_key);
            let result = rt
                .block_on(client.chat(&request))
                .map(|response| response.first_text().unwrap_or_default().to_string());
            let _ = tx.send(result);
        })
        .expect("spawning the AI request worker thread");
    rx
}

/// Map a provider error to one-line, next-step copy for the AI tab's
/// error card. The raw error text rides along where it helps.
#[must_use]
pub fn ai_error_copy(error: &AiError) -> String {
    match error {
        AiError::Transport(_) => {
            "Couldn't reach the provider — check the base URL and network, then retry.".into()
        }
        AiError::Auth(_) => "The provider rejected the key — replace it in AI settings.".into(),
        AiError::RateLimited(_) => "Rate limited — wait a moment, then retry.".into(),
        AiError::EmptyResponse(_) => {
            "The model returned no text — retry, or pick another model in AI settings.".into()
        }
        // Provider bodies already carry status + message (and any
        // future variant falls through with its Display form).
        _ => format!("{error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(provider: &str, model: &str, base_url: &str) -> Config {
        let mut config = Config::default();
        config.ai.provider = provider.into();
        config.ai.model = model.into();
        config.ai.base_url = base_url.into();
        config
    }

    #[test]
    fn resolve_requires_a_model() {
        let err = resolve_ai(&config("openai", "  ", "")).unwrap_err();
        assert!(err.contains("model"), "copy was: {err}");
    }

    #[test]
    fn resolve_requires_an_endpoint() {
        let err = resolve_ai(&config("custom", "m", "")).unwrap_err();
        assert!(err.contains("endpoint"), "copy was: {err}");
    }

    #[test]
    fn resolve_rejects_keyless_hosted_presets() {
        // Keyring-less CI resolves as no key stored: a hosted preset
        // without a key is not configured.
        let result = resolve_ai(&config("openai", "gpt-4o-mini", ""));
        match result {
            Ok(resolved) => {
                // A key exists on this machine (a real keyring run):
                // then it must be the OpenAI endpoint.
                assert_eq!(resolved.base_url, "https://api.openai.com/v1");
            }
            Err(err) => assert!(err.contains("API key"), "copy was: {err}"),
        }
    }

    #[test]
    fn resolve_allows_keyless_custom_local_servers() {
        let resolved = resolve_ai(&config(
            "custom",
            "llama3.2-vision",
            "http://localhost:11434/v1",
        ))
        .expect("custom endpoints run keyless");
        assert_eq!(resolved.base_url, "http://localhost:11434/v1");
        assert_eq!(resolved.model, "llama3.2-vision");
        assert_eq!(resolved.api_key, "");
    }

    #[test]
    fn error_copy_is_actionable_per_variant() {
        assert!(ai_error_copy(&AiError::Transport("x".into())).contains("reach"));
        assert!(ai_error_copy(&AiError::Auth("x".into())).contains("key"));
        assert!(ai_error_copy(&AiError::RateLimited("x".into())).contains("Rate"));
        assert!(ai_error_copy(&AiError::EmptyResponse("x".into())).contains("no text"));
    }

    #[test]
    fn worker_round_trips_against_a_mock_server() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("POST", "/chat/completions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"choices":[{"index":0,"message":{"role":"assistant","content":"hello"},"finish_reason":"stop"}]}"#)
            .create();
        let resolved = ResolvedAi {
            base_url: server.url(),
            model: "m".into(),
            api_key: String::new(),
        };
        let rx = spawn_chat_worker(
            resolved,
            vec![flintshot_core::ChatMessage::User {
                content: vec![flintshot_core::text_part("hi")],
            }],
        );
        let result = rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("the worker answers");
        assert_eq!(result.expect("mock 2xx"), "hello");
        mock.assert();
    }

    #[test]
    fn worker_surfaces_provider_errors() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/chat/completions")
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": {"message": "bad key"}}"#)
            .create();
        let resolved = ResolvedAi {
            base_url: server.url(),
            model: "m".into(),
            api_key: "k".into(),
        };
        let rx = spawn_chat_worker(
            resolved,
            vec![flintshot_core::ChatMessage::User {
                content: vec![flintshot_core::text_part("hi")],
            }],
        );
        let result = rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("the worker answers");
        assert!(matches!(result, Err(AiError::Auth(_))));
    }
}
