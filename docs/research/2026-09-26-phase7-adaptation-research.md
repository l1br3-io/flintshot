# Phase 7 adaptation research — what to copy, from where

Research notes, 2026-09-26. Primary sources only (official API docs, crate
sources on GitHub/docs.rs/crates.io API, project issue trackers); every repo,
page, and license re-verified today. Scope: Phase 7 (streaming + Stop,
remember-transform, live-provider verification) and the stretch goals (OCR,
searchable history, after-capture actions, MCP tool server), per
[`../ROADMAP.md`](../ROADMAP.md).

Establishes `docs/research/` alongside [`../REFERENCES.md`](../REFERENCES.md):
REFERENCES stays the curated who-to-watch list; this directory holds dated,
sourced deep-dives that back a specific decision.

## 1. SSE wire format (chat-completions streaming)

**OpenAI (the spec we target).** Request sets `stream: true`; the response is
server-sent events. `stream_options: {"include_usage": true}` must only be set
when streaming — [chat/create reference](https://platform.openai.com/docs/api-reference/chat/create).

Chunk shape (schema `CreateChatCompletionStreamResponse`, object always
`chat.completion.chunk`): `choices[]` each with `delta` (`content`, optional
`role`, `refusal`, `tool_calls`), `index`, and `finish_reason` — `null` on
content chunks, `"stop" | "length" | "tool_calls" | "content_filter" | "function_call"`
on the terminal chunk whose `delta` is empty — [streaming-events reference](https://developers.openai.com/api/reference/resources/chat/subresources/completions/streaming-events).

`[DONE]` sentinel and usage accounting, both from the same two references:

- With `include_usage`, an extra chunk is streamed **before** the
  `data: [DONE]` message; its `usage` holds totals and its `choices` is
  **always an empty array**. All other chunks carry `usage: null`.
- "If the stream is interrupted or cancelled, you may not receive the final
  usage chunk" — don't gate UI completion on it.

**Provider divergences** (all are our ADR-0069 presets):

| Provider | Base URL | Streaming | Divergences from OpenAI |
| --- | --- | --- | --- |
| OpenAI | `https://api.openai.com/v1` | Yes | — (baseline) |
| xAI | `https://api.x.ai/v1` | Yes ([streaming guide](https://docs.x.ai/docs/guides/streaming-response)) | `usage` on **every** chunk, cumulative (vs null-until-last); `delta.reasoning_content` for reasoning models; recommends overriding client timeout (~3600 s) for reasoning models; terminates `data: [DONE]` |
| Gemini | `https://generativelanguage.googleapis.com/v1beta/openai/` | Yes ([OpenAI-compat page](https://ai.google.dev/gemini-api/docs/openai)) | Compat layer "still in beta"; their own example uses `stream_options={'include_usage': True}` ([same page, extra_body section](https://ai.google.dev/gemini-api/docs/openai#extra-body)); Gemini-only knobs ride in `extra_body` |
| Ollama | local `http://localhost:11434/v1`, cloud `https://ollama.com/v1` | Yes ([compat page](https://docs.ollama.com/api/openai-compatibility)) | `stream` + `stream_options.include_usage` supported; vision accepts **base64 only, not image URLs** (checkbox unchecked on compat page); `tool_choice`, `n`, `logprobs` unsupported; local server requires an api_key value but ignores it (`api_key='ollama'`) |

**Adopt:** always send `stream_options: {"include_usage": true}`; parse
`usage` only from the last non-null chunk (tolerates xAI's every-chunk usage);
treat both `[DONE]` and a plain stream close as valid end-of-stream; keep
`delta.content` as the render path and stash `reasoning_content` (xAI) as an
optional dimmed section. We already send base64 images, so Ollama's
no-image-URL limit is a non-issue.

## 2. async-openai's chat-stream event loop

Repo [64bit/async-openai](https://github.com/64bit/async-openai), **MIT**
([LICENSE](https://github.com/64bit/async-openai/blob/main/LICENSE)). We stay
on reqwest per ADR-0069; we crib the loop. From
[`src/chat.rs`](https://github.com/64bit/async-openai/blob/main/async-openai/src/chat.rs)
and
[`src/client.rs`](https://github.com/64bit/async-openai/blob/main/async-openai/src/client.rs)
(`post_stream` + `stream`):

| Stage | What it does | Worth copying? |
| --- | --- | --- |
| Entry guard | `create_stream` rejects `stream: false` (`create` rejects `stream: true`) | Yes — a `stream()` method that can't be called non-streaming |
| SSE transport | reqwest POST builder → `.eventsource()` (the [`eventsource-stream`](https://crates.io/crates/eventsource-stream) crate) → `EventSource` | Optional — either adopt that crate or hand-parse `data:` lines; it handles SSE framing edge cases for us |
| Delivery | `tokio::spawn` a task; deliver parsed chunks over an unbounded mpsc; stream is the receiver wrapper | Shape maps 1:1 onto our std::thread + `std::sync::mpsc` worker |
| Error mapping | `EventSourceError::InvalidStatusCode` → re-read the HTTP body and parse the OpenAI error JSON into `ApiError`; other transport errors → `StreamError` | Yes — mid-stream HTTP-status errors must surface the provider's message, not a transport string |
| End handling | `data == "[DONE]"` → break; `event == "keepalive"` → skip; `EventSourceError::StreamEnded` → **graceful** break (fixed via [issue #456](https://github.com/64bit/async-openai/issues/456)); then `event_source.close()` | Yes — all three; servers exist that close without `[DONE]` |
| Cancellation | if `tx.send` fails (receiver dropped) → break + close | Yes — drop-to-cancel is the whole mechanism (§3) |

The consumer side is two lines: iterate `choices[].delta.content` —
[`examples/chat-stream/src/main.rs`](https://github.com/64bit/async-openai/blob/master/examples/chat-stream/src/main.rs).

## 3. Cancellation with reqwest

All from docs.rs (reqwest 0.13.5 current; our pin is `"0.12"`, per
`Cargo.toml`):

- **Blocking client + async runtime don't mix**: "the functionality in
  reqwest::blocking must not be executed within an async runtime, or it will
  panic when attempting to block" — [reqwest::blocking module docs](https://docs.rs/reqwest/latest/reqwest/blocking/).
  Our ADR-0069 shape (plain `std::thread` per request) is the sanctioned
  workaround; for a streaming worker the same applies — own thread, own
  (current-thread) runtime, or the blocking client on a non-async thread.
- **Drop = abort.** There is no explicit cancel API; the async-openai loop
  (§2) cancels purely by the receiver dropping, which makes `send` fail and
  the task `close()` the event source. In our worker-thread model the
  equivalent is: UI drops/ignores the receiver, worker's next `send` errors,
  worker drops the `Response`/stream and the connection is torn down. Closing
  the editor mid-stream therefore already works today for whole-request
  responses; the Stop button only needs to drop the receiver early.
- **Per-read vs total timeout.** `ClientBuilder::timeout` is a *total*
  deadline "from when the request starts connecting until the response body
  has finished" — wrong for a 30 s vision stream.
  `ClientBuilder::read_timeout` "applies to each read operation, and resets
  after a successful read. This is more appropriate for detecting stalled
  connections when the size isn't known beforehand" —
  [ClientBuilder docs](https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html#method.read_timeout).
  It exists on our line: added in **0.12.4** ("applies the duration for each
  read operation. The timeout resets after a successful read") —
  [reqwest CHANGELOG](https://github.com/seanmonstar/reqwest/blob/master/CHANGELOG.md).
  **Adopt:** `read_timeout(30–60s)` on the AI client, no total `timeout`
  while streaming; optionally a generous per-request `RequestBuilder::timeout`
  backstop.

## 4. egui repaint during streaming

We're on egui 0.35 (`Cargo.toml`); the method exists there verbatim —
[`Context::request_repaint_after` (0.35.0 docs)](https://docs.rs/egui/0.35.0/egui/struct.Context.html#method.request_repaint_after):

- "Request repaint after at most the specified duration elapses. The backend
  can chose to repaint sooner … only the smallest duration will be considered."
- The doc's stopwatch example is exactly our case: repaint just often enough
  to show fresh content, "not wasting resources on needless repaints".
- Quirk: the duration counts from the **next** frame's idle time — so calling
  it every frame with a fixed value yields a steady cadence.
- `Context::request_repaint` "If called from outside the UI thread, the UI
  thread will wake up and run, provided the egui integration has set that up …
  (this will work on eframe)"
  ([same page](https://docs.rs/egui/0.35.0/egui/struct.Context.html#method.request_repaint))
  — but ADR-0069 deliberately keeps `Context` off worker threads; stick with
  mpsc polling.

**Cadence:** while a stream is pending, drain the mpsc each frame and call
`request_repaint_after(Duration::from_millis(33))` — smooth token typing at
~30 fps without busy-looping (tokens queue in the channel between frames; a
frame renders whatever has arrived). Today's 100 ms pending-poll is fine for
the spinner-only case but will read as choppy typing. Back to no repaint
requests when idle.

## 5. MCP tool server via RMCP

Repo [modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk)
(official, per README). Crate `rmcp` **3.4.1** published 2026-09-23, 29.3 M
downloads, **Apache-2.0** on crates.io (repo LICENSE is mid-transition
MIT → Apache-2.0; both permits our use) —
[crates.io API](https://crates.io/api/v1/crates/rmcp), [LICENSE](https://github.com/modelcontextprotocol/rust-sdk/blob/main/LICENSE).

- **Minimal stdio tools-only server** is a doc-comment away: `#[tool_router(server_handler)]`
  on the impl + `serve(stdio())`, no separate `ServerHandler` needed — the
  README's calculator example is ~20 lines of user code
  ([README](https://github.com/modelcontextprotocol/rust-sdk#tools));
  runnable runner at
  [`examples/servers/src/calculator_stdio.rs`](https://github.com/modelcontextprotocol/rust-sdk/blob/main/examples/servers/src/calculator_stdio.rs).
- Tool results can return images natively: `ContentBlock::image(png_base64, "image/png")`
  (README "Tool result content types") — a `capture` tool returns the PNG plus
  text, no custom encoding scheme.
- **Tokio weight:** the SDK is "an official Rust Model Context Protocol SDK
  implementation with tokio async runtime" (README); basic deps are tokio,
  serde, schemars. The `server` feature pulls `transport-async-rw`
  (tokio io-util + tokio-util codec) — no HTTP stack needed for stdio
  (crates.io feature graph). MSRV 1.88 ≤ our 1.95; edition 2024 compiles on
  1.95. Cost to `flintshot-app`: tokio is already in-tree (transitively via
  reqwest), but RMCP would make it a direct dep of whatever bin hosts it.
- **Hosting:** stdio servers own stdin and block on `service.waiting().await`.
  Our daemon already has an IPC channel and shouldn't squat on stdin — and
  keeping tokio-runtime code out of the daemon keeps its threading story
  unchanged. **Recommendation:** a separate `flintshot mcp` bin (tiny, like
  `flintshot-cosmic` — opt-in or default-member) that serves stdio and
  forwards `capture` requests to the daemon over the existing IPC. Effort M,
  mostly plumbing + tool schema design.

## 6. OCR on Linux (Wayland)

License/maintenance verified via crates.io API + GitHub API today.

| Option | Kind | License | Maintenance | Packaging burden | Notes |
| --- | --- | --- | --- | --- | --- |
| [tesseract-ocr/tesseract](https://github.com/tesseract-ocr/tesseract) (C++) | Engine (C API + CLI) | Apache-2.0 | pushed 2026-09-11, 76.7k★ | System dep (`tesseract-ocr` + `tessdata`); no Rust packaging | The engine everyone wraps; NormCap shells out to it |
| [`rusty-tesseract` 1.1.10](https://crates.io/crates/rusty-tesseract) ([repo](https://github.com/thomasgruebl/rusty-tesseract)) | CLI wrapper (temp file/pipe → `tesseract` binary) | MIT | Last crate update 2024-03 — stale but tiny surface | Needs `tesseract` binary on PATH; no link deps | Safest integration: no C linkage, version drift only affects CLI flags |
| [`tesseract` 0.15.2](https://crates.io/crates/tesseract) | C-API bindings | MIT | Updated 2025-04 | Link `libtesseract`/`libleptonica` at build | "Higher-level bindings for Tesseract OCR" |
| [`tesseract-rs` 0.4.0](https://crates.io/crates/tesseract-rs) ([repo](https://github.com/cafercangundogdu/tesseract-rs)) | C-API bindings, "optional built-in compilation" | MIT | Updated 2026-07-31 | Can build libtesseract from source — removes system dep, adds build time | Most active binding; watch output maturity |
| [`leptess` 0.14.0](https://crates.io/crates/leptess) | C-API bindings | MIT | Last release 2023-02 | Link deps | Effectively dormant |
| [`paddle-ocr-rs` 0.6.1](https://crates.io/crates/paddle-ocr-rs) ([repo](https://github.com/mg-chao/paddle-ocr-rs)) | ONNX Runtime + PaddleOCR models | Apache-2.0 | Updated 2025-08 | No tesseract; **bundles model files** (~10s of MB) + `ort` runtime | Better on low-DPI/rotated text; heavier artifact; model licensing to audit |
| [`rusto-rs` 0.2.5](https://crates.io/crates/rusto-rs) | Pure-Rust RapidOCR (Paddle models) | (early project) | 0.x | Pure Rust + model files | Too early to lean on; watch |

**NormCap as the UX reference** ([dynobo/normcap](https://github.com/dynobo/normcap),
**GPL-3.0+** per [pyproject.toml](https://github.com/dynobo/normcap/blob/main/pyproject.toml) —
reference only, never copy code): "OCR powered screen-capture tool to capture
information instead of images" — select region → OCR → clipboard (+ a
notification), with post-processing "parse" (URLs, emails, colors, QR).
On Linux it *requires* system tesseract and `wl-clipboard`
([README install section](https://github.com/dynobo/normcap/blob/main/README.md)) —
i.e. even the flagship OCR-snipping tool accepts the system-tesseract dep
rather than bundling.

**Adopt:** an `OcrEngine` trait in `flintshot-core` with the first impl
shelling out to the `tesseract` CLI (rusty-tesseract approach — possibly that
crate, possibly direct `Command` to avoid the stale dep) writing PNG to stdin,
reading text from stdout. Zero new link-time deps, matches NormCap's own
packaging posture, Wayland-agnostic because it runs on our already-captured
region. Revisit `tesseract-rs` built-in compilation if the system dep hurts
packaging; keep PaddleOCR/ONNX as a future second engine behind the trait.

## 7. Demand evidence (issue trackers)

Via GitHub search API, sorted by reactions, checked 2026-09-26.

**flameshot** ([flameshot-org/flameshot](https://github.com/flameshot-org/flameshot)) —
top open issues by reactions: scrolling capture
[#1130](https://github.com/flameshot-org/flameshot/issues/1130) (183👍),
load-existing-image-and-edit [#240](https://github.com/flameshot-org/flameshot/issues/240)
(182👍), GIF [#172](https://github.com/flameshot-org/flameshot/issues/172)
(135👍), window-select [#5](https://github.com/flameshot-org/flameshot/issues/5)
(122👍), cursor-in-capture
[#604](https://github.com/flameshot-org/flameshot/issues/604) (119👍), then two
Wayland capture failures
[#3700](https://github.com/flameshot-org/flameshot/issues/3700) (95👍) and
[#3365](https://github.com/flameshot-org/flameshot/issues/3365) (79👍) — and at
#7, **"OCR to clipboard hook for selections"**
[#702](https://github.com/flameshot-org/flameshot/issues/702) (60👍, 38
comments, open since 2018). Three community OCR PRs
([#1239](https://github.com/flameshot-org/flameshot/pull/1239),
[#3074](https://github.com/flameshot-org/flameshot/pull/3074),
[#4228](https://github.com/flameshot-org/flameshot/pull/4228)) all closed/unmerged —
demand is real, chronic, and unmet. Notably the requests above OCR are
editor/capture features flintshot already ships or has scoped (editor,
Wayland correctness) — validation of direction.

**ShareX** ([ShareX/ShareX](https://github.com/ShareX/ShareX)) — OCR shipped
long ago (Windows.Media.Ocr), so remaining asks are refinements: auto-copy OCR
text [#2848](https://github.com/ShareX/ShareX/issues/2848), auto-translate
after OCR [#5935](https://github.com/ShareX/ShareX/issues/5935), bring back
online OCR [#6350](https://github.com/ShareX/ShareX/issues/6350). The AI ask
exists but was closed unanswered: "No AI version"
[#8348](https://github.com/ShareX/ShareX/issues/8348) (12👍). Lesson: OCR +
after-capture actions are a *pipeline* users keep extending; AI is not
something the ShareX project commits to.

**NormCap** — searching issues for AI/LLM/GPT surfaces **zero feature
requests** (only dependency-bump PRs) — its constituency explicitly wants
local-only OCR. flintshot's Ask-the-shot is differentiation, not parity
chasing.

## 8. Synthesis — ranking for "screenshot tool with AI"

| # | Feature | Value | Effort | Rationale (evidence) |
| --- | --- | --- | --- | --- |
| 1 | **Streaming + Stop** | High | M | Vision round-trips are 5–30 s (ROADMAP Phase 7); every reference chat app treats token-render + cancel as baseline (REFERENCES.md); all four presets support SSE + `include_usage` (§1); async-openai's loop is a crib sheet (§2); cancel is drop + `read_timeout` (§3); repaint cadence solved (§4) |
| 2 | **Remember last transform** | Med-high | S | Repeated 1–2 transforms dominate usage (ROADMAP); one persisted config key + dialog preselect |
| 3 | **MCP `capture` tool** | High (strategic) | M | RMCP makes the server ~20 LOC + plumbing (§5); no Linux-native annotating-screenshot MCP incumbent (REFERENCES.md landscape note); agents get screenshots → flintshot rides the agent wave instead of just chat apps |
| 4 | **OCR region → clipboard** | High | M | Highest-evidenced *unmet* user demand: flameshot #702 (60👍, 7-yr open) + ShareX's OCR-pipeline tail (§7); CLI-wrapper keeps packaging to one optional system dep (§6); unlocks #5/#6 |
| 5 | **After-capture action pipeline** | Med | M-L | ShareX's core retention pattern (§7); needs OCR (#4) plus a config schema + execution order semantics before it's worth building |
| 6 | **OCR-indexed searchable history** | Med-high, long-term | L | screenpipe/Screenotate precedent (REFERENCES.md); the capture store already exists (ADR-0067) but needs an index, background OCR policy, and a search UI; strictly after #4 |

Phase 7 as planned (streaming, remember-transform, live verification) is the
right next slice; the evidence pushes **MCP tool server** to the front of the
post-Phase-7 queue (cheap, differentiating, evidence-backed by the ecosystem's
direction), with **OCR** right behind it on pure user demand.

## What we adapt

| Feature | Source (project/spec) | What exactly we take | Effort |
| --- | --- | --- | --- |
| SSE streaming | [OpenAI chat-completions streaming spec](https://developers.openai.com/api/reference/resources/chat/subresources/completions/streaming-events) | `stream:true` + `stream_options.include_usage`; chunk = `chat.completion.chunk` w/ `choices[].delta.content`, `finish_reason`; `data: [DONE]` terminator; usage-only final chunk with empty `choices` | (part of row below) |
| Streaming client loop | [async-openai](https://github.com/64bit/async-openai) `client.rs` (MIT) | `[DONE]`/keepalive/StreamEnded-quiet-close handling; mid-stream HTTP-error → parse provider JSON error; rx-drop ⇒ close; stream/non-stream entry guards. Keep reqwest + std mpsc worker | M |
| Provider matrix | [xAI](https://docs.x.ai/docs/guides/streaming-response) · [Gemini](https://ai.google.dev/gemini-api/docs/openai) · [Ollama](https://docs.ollama.com/api/openai-compatibility) docs | Base-URL presets already in ADR-0069; add: tolerate every-chunk usage (xAI), `reasoning_content` delta (xAI), base64-only vision (Ollama), beta caveats (Gemini) into the verification pass | S |
| Cancellation | [reqwest docs.rs](https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html#method.read_timeout) + [CHANGELOG](https://github.com/seanmonstar/reqwest/blob/master/CHANGELOG.md) | `read_timeout` (≥0.12.4) instead of total `timeout` for streams; Stop = drop receiver; worker on own thread (blocking/async split rule) | S |
| Repaint cadence | [egui 0.35 Context docs](https://docs.rs/egui/0.35.0/egui/struct.Context.html#method.request_repaint_after) | Drain mpsc per frame + `request_repaint_after(33ms)` while tokens pending; no repaint when idle; stopwatch idiom | S |
| MCP server | [RMCP](https://github.com/modelcontextprotocol/rust-sdk) (Apache-2.0) | `#[tool_router(server_handler)]` + `serve(stdio())` shape; `ContentBlock::image(base64,"image/png")` result; separate `flintshot mcp` bin proxying daemon IPC | M |
| OCR | [tesseract](https://github.com/tesseract-ocr/tesseract) (Apache-2.0) via CLI; UX per [NormCap](https://github.com/dynobo/normcap) (GPL-3.0+, reference only) | `OcrEngine` trait in core; first impl = `tesseract` CLI subprocess (stdin PNG → stdout text); region-select → text → clipboard flow; watch [tesseract-rs](https://crates.io/crates/tesseract-rs) for bundling | M |
| After-capture actions | [ShareX](https://github.com/ShareX/ShareX) pattern (GPL-3.0, UX reference) | Ordered action list per capture (copy/save/upload/AI/OCR) — design only after OCR lands | M-L |
