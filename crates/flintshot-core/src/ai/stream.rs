//! Pure SSE decoder for streaming chat completions.
//!
//! Framing follows the WHATWG HTML standard's server-sent-events
//! parsing rules (§9.2.5 "Parsing an event stream" — `stream = [bom]
//! *event`, `end-of-line = cr lf / cr / lf`, always UTF-8; §9.2.6
//! "Interpreting an event stream" — blank line dispatches, a leading
//! `:` marks a comment, `field: value` splits on the first colon with
//! one optional leading space stripped from the value, and multiple
//! `data:` lines of one event join with `\n`):
//! <https://html.spec.whatwg.org/multipage/server-sent-events.html#parsing-an-event-stream>
//!
//! The payload protocol is the `OpenAI` `chat-completions` streaming
//! shape (`chat.completion.chunk` envelopes: first chunk
//! `{"delta":{"role":"assistant","content":""}}`, text chunks
//! `{"delta":{"content":"…"}}`, final `{"delta":{},"finish_reason":
//! "stop"}`, stream terminated by the `data: [DONE]` sentinel):
//! <https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create/>
//!
//! One deliberate divergence from §9.2.6: the standard *discards*
//! pending data at end-of-file ("the incomplete event is not
//! dispatched"), but this module's whole point is never losing text
//! the user already saw — [`SseDecoder::finish`] flushes a trailing
//! unterminated event instead. A server that ends its stream right
//! after a final `data:` line (no blank line) still yields that
//! delta.
//!
//! Core decoder stays pure — bytes in, events out, no runtime types.

use serde::{Deserialize, Serialize};

/// The `data:` payload that terminates a chat-completions stream.
const DONE_SENTINEL: &str = "[DONE]";

/// One decoded event from a chat-completions SSE stream.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SseEvent {
    /// Text from `choices[0].delta.content` of one chunk.
    Delta(String),
    /// The `data: [DONE]` sentinel — the provider finished cleanly.
    Done,
    /// A data payload that isn't valid chunk JSON. The decoder
    /// skips it and stays usable for subsequent events.
    Malformed(String),
}

/// A `chat.completion.chunk` envelope (the streaming analogue of
/// [`crate::ai::AiResponse`]).
///
/// Field names serialize `snake_case` per the module-wide
/// OpenAI-wire-format carve-out (see `ai/mod.rs`). Every field is
/// optional on the wire: the role-only first chunk omits `content`,
/// the final chunk carries an empty `delta`, and usage-only chunks
/// (when requested) carry an empty `choices` — the decoder tolerates
/// them all by design, so provider flavor differences never abort a
/// stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiStreamChunk {
    /// Chunk choices; text consumers read `choices[0]`.
    #[serde(default)]
    pub choices: Vec<AiStreamChoice>,
}

impl AiStreamChunk {
    /// The text of `choices[0].delta.content`, if this chunk carries
    /// one (`None` for role-only, tool, and usage chunks).
    #[must_use]
    pub fn first_delta_content(&self) -> Option<&str> {
        self.choices
            .first()
            .and_then(|choice| choice.delta.content.as_deref())
    }
}

/// One choice in a `chat.completion.chunk` envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiStreamChoice {
    /// Position of this choice in `choices` (we request one).
    #[serde(default)]
    pub index: u32,
    /// The streamed delta (role on the first chunk, content after).
    #[serde(default)]
    pub delta: AiStreamDelta,
    /// Why the model stopped, on the final chunk only
    /// (`"stop"`, `"length"`, ...).
    #[serde(default)]
    pub finish_reason: Option<String>,
}

/// The `delta` object of a stream chunk.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AiStreamDelta {
    /// `"assistant"` on the first chunk only.
    #[serde(default)]
    pub role: Option<String>,
    /// The text delta; absent on the role-only first chunk and on
    /// tool chunks, `""` on some providers' first chunk.
    #[serde(default)]
    pub content: Option<String>,
}

/// A pure push decoder for SSE-framed chat-completions streams:
/// [`feed`](Self::feed) bytes as they arrive, collect
/// [`SseEvent`]s, then [`finish`](Self::finish) at stream end.
///
/// No I/O, no tokio, no GUI types — the worker owns the transport.
///
/// # Examples
///
/// ```
/// use flintshot_core::{SseDecoder, SseEvent};
///
/// let mut decoder = SseDecoder::new();
/// let mut events = Vec::new();
/// decoder.feed(b"data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}\n\n", &mut events);
/// decoder.feed(b"data: [DONE]\n\n", &mut events);
/// assert_eq!(events, vec![SseEvent::Delta("Hi".into()), SseEvent::Done]);
/// ```
#[derive(Debug, Clone, Default)]
pub struct SseDecoder {
    /// Bytes of the not-yet-terminated line (events can split
    /// across `feed` calls).
    buf: Vec<u8>,
    /// `data:` field values of the event being accumulated
    /// (dispatched on the next blank line).
    data_lines: Vec<String>,
    /// Whether the start-of-stream BOM decision is settled.
    started: bool,
}

impl SseDecoder {
    /// A fresh decoder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Core decoder stays pure — bytes in, events out, no runtime
    /// types.
    ///
    /// Accepts an arbitrary slice of the stream (TCP-sized chunks or
    /// whole bodies; events may split across calls) and appends any
    /// complete events to `out`.
    pub fn feed(&mut self, bytes: &[u8], out: &mut Vec<SseEvent>) {
        self.buf.extend_from_slice(bytes);
        self.strip_bom_once();
        self.drain_complete_lines(out);
    }

    /// Flush at stream end: any trailing line missing its terminator
    /// is processed, and a trailing event missing its blank line is
    /// still dispatched (see the module docs for the deliberate
    /// divergence from the SSE end-of-file rule).
    pub fn finish(&mut self, out: &mut Vec<SseEvent>) {
        let mut line = std::mem::take(&mut self.buf);
        // A lone trailing CR is a complete terminator at end-of-file
        // (it can no longer be half of a split CRLF pair).
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        self.started = true;
        if !line.is_empty() {
            self.process_line(&String::from_utf8_lossy(&line), out);
        }
        self.dispatch(out);
    }

    /// Strip one leading UTF-8 BOM at stream start (SSE §9.2.6: "The
    /// UTF-8 decode algorithm strips one leading UTF-8 Byte Order
    /// Mark (BOM), if any"). A partial BOM prefix waits for more
    /// bytes rather than being mistaken for payload.
    fn strip_bom_once(&mut self) {
        const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];
        if self.started || self.buf.is_empty() {
            return;
        }
        if self.buf.starts_with(&BOM) {
            self.buf.drain(..BOM.len());
            self.started = true;
        } else if !BOM.starts_with(&self.buf) {
            // Not a BOM prefix — the stream starts with real data.
            self.started = true;
        }
    }

    /// Extract and process every fully-terminated line in `buf`. A
    /// lone trailing CR holds (it may be the first half of a CRLF
    /// pair arriving in the next feed).
    fn drain_complete_lines(&mut self, out: &mut Vec<SseEvent>) {
        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n' || b == b'\r') {
            if self.buf[pos] == b'\r' && pos + 1 == self.buf.len() {
                break;
            }
            let terminator = if self.buf[pos] == b'\r' && self.buf.get(pos + 1) == Some(&b'\n') {
                2
            } else {
                1
            };
            let line: Vec<u8> = self.buf.drain(..pos).collect();
            self.buf.drain(..terminator);
            self.process_line(&String::from_utf8_lossy(&line), out);
        }
    }

    /// Apply the SSE §9.2.6 line rules: blank dispatches, leading
    /// `:` is a comment, otherwise `field: value` (one optional
    /// leading space stripped) with a colon-less line naming a field
    /// whose value is empty. Only `data` accumulates; `event`, `id`,
    /// `retry`, and unknown fields are ignored (the chat protocol
    /// rides `data` only).
    fn process_line(&mut self, line: &str, out: &mut Vec<SseEvent>) {
        if line.is_empty() {
            self.dispatch(out);
        } else if !line.starts_with(':') {
            let (field, value) = match line.split_once(':') {
                Some((field, rest)) => (field, rest.strip_prefix(' ').unwrap_or(rest)),
                None => (line, ""),
            };
            if field == "data" {
                self.data_lines.push(value.to_owned());
            }
        }
    }

    /// Dispatch the accumulated event (SSE §9.2.6: `data` values
    /// join with `\n` — each append adds a trailing LF, removed at
    /// dispatch). An event with no `data` lines dispatches nothing.
    fn dispatch(&mut self, out: &mut Vec<SseEvent>) {
        if self.data_lines.is_empty() {
            return;
        }
        let payload = self.data_lines.join("\n");
        self.data_lines.clear();
        if payload == DONE_SENTINEL {
            out.push(SseEvent::Done);
        } else {
            match serde_json::from_str::<AiStreamChunk>(&payload) {
                Ok(chunk) => {
                    // Role-only chunks (content absent or ""), empty
                    // `delta`, and usage chunks (no choices) carry no
                    // text — no event. A zero-length delta carries no
                    // information, so "delta emitted" stays equal to
                    // "text received" (the fallback logic upstream
                    // depends on that).
                    if let Some(text) = chunk.first_delta_content().filter(|text| !text.is_empty())
                    {
                        out.push(SseEvent::Delta(text.to_owned()));
                    }
                }
                Err(error) => out.push(SseEvent::Malformed(error.to_string())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(bytes: &[u8]) -> Vec<SseEvent> {
        let mut decoder = SseDecoder::new();
        let mut events = Vec::new();
        decoder.feed(bytes, &mut events);
        decoder.finish(&mut events);
        events
    }

    fn delta(content: &str) -> String {
        format!("data: {{\"choices\":[{{\"delta\":{{\"content\":\"{content}\"}}}}]}}\n\n")
    }

    #[test]
    fn happy_path_stream_yields_ordered_deltas_then_done() {
        let mut stream = String::new();
        stream.push_str("data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\n");
        stream.push_str(&delta("Hello"));
        stream.push_str(&delta(" world"));
        stream.push_str("data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n");
        stream.push_str("data: [DONE]\n\n");
        let events = decode(stream.as_bytes());
        assert_eq!(
            events,
            vec![
                SseEvent::Delta("Hello".into()),
                SseEvent::Delta(" world".into()),
                SseEvent::Done,
            ]
        );
    }

    #[test]
    fn done_sentinel_yields_exactly_one_done() {
        let events = decode(b"data: [DONE]\n\n");
        assert_eq!(events, vec![SseEvent::Done]);
    }

    #[test]
    fn role_only_chunk_emits_no_event_and_no_error() {
        // content key entirely absent (Gemini-style)…
        let events = decode(b"data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\n");
        assert_eq!(events, Vec::<SseEvent>::new());
        // …and OpenAI's first-chunk flavor: content present but empty.
        let events = decode(
            b"data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"\"}}]}\n\n",
        );
        assert_eq!(events, Vec::<SseEvent>::new());
    }

    #[test]
    fn comment_lines_are_ignored() {
        let stream = ": keep-alive\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\n: trailing comment\n\n";
        let events = decode(stream.as_bytes());
        assert_eq!(events, vec![SseEvent::Delta("ok".into())]);
    }

    #[test]
    fn crlf_and_lf_line_endings_both_parse() {
        let lf = decode(delta("hi").as_bytes());
        let crlf = decode(delta("hi").replace('\n', "\r\n").as_bytes());
        assert_eq!(lf, vec![SseEvent::Delta("hi".into())]);
        assert_eq!(crlf, vec![SseEvent::Delta("hi".into())]);
    }

    #[test]
    fn cr_only_line_endings_parse_per_the_sse_grammar() {
        let cr = decode(delta("hi").replace('\n', "\r").as_bytes());
        assert_eq!(cr, vec![SseEvent::Delta("hi".into())]);
    }

    #[test]
    fn event_split_across_feeds_decodes_once() {
        let payload = delta("split");
        // Split mid-line, between feeds — exactly one Delta may come out.
        let mut decoder = SseDecoder::new();
        let mut events = Vec::new();
        let bytes = payload.as_bytes();
        let cut = bytes.len() / 2;
        decoder.feed(&bytes[..cut], &mut events);
        assert_eq!(events, Vec::<SseEvent>::new(), "partial line holds");
        decoder.feed(&bytes[cut..], &mut events);
        assert_eq!(events, vec![SseEvent::Delta("split".into())]);
    }

    #[test]
    fn split_crlf_terminator_waited_for_across_feeds() {
        let mut payload = delta("crlf").replace('\n', "\r\n");
        payload.push_str("data: [DONE]\r\n\r\n");
        let bytes = payload.as_bytes();
        // Cut between the CR and its LF: the first feed ends with a
        // lone CR, which the decoder must hold rather than treat as
        // a terminator (it could still be half of CRLF).
        let cut = payload.find("}\r\n").expect("terminator") + 2;
        let mut decoder = SseDecoder::new();
        let mut events = Vec::new();
        decoder.feed(&bytes[..cut], &mut events);
        assert_eq!(
            events,
            Vec::<SseEvent>::new(),
            "the half-CRLF line is held, not dispatched"
        );
        decoder.feed(&bytes[cut..], &mut events);
        decoder.finish(&mut events);
        assert_eq!(events, vec![SseEvent::Delta("crlf".into()), SseEvent::Done]);
    }

    #[test]
    fn multi_data_lines_join_into_one_payload() {
        // A JSON-legal split point: the join separator is JSON
        // whitespace, so the merged payload parses to one Delta.
        let stream = "data: {\"choices\":\ndata: [{\"delta\":{\"content\":\"joined\"}}]}\n\n";
        assert_eq!(
            decode(stream.as_bytes()),
            vec![SseEvent::Delta("joined".into())]
        );

        // A separator IS inserted between the lines (raw
        // concatenation would parse; any real separator inside a
        // JSON string makes it malformed).
        let stream = "data: {\"choices\":[{\"delta\":{\"content\":\"a\ndata: b\"}}]}\n\n";
        assert!(matches!(
            decode(stream.as_bytes()).as_slice(),
            [SseEvent::Malformed(_)]
        ));
    }

    #[test]
    fn malformed_json_is_reported_and_the_decoder_continues() {
        let stream = format!("data: not-json\n\n{}data: [DONE]\n\n", delta("after"));
        let events = decode(stream.as_bytes());
        assert_eq!(events.len(), 3, "malformed does not swallow the rest");
        assert!(matches!(&events[0], SseEvent::Malformed(_)));
        assert_eq!(events[1], SseEvent::Delta("after".into()));
        assert_eq!(events[2], SseEvent::Done);
    }

    #[test]
    fn finish_flushes_a_trailing_event_without_blank_line() {
        // No trailing blank line after the last data line — finish()
        // still yields the delta (documented divergence from the
        // SSE end-of-file discard rule).
        let events = decode(delta("tail").trim_end_matches("\n\n").as_bytes());
        assert_eq!(events, vec![SseEvent::Delta("tail".into())]);
    }

    #[test]
    fn finish_treats_a_trailing_cr_as_a_line_terminator() {
        let payload = format!(
            "data: {}",
            "{\"choices\":[{\"delta\":{\"content\":\"cr\"}}]}"
        );
        let mut decoder = SseDecoder::new();
        let mut events = Vec::new();
        decoder.feed(format!("{payload}\r").as_bytes(), &mut events);
        decoder.finish(&mut events);
        assert_eq!(events, vec![SseEvent::Delta("cr".into())]);
    }

    #[test]
    fn leading_bom_is_stripped_once() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(delta("bom").as_bytes());
        assert_eq!(decode(&bytes), vec![SseEvent::Delta("bom".into())]);
    }

    #[test]
    fn leading_bom_split_across_feeds_is_still_stripped() {
        let mut decoder = SseDecoder::new();
        let mut events = Vec::new();
        decoder.feed(&[0xEF], &mut events);
        decoder.feed(&[0xBB, 0xBF], &mut events);
        decoder.feed(delta("bom").as_bytes(), &mut events);
        decoder.finish(&mut events);
        assert_eq!(events, vec![SseEvent::Delta("bom".into())]);
    }

    #[test]
    fn non_data_fields_are_ignored() {
        let stream =
            "event: message\nid: 42\nretry: 1000\ndata: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\n";
        assert_eq!(
            decode(stream.as_bytes()),
            vec![SseEvent::Delta("ok".into())]
        );
    }

    #[test]
    fn chunk_wire_type_parses_the_documented_openai_shapes() {
        // First chunk, text chunk, final chunk — from the OpenAI
        // chat-completions streaming reference examples.
        let first: AiStreamChunk = serde_json::from_str(
            r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1694268190,"model":"gpt-4o-mini","choices":[{"index":0,"delta":{"role":"assistant","content":""},"logprobs":null,"finish_reason":null}]}"#,
        )
        .expect("first chunk");
        // Raw wire value is Some("") — the decoder skips it; callers
        // filtering empties treat it as "no text".
        assert_eq!(first.first_delta_content(), Some(""));

        let text: AiStreamChunk = serde_json::from_str(
            r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1694268190,"model":"gpt-4o-mini","choices":[{"index":0,"delta":{"content":"Hello"},"logprobs":null,"finish_reason":null}]}"#,
        )
        .expect("text chunk");
        assert_eq!(text.first_delta_content(), Some("Hello"));

        let last: AiStreamChunk = serde_json::from_str(
            r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1694268190,"model":"gpt-4o-mini","choices":[{"index":0,"delta":{},"logprobs":null,"finish_reason":"stop"}]}"#,
        )
        .expect("final chunk");
        assert_eq!(last.first_delta_content(), None);
        assert_eq!(
            last.choices[0].finish_reason.as_deref(),
            Some("stop"),
            "finish_reason parses for callers that want it"
        );

        // Usage-only chunk: choices is an empty array.
        let usage: AiStreamChunk =
            serde_json::from_str(r#"{"choices":[],"usage":{"total_tokens":108}}"#)
                .expect("usage chunk");
        assert_eq!(usage.first_delta_content(), None);
    }
}
