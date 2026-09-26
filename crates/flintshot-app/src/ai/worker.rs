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

use flintshot_core::{
    parse_stream_data_line, AiClient, AiError, AiRequest, AiStreamEvent, AiStreamOptions,
    ChatMessage, Config, ProviderPreset,
};

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

/// One streamed event from a chat-stream worker.
#[derive(Debug)]
#[non_exhaustive]
pub enum StreamMsg {
    /// A text delta to append to the growing assistant turn.
    Delta(String),
    /// The stream ended (`[DONE]` or a clean close — both valid).
    Done,
    /// The stream failed with a classified error.
    Failed(AiError),
}

/// Spawn the streaming request worker: same shape as
/// [`spawn_chat_worker`] (own thread + current-thread runtime), but
/// the POST carries `stream: true` and the SSE response is forwarded
/// as [`StreamMsg::Delta`]s. Returns the receiver plus the thread
/// handle (tests join it; production drops it).
///
/// GOTCHA (drop-to-cancel): reqwest has no cancel API — dropping the
/// response/stream is the only mechanism. The UI's Stop drops the
/// receiver; this worker's next `send` fails, the loop returns, the
/// response drops, and the connection is torn down. Partial text
/// already delivered stays with the caller.
pub fn spawn_chat_stream_worker(
    resolved: ResolvedAi,
    messages: Vec<ChatMessage>,
) -> (mpsc::Receiver<StreamMsg>, std::thread::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    let handle = std::thread::Builder::new()
        .name("flintshot-ai-stream".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = tx.send(StreamMsg::Failed(AiError::Transport(format!(
                        "building the stream runtime: {e}"
                    ))));
                    return;
                }
            };
            let request = AiRequest {
                model: resolved.model,
                messages,
                max_tokens: Some(MAX_TOKENS),
                stream: Some(true),
                stream_options: Some(AiStreamOptions {
                    include_usage: true,
                }),
            };
            let client = OpenAiCompatClient::streaming_with_base_url_and_key(
                &resolved.base_url,
                &resolved.api_key,
            );
            let outcome = rt.block_on(run_stream(&client, &request, &tx));
            // The terminal event; a send failure here is the Stop
            // case (receiver already gone) — ignored.
            let _ = tx.send(match outcome {
                Ok(()) => StreamMsg::Done,
                Err(error) => StreamMsg::Failed(error),
            });
        })
        .expect("spawning the AI stream worker thread");
    (rx, handle)
}

/// Drive one SSE chat stream to its end, forwarding deltas.
///
/// Every path that stops consuming (`[DONE]`, clean close, Stop,
/// error) returns here so the response — and with it the connection
/// — drops exactly once, on the worker thread.
async fn run_stream(
    client: &OpenAiCompatClient,
    request: &AiRequest,
    tx: &mpsc::Sender<StreamMsg>,
) -> Result<(), AiError> {
    use eventsource_stream::Eventsource;
    use futures_util::StreamExt;

    let response = client.post_stream(request).await?;
    let mut events = response.bytes_stream().eventsource();
    while let Some(item) = events.next().await {
        let event = match item {
            Ok(event) => event,
            // Transport errors from the underlying body stream
            Err(eventsource_stream::EventStreamError::Transport(e)) => {
                return Err(AiError::Transport(e.to_string()));
            }
            Err(other) => {
                return Err(AiError::Transport(format!(
                    "reading the event stream: {other}"
                )));
            }
        };
        // Keepalives arrive as named events or comment lines
        // (comments are dropped by eventsource-stream already).
        if event.event == "keepalive" || event.event == "ping" {
            continue;
        }
        // Role-only chunks and unknown lines: skipped, never fatal.
        let Some(event) = parse_stream_data_line(&event.data) else {
            continue;
        };
        match event {
            AiStreamEvent::Token(text) => {
                let stopped = tx.send(StreamMsg::Delta(text)).is_err();
                // Stop: the receiver is gone. Returning drops the
                // stream — reqwest tears the connection down.
                if stopped {
                    return Ok(());
                }
            }
            AiStreamEvent::Done => return Ok(()),
            // Usage parses for provider tolerance (unshown in v2).
            _ => {}
        }
    }
    // Underlying body ended without `[DONE]` — also a clean close.
    Ok(())
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

    // ---- streaming worker (Task 002) ----

    /// Join a worker thread under a timeout; panic on hang or panic.
    fn join_with_timeout(handle: std::thread::JoinHandle<()>, timeout: std::time::Duration) {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(handle.join());
        });
        match rx.recv_timeout(timeout) {
            Ok(Ok(())) => {}
            Ok(Err(panic)) => panic!("worker thread panicked: {panic:?}"),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                panic!("worker thread did not exit within {timeout:?}");
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                panic!("join watcher exited without a result");
            }
        }
    }

    fn collect_stream(rx: &mpsc::Receiver<StreamMsg>) -> (String, Option<AiError>) {
        let mut text = String::new();
        let mut error = None;
        while let Ok(msg) = rx.recv_timeout(std::time::Duration::from_secs(10)) {
            match msg {
                StreamMsg::Delta(text_delta) => text.push_str(&text_delta),
                StreamMsg::Done => return (text, error),
                StreamMsg::Failed(e) => {
                    error = Some(e);
                    return (text, error);
                }
            }
        }
        panic!("stream ended without Done or Failed");
    }

    fn stream_resolved(server: &mockito::Server) -> ResolvedAi {
        ResolvedAi {
            base_url: server.url(),
            model: "m".into(),
            api_key: String::new(),
        }
    }

    fn one_user_message() -> Vec<ChatMessage> {
        vec![ChatMessage::User {
            content: vec![flintshot_core::text_part("hi")],
        }]
    }

    #[test]
    fn stream_worker_happy_path_deltas_then_done() {
        // OpenAI shape: role chunk, three content chunks, terminal
        // finish_reason chunk, usage-only chunk, [DONE]. A ping event
        // (Anthropic-style keepalive) and an OpenAI comment line ride
        // along — both must be skipped, not fatal.
        let mut server = mockito::Server::new();
        let body = concat!(
            ": OPENAI_PROCESSING\n\n",
            "event: ping\n",
            "data: {}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hel\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"lo\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"!\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":9,\"completion_tokens\":3,\"total_tokens\":12}}\n\n",
            "data: [DONE]\n\n",
        );
        let mock = server
            .mock("POST", "/chat/completions")
            .match_body(mockito::Matcher::PartialJson(serde_json::json!({
                "stream": true,
                "stream_options": {"include_usage": true}
            })))
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(body)
            .create();
        let (rx, handle) = spawn_chat_stream_worker(stream_resolved(&server), one_user_message());
        let (text, error) = collect_stream(&rx);
        assert_eq!(text, "Hello!");
        assert!(error.is_none(), "unexpected error: {error:?}");
        mock.assert(); // request carried stream:true + include_usage
        join_with_timeout(handle, std::time::Duration::from_secs(10));
    }

    #[test]
    fn stream_worker_treats_clean_close_without_done_as_done() {
        // Servers exist that end the body without the [DONE]
        // sentinel (async-openai issue #456): a clean stream close is
        // a valid end, not an error.
        let mut server = mockito::Server::new();
        let body = concat!(
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ok\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"?\"},\"finish_reason\":null}]}\n\n",
        );
        server
            .mock("POST", "/chat/completions")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(body)
            .create();
        let (rx, handle) = spawn_chat_stream_worker(stream_resolved(&server), one_user_message());
        let (text, error) = collect_stream(&rx);
        assert_eq!(text, "ok?");
        assert!(error.is_none(), "unexpected error: {error:?}");
        join_with_timeout(handle, std::time::Duration::from_secs(10));
    }

    #[test]
    fn stream_worker_classifies_non_2xx_error_bodies() {
        // The provider's error message surfaces as a classified
        // AiError, never a bare transport string.
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/chat/completions")
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": {"message": "Incorrect API key provided"}}"#)
            .create();
        let (rx, handle) = spawn_chat_stream_worker(stream_resolved(&server), one_user_message());
        let (_, error) = collect_stream(&rx);
        match error {
            Some(err @ AiError::Auth(_)) => {
                assert!(err.to_string().contains("Incorrect API key"));
            }
            other => panic!("expected Auth, got {other:?}"),
        }
        join_with_timeout(handle, std::time::Duration::from_secs(10));
    }

    #[test]
    fn stream_worker_cancels_when_the_receiver_drops() {
        // Stop = drop-to-cancel (reqwest has no cancel API): a raw
        // SSE server keeps the stream open; dropping the receiver
        // makes the worker's next send fail, the worker drops the
        // response, and the OS tears the connection down — the
        // server's next write errors. Nothing panics.
        use std::io::{Read, Write};
        use std::net::{TcpListener, TcpStream};
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use std::time::{Duration, Instant};

        /// Consume the request head + body before responding — hyper
        /// fails with "received unexpected message from connection"
        /// if the answer races the request bytes.
        fn drain_request(sock: &mut TcpStream) {
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            let headers_end = loop {
                let n = sock.read(&mut chunk).expect("read request");
                buf.extend_from_slice(&chunk[..n]);
                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    break pos + 4;
                }
            };
            let headers = String::from_utf8_lossy(&buf[..headers_end]).to_ascii_lowercase();
            let len: usize = headers
                .lines()
                .find_map(|l| l.strip_prefix("content-length: ")?.trim().parse().ok())
                .unwrap_or(0);
            while buf.len() < headers_end + len {
                let n = sock.read(&mut chunk).expect("read body");
                buf.extend_from_slice(&chunk[..n]);
            }
        }

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let torn_down = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&torn_down);

        let server = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().expect("accept");
            drain_request(&mut sock);
            sock.write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
            )
            .expect("write headers");
            let chunk = b"data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\".\"},\"finish_reason\":null}]}\n\n";
            // ~12 s of live stream; the test drops the receiver long
            // before this loop runs out.
            for _ in 0..1_200 {
                if sock.write_all(chunk).and_then(|()| sock.flush()).is_err() {
                    flag.store(true, Ordering::SeqCst);
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        });

        let resolved = ResolvedAi {
            base_url: format!("http://{addr}"),
            model: "m".into(),
            api_key: String::new(),
        };
        let (rx, handle) = spawn_chat_stream_worker(resolved, one_user_message());
        match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(StreamMsg::Delta(_)) => {}
            other => panic!("expected a first delta, got {other:?}"),
        }
        drop(rx); // Stop
        let deadline = Instant::now() + Duration::from_secs(10);
        while !torn_down.load(Ordering::SeqCst) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            torn_down.load(Ordering::SeqCst),
            "the connection was not torn down after the receiver dropped"
        );
        join_with_timeout(handle, Duration::from_secs(10));
        server.join().expect("server thread exits");
    }
}
