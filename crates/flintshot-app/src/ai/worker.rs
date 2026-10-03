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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

use flintshot_core::{
    AiClient, AiError, AiRequest, ChatMessage, Config, ProviderPreset, SseDecoder, SseEvent,
};

use super::{ChatStreamStart, OpenAiCompatClient};

/// The cap for `max_tokens`. xAI requires the field; everyone else
/// accepts it. Generous enough for OCR of a dense screenshot.
const MAX_TOKENS: u32 = 1024;

/// One event from a streaming chat worker.
///
/// Protocol invariant: zero or more [`Delta`](ChatWorkerEvent::Delta)
/// events in arrival order, then **exactly one** terminal
/// ([`Done`](ChatWorkerEvent::Done) or
/// [`Failed`](ChatWorkerEvent::Failed)), then the channel closes. The
/// worker is the accumulation authority — `Done` carries the
/// authoritative full text (every delta it emitted, concatenated), so
/// the UI never drifts from what was received.
#[derive(Debug)]
#[non_exhaustive]
pub enum ChatWorkerEvent {
    /// A text delta, in arrival order.
    Delta(String),
    /// Terminal success: the full final text.
    Done(String),
    /// Terminal failure: the stream (or the fallback) failed. Text
    /// already delivered via `Delta` stays valid — the UI keeps the
    /// partial and appends the error card.
    Failed(AiError),
}

/// The user-facing stop signal for a streaming worker: set the flag
/// and the pump ends the run at its next observation (between chunks
/// immediately; within the silence tick once Task 005 races the
/// awaits). Clone-cheap, idempotent, safe to call after the worker
/// finished — a late signal just lands on a dead flag.
#[derive(Debug, Clone, Default)]
pub struct StopHandle {
    flag: Arc<AtomicBool>,
}

impl StopHandle {
    /// A fresh, un-set handle (the worker thread holds a clone).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Signal the worker to stop. Stopping finalizes the partial
    /// text as the response — it is a user action, never an error.
    pub fn stop(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    /// Whether stop was signalled (observable state; the UI's Stop
    /// pill wiring and tests use it).
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}

/// The worker's byte-source seam: `next_chunk` returns the next body
/// chunk, a transport error, or `None` once the body is delivered
/// cleanly. The HTTP implementation adapts
/// [`reqwest::Response::chunk`]; tests hand-build sources — the pump
/// is unit-testable with no server at all.
///
/// (App-crate-private by design: core stays I/O-free.)
trait ChunkSource {
    fn next_chunk(&mut self)
        -> impl std::future::Future<Output = Option<Result<Vec<u8>, AiError>>>;
}

/// The HTTP [`ChunkSource`]: owns the streaming response. Dropping it
/// (pump return, stop, worker exit) closes the connection.
struct HttpResponseSource {
    response: reqwest::Response,
}

impl ChunkSource for HttpResponseSource {
    async fn next_chunk(&mut self) -> Option<Result<Vec<u8>, AiError>> {
        match self.response.chunk().await {
            Ok(Some(bytes)) => Some(Ok(bytes.to_vec())),
            Ok(None) => None,
            Err(e) => Some(Err(AiError::Transport(e.to_string()))),
        }
    }
}

/// Forward decoded SSE events over the channel, accumulating the
/// authoritative text. Malformed payloads are skipped (logged), not
/// fatal — the decoder stays usable for subsequent lines. Returns
/// whether the `[DONE]` sentinel was seen.
fn forward_events(
    events: impl IntoIterator<Item = SseEvent>,
    tx: &mpsc::Sender<ChatWorkerEvent>,
    accumulated: &mut String,
) -> bool {
    let mut done = false;
    for event in events {
        match event {
            SseEvent::Delta(text) => {
                accumulated.push_str(&text);
                let _ = tx.send(ChatWorkerEvent::Delta(text));
            }
            SseEvent::Done => done = true,
            SseEvent::Malformed(message) => {
                tracing::debug!("skipping malformed SSE payload: {message}");
            }
            // `SseEvent` is non_exhaustive; unknown future events
            // carry no text and no terminal — ignoring is correct.
            _ => {}
        }
    }
    done
}

/// How often the pump re-checks the stop flag while awaiting body
/// chunks: stop must land within ~100 ms even against a fully
/// silent provider.
const STOP_POLL: std::time::Duration = std::time::Duration::from_millis(50);

/// The streaming pump: bytes in from a [`ChunkSource`], decoder
/// events out over the channel. Returns the accumulated full text on
/// a clean end (sentinel or EOF), the transport error on a mid-stream
/// failure, or [`AiError::EmptyResponse`] when a naturally-ended
/// stream carried no text at all. A stop signal ends the run as a
/// success carrying whatever partial text arrived (possibly none).
async fn pump_stream<S: ChunkSource>(
    mut source: S,
    tx: &mpsc::Sender<ChatWorkerEvent>,
    stop: &AtomicBool,
) -> Result<String, AiError> {
    let mut decoder = SseDecoder::new();
    let mut accumulated = String::new();
    let mut sse_events = Vec::new();
    let mut sentinel_seen = false;
    while !stop.load(Ordering::SeqCst) {
        // Race the chunk await against the stop tick: `biased` keeps
        // a ready chunk winning, and a ticked read is re-issued next
        // iteration (reqwest chunk reads are cancel-safe — an
        // incomplete read consumes nothing from the connection).
        let chunk = tokio::select! {
            biased;
            chunk = source.next_chunk() => Some(chunk),
            () = tokio::time::sleep(STOP_POLL) => None,
        };
        let Some(chunk) = chunk else {
            continue; // tick elapsed — loop head re-checks the flag
        };
        let Some(result) = chunk else {
            break; // clean end-of-body
        };
        let bytes = result?;
        decoder.feed(&bytes, &mut sse_events);
        if forward_events(sse_events.drain(..), tx, &mut accumulated) {
            sentinel_seen = true;
            break;
        }
    }
    if !sentinel_seen && !stop.load(Ordering::SeqCst) {
        // Clean end-of-body without the sentinel: flush any trailing
        // unterminated event the decoder still holds.
        decoder.finish(&mut sse_events);
        forward_events(sse_events.drain(..), tx, &mut accumulated);
    }
    if accumulated.is_empty() && !stop.load(Ordering::SeqCst) {
        return Err(AiError::EmptyResponse(
            "the stream carried no assistant text".into(),
        ));
    }
    Ok(accumulated)
}

/// Non-streaming completion over the same event channel: the
/// transparent fallback (a 2xx non-SSE body) and the shared request
/// body builder. Emits one `Delta` carrying the whole text so a
/// fallback-completed run looks exactly like a (fast) stream.
async fn complete_non_streaming(
    client: &OpenAiCompatClient,
    request: &AiRequest,
    tx: &mpsc::Sender<ChatWorkerEvent>,
) -> Result<String, AiError> {
    let mut request = request.clone();
    request.stream = None;
    let response = client.chat(&request).await?;
    let text = response.first_text().unwrap_or_default().to_string();
    if !text.is_empty() {
        let _ = tx.send(ChatWorkerEvent::Delta(text.clone()));
    }
    Ok(text)
}

/// The fallback contract's subtlest clause — checked exactly here,
/// before any delta can exist: retry non-streaming **only** when the
/// stream never opened and the failure says the provider rejected
/// streaming itself, i.e. an HTTP 400 whose provider message mentions
/// `stream` (case-insensitive). A 400 about anything else (bad
/// model, oversized image) would fail identically without streaming,
/// so a retry would only double the bill. Mid-stream failures never
/// reach this check — after any delta, failure is terminal.
fn is_stream_rejection(err: &AiError) -> bool {
    match err {
        AiError::Provider {
            status: 400,
            message,
        } => message.to_ascii_lowercase().contains("stream"),
        _ => false,
    }
}

/// Drive one streaming run to its terminal outcome (the thread body
/// after runtime construction): open the stream, pump it, or fall
/// back when the provider refuses streaming. Both fallback triggers
/// occur before any delta is emitted, satisfying the spec's
/// "zero deltas" precondition by construction.
async fn run_stream(
    client: &OpenAiCompatClient,
    request: &AiRequest,
    tx: &mpsc::Sender<ChatWorkerEvent>,
    stop: &AtomicBool,
) -> Result<String, AiError> {
    match client.chat_stream(request).await {
        Ok(ChatStreamStart::Sse(response)) => {
            pump_stream(HttpResponseSource { response }, tx, stop).await
        }
        Ok(ChatStreamStart::NotSse(response)) => {
            // The provider ignored `stream: true`. Drain the body
            // (closing it) and retry non-streaming — invisible to
            // the conversation.
            drop(response);
            complete_non_streaming(client, request, tx).await
        }
        Err(err) if is_stream_rejection(&err) => {
            // The provider rejected `stream: true` outright — same
            // transparent non-streaming retry.
            complete_non_streaming(client, request, tx).await
        }
        Err(err) => Err(err),
    }
}

fn build_request(model: String, messages: Vec<ChatMessage>, stream: bool) -> AiRequest {
    AiRequest {
        model,
        messages,
        max_tokens: Some(MAX_TOKENS),
        stream: stream.then_some(true),
    }
}

/// Spawn the request worker: a `std::thread` with its own
/// current-thread tokio runtime that streams the chat completion and
/// forwards [`ChatWorkerEvent`]s — zero or more `Delta`s, then
/// exactly one terminal (`Done` with the authoritative full text, or
/// `Failed`), then the channel closes. The [`StopHandle`] ends the
/// run as a success carrying the partial. Sender failure (receiver
/// dropped — the editor closed) is ignored by construction: `send`
/// returns `Err` and the thread simply exits.
pub fn spawn_chat_worker(
    resolved: ResolvedAi,
    messages: Vec<ChatMessage>,
) -> (mpsc::Receiver<ChatWorkerEvent>, StopHandle) {
    let (tx, rx) = mpsc::channel();
    let stop = StopHandle::new();
    let stop_flag = Arc::clone(&stop.flag);
    std::thread::Builder::new()
        .name("flintshot-ai-request".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = tx.send(ChatWorkerEvent::Failed(AiError::Transport(format!(
                        "building the request runtime: {e}"
                    ))));
                    return;
                }
            };
            let request = build_request(resolved.model, messages, true);
            let client =
                OpenAiCompatClient::with_base_url_and_key(&resolved.base_url, &resolved.api_key);
            let result = rt.block_on(run_stream(&client, &request, &tx, &stop_flag));
            let _ = tx.send(match result {
                Ok(text) => ChatWorkerEvent::Done(text),
                Err(err) => ChatWorkerEvent::Failed(err),
            });
        })
        .expect("spawning the AI request worker thread");
    (rx, stop)
}

/// Spawn the one-shot, non-streaming probe worker
/// (`ai_test_connection`): same thread + runtime shape, but a single
/// `Result<String, AiError>` — the probe wants a whole answer and a
/// one-shot receiver, never a stream.
pub fn spawn_probe_worker(
    resolved: ResolvedAi,
    messages: Vec<ChatMessage>,
) -> mpsc::Receiver<Result<String, AiError>> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("flintshot-ai-probe".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = tx.send(Err(AiError::Transport(format!(
                        "building the probe runtime: {e}"
                    ))));
                    return;
                }
            };
            let request = build_request(resolved.model, messages, false);
            let client =
                OpenAiCompatClient::with_base_url_and_key(&resolved.base_url, &resolved.api_key);
            let result = rt
                .block_on(client.chat(&request))
                .map(|response| response.first_text().unwrap_or_default().to_string());
            let _ = tx.send(result);
        })
        .expect("spawning the AI probe worker thread");
    rx
}

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

/// Map a provider error to one-line, next-step copy for the AI tab's
/// error card. The raw error text rides along where it helps.
#[must_use]
pub fn ai_error_copy(error: &AiError) -> String {
    match error {
        AiError::Transport(_) => {
            "Couldn't reach the provider — check the base URL and network, then retry.".into()
        }
        AiError::Auth(detail) => format!(
            "The provider rejected the key — replace it in AI settings. Provider said: {detail}"
        ),
        AiError::RateLimited(detail) => {
            format!("Rate limited — wait a moment, then retry. Provider said: {detail}")
        }
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

    /// Regression: the `Auth` and `RateLimited` copies discarded the
    /// provider's message, making 401s undiscoverable from the UI —
    /// "replace the key" vs "no access to this model" vs a keyless
    /// probe all rendered identically. The detail now rides along
    /// after the actionable headline.
    #[test]
    fn auth_and_rate_limit_copy_carry_the_provider_detail() {
        let copy = ai_error_copy(&AiError::Auth("Invalid API key provided".into()));
        assert!(
            copy.contains("replace it in AI settings"),
            "headline stays: {copy}"
        );
        assert!(
            copy.contains("Invalid API key provided"),
            "provider detail rides along: {copy}"
        );

        let copy = ai_error_copy(&AiError::RateLimited("quota exhausted".into()));
        assert!(copy.contains("wait a moment"), "headline stays: {copy}");
        assert!(
            copy.contains("quota exhausted"),
            "detail rides along: {copy}"
        );
    }

    #[test]
    fn probe_worker_round_trips_against_a_mock_server() {
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
        let rx = spawn_probe_worker(
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
    fn probe_worker_surfaces_provider_errors() {
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
        let rx = spawn_probe_worker(
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

    // --- streaming worker (Task 004) ---

    /// A scripted byte source for seam tests: yields each scripted
    /// chunk in order; an error surfaces as-is; exhaustion is a
    /// clean end-of-body. No HTTP required.
    struct ScriptedSource {
        script: std::collections::VecDeque<Result<Vec<u8>, AiError>>,
    }

    impl ScriptedSource {
        fn chunks(chunks: &[&[u8]]) -> Self {
            Self {
                script: chunks.iter().map(|c| Ok((*c).to_vec())).collect(),
            }
        }
    }

    impl ChunkSource for ScriptedSource {
        async fn next_chunk(&mut self) -> Option<Result<Vec<u8>, AiError>> {
            self.script.pop_front()
        }
    }

    fn sse(text: &str) -> Vec<u8> {
        format!("data: {{\"choices\":[{{\"delta\":{{\"content\":\"{text}\"}}}}]}}\n\n").into_bytes()
    }

    fn collect_events(rx: &mpsc::Receiver<ChatWorkerEvent>) -> Vec<ChatWorkerEvent> {
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        events
    }

    fn delta_texts(events: &[ChatWorkerEvent]) -> Vec<&str> {
        events
            .iter()
            .filter_map(|event| match event {
                ChatWorkerEvent::Delta(text) => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    fn terminal_count(events: &[ChatWorkerEvent]) -> usize {
        events
            .iter()
            .filter(|event| matches!(event, ChatWorkerEvent::Done(_) | ChatWorkerEvent::Failed(_)))
            .count()
    }

    fn user_message() -> Vec<flintshot_core::ChatMessage> {
        vec![flintshot_core::ChatMessage::User {
            content: vec![flintshot_core::text_part("hi")],
        }]
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
    async fn pump_concatenates_split_feed_deltas_in_arrival_order() {
        let (tx, rx) = mpsc::channel();
        let source = ScriptedSource::chunks(&[
            &sse("Hel"),
            b"data: {\"choices\":[{\"delta\":{\"con",
            b"tent\":\"lo\"}}]}\n\n",
            &sse(" world"),
            b"data: [DONE]\n\n",
        ]);
        let stop = std::sync::atomic::AtomicBool::new(false);
        let full = pump_stream(source, &tx, &stop).await.expect("clean stream");
        assert_eq!(full, "Hello world");
        assert_eq!(
            delta_texts(&collect_events(&rx)),
            ["Hel", "lo", " world"],
            "the terminal rides the return value, not the channel"
        );
    }

    #[tokio::test]
    async fn pump_mid_stream_transport_error_is_terminal_after_deltas() {
        let (tx, rx) = mpsc::channel();
        let mut script = ScriptedSource::chunks(&[&sse("a"), &sse("b")]).script;
        script.push_back(Err(AiError::Transport("connection reset".into())));
        let stop = std::sync::atomic::AtomicBool::new(false);
        let result = pump_stream(ScriptedSource { script }, &tx, &stop).await;
        assert!(
            matches!(result, Err(AiError::Transport(_))),
            "got {result:?}"
        );
        assert_eq!(
            delta_texts(&collect_events(&rx)),
            ["a", "b"],
            "deltas already delivered stay delivered"
        );
    }

    #[tokio::test]
    async fn pump_skips_malformed_lines_without_aborting() {
        let (tx, rx) = mpsc::channel();
        let source =
            ScriptedSource::chunks(&[b"data: not-json\n\n", &sse("ok"), b"data: [DONE]\n\n"]);
        let stop = std::sync::atomic::AtomicBool::new(false);
        let full = pump_stream(source, &tx, &stop).await.expect("clean");
        assert_eq!(full, "ok");
        assert_eq!(delta_texts(&collect_events(&rx)), ["ok"]);
    }

    #[tokio::test]
    async fn pump_clean_eof_without_sentinel_still_completes() {
        let (tx, rx) = mpsc::channel();
        let source = ScriptedSource::chunks(&[&sse("tail")]);
        let stop = std::sync::atomic::AtomicBool::new(false);
        let full = pump_stream(source, &tx, &stop)
            .await
            .expect("clean EOF is a successful end");
        assert_eq!(full, "tail");
        assert_eq!(delta_texts(&collect_events(&rx)), ["tail"]);
    }

    #[tokio::test]
    async fn pump_empty_stream_maps_to_empty_response_error() {
        let (tx, _rx) = mpsc::channel();
        let source = ScriptedSource::chunks(&[
            b"data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\n",
            b"data: [DONE]\n\n",
        ]);
        let stop = std::sync::atomic::AtomicBool::new(false);
        let result = pump_stream(source, &tx, &stop).await;
        assert!(
            matches!(result, Err(AiError::EmptyResponse(_))),
            "got {result:?}"
        );
    }

    #[tokio::test]
    async fn pump_stop_between_chunks_finalizes_the_partial() {
        use std::sync::atomic::Ordering;

        /// Yields the scripted chunks but flips the stop flag while
        /// yielding the first — the pump must deliver that chunk,
        /// then finalize without reading further.
        struct FlagOnYieldSource {
            chunks: std::collections::VecDeque<Vec<u8>>,
            stop: Arc<AtomicBool>,
        }
        impl ChunkSource for FlagOnYieldSource {
            async fn next_chunk(&mut self) -> Option<Result<Vec<u8>, AiError>> {
                let chunk = self.chunks.pop_front()?;
                self.stop.store(true, Ordering::SeqCst);
                Some(Ok(chunk))
            }
        }

        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let source = FlagOnYieldSource {
            chunks: std::collections::VecDeque::from(vec![sse("part"), sse(" never")]),
            stop: Arc::clone(&stop),
        };
        let full = pump_stream(source, &tx, &stop)
            .await
            .expect("stop is a user action, not an error");
        assert_eq!(full, "part", "the post-stop chunk must not be read");
        assert_eq!(delta_texts(&collect_events(&rx)), ["part"]);
    }

    #[tokio::test]
    async fn pump_stop_with_zero_deltas_yields_empty_done_not_error() {
        let (tx, _rx) = mpsc::channel();
        let source = ScriptedSource::chunks(&[]);
        let stop = std::sync::atomic::AtomicBool::new(true);
        let full = pump_stream(source, &tx, &stop)
            .await
            .expect("stop with nothing received is still not an error");
        assert_eq!(full, "");
    }

    /// A provider that never sends a byte: `next_chunk` never
    /// resolves. The stop tick must still end the pump.
    struct SilentSource;

    impl ChunkSource for SilentSource {
        async fn next_chunk(&mut self) -> Option<Result<Vec<u8>, AiError>> {
            std::future::pending().await
        }
    }

    #[tokio::test]
    async fn pump_stop_during_silence_returns_within_the_tick_bound() {
        use std::sync::atomic::Ordering;

        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopper = Arc::clone(&stop);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            stopper.store(true, Ordering::SeqCst);
        });
        let started = std::time::Instant::now();
        let full = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            pump_stream(SilentSource, &tx, &stop),
        )
        .await
        .expect("stop must end an otherwise-silent pump")
        .expect("stop is a user action, not an error");
        assert_eq!(full, "");
        assert!(
            delta_texts(&collect_events(&rx)).is_empty(),
            "no deltas may appear after stop during silence"
        );
        // ~50 ms tick + slack: well under the 500 ms timeout, and
        // bounded far away from "never".
        assert!(started.elapsed() < std::time::Duration::from_millis(400));
    }

    #[test]
    fn streaming_worker_falls_back_on_a_400_that_mentions_stream() {
        // The provider rejects `stream: true` with a 400 whose
        // message names streaming — retried once, non-streaming,
        // invisibly: Delta(full) + Done(full).
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/chat/completions")
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "model": "m",
                "messages": [{"role": "user", "content": [{"type": "text", "text": "hi"}]}],
                "max_tokens": 1024,
                "stream": true
            })))
            .with_status(400)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": {"message": "streaming is not supported for this model"}}"#)
            .create();
        server
            .mock("POST", "/chat/completions")
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "model": "m",
                "messages": [{"role": "user", "content": [{"type": "text", "text": "hi"}]}],
                "max_tokens": 1024
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"choices":[{"index":0,"message":{"role":"assistant","content":"whole"},"finish_reason":"stop"}]}"#)
            .create();
        let resolved = ResolvedAi {
            base_url: server.url(),
            model: "m".into(),
            api_key: String::new(),
        };
        let (rx, _stop) = spawn_chat_worker(resolved, user_message());
        let mut events = Vec::new();
        while let Ok(event) = rx.recv_timeout(std::time::Duration::from_secs(10)) {
            events.push(event);
        }
        assert_eq!(
            terminal_count(&events),
            1,
            "exactly one terminal: {events:?}"
        );
        assert_eq!(delta_texts(&events), ["whole"]);
        assert!(
            matches!(&events[1], ChatWorkerEvent::Done(text) if text == "whole"),
            "the retry pipes through the same protocol: {events:?}"
        );
    }

    #[test]
    fn streaming_worker_does_not_retry_a_plain_400() {
        // A 400 that never mentions `stream` (e.g. a bad model id)
        // is a plain provider error — no hidden second request.
        let mut server = mockito::Server::new();
        let mock = server
            .mock("POST", "/chat/completions")
            .with_status(400)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": {"message": "The model `m` does not exist"}}"#)
            .expect(1)
            .create();
        let resolved = ResolvedAi {
            base_url: server.url(),
            model: "m".into(),
            api_key: String::new(),
        };
        let (rx, _stop) = spawn_chat_worker(resolved, user_message());
        let mut events = Vec::new();
        while let Ok(event) = rx.recv_timeout(std::time::Duration::from_secs(10)) {
            events.push(event);
        }
        assert_eq!(
            terminal_count(&events),
            1,
            "exactly one terminal: {events:?}"
        );
        assert!(matches!(
            &events[0],
            ChatWorkerEvent::Failed(AiError::Provider { status: 400, .. })
        ));
        // The stream-rejection retry never fired: exactly one hit.
        std::thread::sleep(std::time::Duration::from_millis(100));
        mock.assert();
    }

    #[test]
    fn receiver_drop_mid_stream_exits_the_worker_cleanly() {
        // Editor close mid-stream: the receiver drops, the worker's
        // sends fail into the void, the thread exits. No panic, no
        // hang — and a fresh worker on the same endpoint still works.
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/chat/completions")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse_body())
            .create();
        let resolved = ResolvedAi {
            base_url: server.url(),
            model: "m".into(),
            api_key: String::new(),
        };
        let (rx, _stop) = spawn_chat_worker(resolved.clone(), user_message());
        // A delta arrives, then the "editor closes".
        let first = rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("the stream starts");
        assert!(matches!(first, ChatWorkerEvent::Delta(_)));
        drop(rx);
        // Let the worker run into its failed sends.
        std::thread::sleep(std::time::Duration::from_millis(200));

        // The endpoint still serves a fresh worker (nothing wedged).
        let (rx2, _stop2) = spawn_chat_worker(resolved, user_message());
        let mut events = Vec::new();
        while let Ok(event) = rx2.recv_timeout(std::time::Duration::from_secs(10)) {
            events.push(event);
        }
        assert_eq!(terminal_count(&events), 1);
        assert!(matches!(events.last(), Some(ChatWorkerEvent::Done(_))));
    }

    #[test]
    fn streaming_worker_streams_deltas_then_done() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/chat/completions")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse_body())
            .create();
        let resolved = ResolvedAi {
            base_url: server.url(),
            model: "m".into(),
            api_key: String::new(),
        };
        let (rx, _stop) = spawn_chat_worker(resolved, user_message());
        let mut events = Vec::new();
        while let Ok(event) = rx.recv_timeout(std::time::Duration::from_secs(10)) {
            events.push(event);
        }
        assert_eq!(
            delta_texts(&events),
            ["Hel", "lo, ", "world"],
            "deltas arrive in order: {events:?}"
        );
        assert_eq!(
            terminal_count(&events),
            1,
            "exactly one terminal: {events:?}"
        );
        assert!(
            matches!(&events[3], ChatWorkerEvent::Done(text) if text == "Hello, world"),
            "Done carries the authoritative concatenation: {events:?}"
        );
    }

    #[test]
    fn streaming_worker_falls_back_to_non_streaming_on_json_body() {
        // A 2xx JSON body (stream-rejecting proxy shape) retries via
        // non-streaming chat() and pipes the result through the same
        // protocol: Delta(full) + Done(full), no error surfaced.
        let mut server = mockito::Server::new();
        server
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
        let (rx, _stop) = spawn_chat_worker(resolved, user_message());
        let mut events = Vec::new();
        while let Ok(event) = rx.recv_timeout(std::time::Duration::from_secs(10)) {
            events.push(event);
        }
        assert_eq!(
            terminal_count(&events),
            1,
            "exactly one terminal: {events:?}"
        );
        assert_eq!(delta_texts(&events), ["hello"]);
        assert!(
            matches!(&events[1], ChatWorkerEvent::Done(text) if text == "hello"),
            "fallback completes transparently: {events:?}"
        );
    }

    #[test]
    fn streaming_worker_maps_provider_errors_to_failed() {
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
        let (rx, _stop) = spawn_chat_worker(resolved, user_message());
        let mut events = Vec::new();
        while let Ok(event) = rx.recv_timeout(std::time::Duration::from_secs(10)) {
            events.push(event);
        }
        assert_eq!(
            terminal_count(&events),
            1,
            "exactly one terminal: {events:?}"
        );
        assert!(matches!(&events[0], ChatWorkerEvent::Failed(_)));
        assert!(delta_texts(&events).is_empty());
    }
}
