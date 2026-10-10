# AI capability features — hardening ask-the-shot, then what

Research notes, 2026-10-07. Primary sources only for provider/API claims:
official platform docs, live API endpoints, one live local experiment, and
the GitHub API for issue reaction counts (re-pulled today). Scope: the AI
surface only — candidate features for Ask-the-shot v2+ and the full design
for non-visual-model validation. Builds on
[`2026-10-05-feature-gap-analysis.md`](2026-10-05-feature-gap-analysis.md)
(whose matrix shows no competitor ships AI transforms — re-verified today,
§2) and [`../ROADMAP.md`](../ROADMAP.md). Already-queued work (MCP capture
tool, OCR region → clipboard, after-capture pipeline, searchable history)
is built on, never re-proposed; the v1-spec deferrals (image references in
follow-ups, multiple providers, prompt templating, AI image edits, fuller
chatbox) are fair game and re-evaluated with evidence. Doc-vs-code note:
ROADMAP still lists streaming as Phase 7 "next", but the streaming worker
(`spawn_chat_worker`, `ChatWorkerEvent::Delta/Done/Failed`, `StopHandle` in
`crates/flintshot-app/src/ai/worker.rs`) is merged on main — everything here
ranks against the code, not the roadmap prose. Unverifiable claims are
flagged "unverified"; that is a finding, not a gap in rigor.

## 1. TL;DR — ranked candidates

Ranked by (fit with the shipped AI surface × hardening value) ÷ effort.
Upstream 👍 evidence for *AI transforms specifically* is near-zero
(flameshot has no AI issue at all; ShareX's is a closed "No AI version"
[#8348](https://github.com/ShareX/ShareX/issues/8348), 11👍) — ask-the-shot
is differentiation we invented
([`2026-09-26-ask-the-shot-ux-research.md`](2026-09-26-ask-the-shot-ux-research.md)
§2), so ranking leans on failure-mode evidence and capability verification
rather than demand counts.

| # | Feature | Evidence (primary, verified today unless noted) | Effort | Verdict |
| --- | --- | --- | --- | --- |
| 1 | **Vision-model validation: auto probe + typed error** (§3) | Live: Ollama 0.20.4 silently ignores images sent to a text-only model (HTTP 200, model answers as if no image — §3.3); the shipped `vision_probe_messages()` probe *passes* on such models; OpenAI returns 400 "Invalid content type. image_url is only supported by certain models." ([community thread](https://community.openai.com/t/gpt-4o-search-preview-does-not-support-attachments/1143776)) | S–M | Do first — it hardens the shipped feature against its worst UX cliff |
| 2 | **Structured extraction** (`response_format` json_schema/json_object per transform) | Supported on all six provider surfaces: [OpenAI](https://developers.openai.com/api/docs/guides/structured-outputs) (chat completions incl.), [xAI](https://docs.x.ai/developers/model-capabilities/text/structured-outputs), [Gemini-compat](https://ai.google.dev/gemini-api/docs/openai), [Ollama](https://docs.ollama.com/capabilities/structured-outputs) (native `format` takes a full schema), [LM Studio](https://lmstudio.ai/docs/app/api/structured-output), [OpenRouter](https://openrouter.ai/docs/api-reference/chat-completion) (verified live: `supported_parameters` includes `response_format`, `structured_outputs`) | S–M | Do — "screenshot → table/CSV/code" is the extract transform's actual end state |
| 3 | **Token/cost surfacing** (parse `usage`) | `usage` returned by all: OpenAI ([schema](https://developers.openai.com/api/docs/api-reference/chat/create), **no** cost field), Ollama (live), [OpenRouter `usage.cost` + `cost_details`](https://openrouter.ai/docs/api-reference/chat-completion), xAI (token prices in `/v1/models`) | S | Do alongside #2; one struct, one footer line |
| 4 | **Model picker from `{base}/models` + vision badges where metadata exists** | `GET /v1/models` works on all six (live: Ollama, OpenRouter; docs: [OpenAI](https://developers.openai.com/api/docs/api-reference/models/list), [xAI](https://docs.x.ai/developers/rest-api-reference/inference/models), [Gemini-compat](https://ai.google.dev/gemini-api/docs/openai), [LM Studio](https://lmstudio.ai/docs/app/api/endpoints/openai), [Ollama](https://docs.ollama.com/openai)); modality metadata only on OpenRouter (live: `architecture.input_modalities`), [xAI `/v1/language-models`](https://docs.x.ai/developers/rest-api-reference/inference/models), [Ollama `/api/show`](https://docs.ollama.com/api-reference/show-model-details), [LM Studio native `/api/v1/models`](https://lmstudio.ai/docs/developer/rest/list) | M | Do after #1; badges best-effort, probe verdict authoritative |
| 5 | **MCP `ask` tool beside the queued capture tool** | Precedent: [peekaboo](https://github.com/openclaw/Peekaboo) (5,255★) ships an `analyze` MCP tool (image files/inline data → configured AI provider, BYO key) ([docs/MCP.md](https://github.com/openclaw/Peekaboo/blob/main/docs/MCP.md)) | S delta on queued MCP | Fold into the queued MCP-capture effort's spec as a second tool |
| 6 | **Local-vision quickstart** (Ollama/LM Studio preset guidance) | Local vision floor today ≈9B: `clef-flash` 9B, `ornith-1.5:9b`, then 27B+ ([ollama.com vision catalog](https://ollama.com/search?cid=vision)); LM Studio exposes `capabilities.vision` per model ([docs](https://lmstudio.ai/docs/developer/rest/list)); privacy constituency proven by [NormCap](https://github.com/dynobo/normcap) (2,743★, active) and [screenpipe](https://github.com/screenpipe/screenpipe) (21,839★, active) | S | Documentation + preset hygiene + auto-badge via #4; never bundle (VISION non-goal) |
| 7 | **Multi-image: compare two shots; re-attach after annotating** | Gemini docs literally use "What is different between these two images?" as their multi-image example ([vision guide](https://ai.google.dev/gemini-api/docs/vision), 3,600 imgs max); OpenAI: multiple images, up to 1,500/request ([guide](https://developers.openai.com/api/docs/guides/images-vision)); xAI: no image-count limit ([docs](https://docs.x.ai/developers/model-capabilities/images/understanding)); Ollama: two same-size images can silently collapse into one ([#17321](https://github.com/ollama/ollama/issues/17321), [#17814](https://github.com/ollama/ollama/issues/17814)) | M | Next v2 feature candidate; subsumes the deferred "image references in follow-ups" |
| 8 | **Region-scoped ask** (send only the selection) | Intrinsic: Gemini bills 258 tokens per 768×768 tile ([docs](https://ai.google.dev/gemini-api/docs/vision)) — a region vs a 2048-px canvas is ~10× fewer image tokens; also focuses the model | S | Rider whenever the request builder is next touched |
| 9 | **Prompt templating + OCR-text-in-prompt synergy** | Raycast `{selection}`/`{argument}` precedent ([manual](https://manual.raycast.com/ai/ai-commands), via 09-26 doc); the accuracy claim "text+image beats image-only" is **unverified** — Gemini's prompting guide only says instructions should clearly reference each modality ([docs](https://ai.google.dev/gemini-api/docs/prompting-strategies)) | M | Defer until queued OCR lands, then design the combined transform with evidence |
| 10 | **Conversation persistence across editor sessions** | Market counter-evidence: our NormCap-adjacent constituency rewards the opposite (09-26 §6 already ruled: opt-in only, paired with searchable history) | M | Keep deferred; unchanged stance |

## 2. Provider capability matrix

Verified 2026-10-07 from docs and live endpoints. "Gemini-compat" = the
OpenAI-compatible endpoint flintshot's `gemini` preset targets.

| Capability | OpenAI | xAI | Gemini-compat | Ollama | LM Studio | OpenRouter |
| --- | --- | --- | --- | --- | --- | --- |
| `GET {base}/models` (OpenAI-compat shape) | ✅ | ✅ | ✅ (`…/v1beta/openai/models`) | ✅ (live) | ✅ (`/v1/models`) | ✅ (`/api/v1/models`, keyless — live) |
| Modality/vision metadata in that response | ❌ ([Model object](https://developers.openai.com/api/docs/api-reference/models/list): `id, created, object, owned_by, shutdown_date` only) | ❌ — but xAI-only [`GET /v1/language-models`](https://docs.x.ai/developers/rest-api-reference/inference/models) returns `input_modalities` | ❓ unverified beyond `model.id` in the doc example | ❌ (`id, object, created, owned_by` — live) | ❌ on `/v1/models` (unverified); ✅ native [`/api/v1/models`](https://lmstudio.ai/docs/developer/rest/list): `capabilities.vision` | ✅ `architecture.input_modalities: ["text","image"]` (live) |
| Per-model vision flag elsewhere | — | `/v1/language-models` | — | ✅ [`/api/show`](https://docs.ollama.com/api-reference/show-model-details) `capabilities: [completion, thinking, vision]` (live: `llama3:8b` → `["completion"]`) | ✅ native endpoint | same response |
| Behavior: image part → text-only model | 400, "Invalid content type. image_url is only supported by certain models." ([community](https://community.openai.com/t/gpt-4o-search-preview-does-not-support-attachments/1143776); not in official docs) | ❓ unverified | ❓ unverified | **200, image silently ignored** (live, §3.3) | ❓ unverified | varies by upstream model |
| `response_format` on chat completions | ✅ json_schema + json_object ([guide](https://developers.openai.com/api/docs/guides/structured-outputs)) | ✅ json_schema + json_object ([docs](https://docs.x.ai/developers/model-capabilities/text/structured-outputs)) | ✅ via `chat.completions.parse` ([docs](https://ai.google.dev/gemini-api/docs/openai)) | ✅ via `response_format`; native `format` takes a full JSON schema ([docs](https://docs.ollama.com/capabilities/structured-outputs)); Ollama Cloud: ❌ | ✅ json_schema, "same format as OpenAI's Structured Output API"; <7B models may fail ([docs](https://lmstudio.ai/docs/app/api/structured-output)) | ✅ json_object/json_schema/grammar/python ([docs](https://openrouter.ai/docs/api-reference/chat-completion), live) |
| Multiple images per request | ✅ ≤1,500 imgs / 512 MB ([guide](https://developers.openai.com/api/docs/guides/images-vision)) | ✅ no count limit, ≤20 MiB/img, jpg/png ([docs](https://docs.x.ai/developers/model-capabilities/images/understanding)) | ✅ ≤3,600 imgs ([docs](https://ai.google.dev/gemini-api/docs/vision)) | ✅ `images[]` array; same-size dedup bug ([#17814](https://github.com/ollama/ollama/issues/17814)) | ✅ images accepted ([endpoints](https://lmstudio.ai/docs/app/api/endpoints/openai)); limit ❓ unverified | ✅ model-dependent |
| `usage` in response | ✅ tokens + `*_details`; **no cost** ([schema](https://developers.openai.com/api/docs/api-reference/chat/create)) | ✅ tokens | ✅ tokens | ✅ (live: `{"prompt_tokens":23,…}`) | ✅ | ✅ tokens + `cost` + `cost_details` ([schema](https://openrouter.ai/docs/api-reference/chat-completion)) |
| Cost computable from API | ❌ | ✅ token prices in `/v1/models` ([reference](https://docs.x.ai/developers/rest-api-reference/inference/models)) | ❓ unverified | n/a (local) | n/a (local) | ✅ returned |

Incumbent re-check (2026-10-07): [CleanShot](https://cleanshot.com) —
still no AI feature beyond on-device OCR/QR ("Smart zooms"/transcripts are
not AI-branded); [Shottr](https://shottr.cc) — OCR + QR only, no LLM
anything. The 10-05 doc's "flintshot is alone in this niche" holds.

## 3. Validating non-visual models (Q1, full design)

### 3.1 The failure mode today

A user configures a text-only model (`gpt-3.5`-class, or `llama3` in
Ollama) and discovers it at transform time: either a raw
`AiError::Provider { status: 400, message }` card with the provider's
unparsed message (`crates/flintshot-core/src/ai/mod.rs` maps 400 generically),
or — on Ollama — **no error at all** (§3.3): the model answers confidently
about an image it never saw. The second case is worse than the first; it is
undetectable from the response.

### 3.2 Model-listing metadata, verified

- **OpenAI**: `/v1/models` returns identity + lifecycle fields only —
  `id`, `created`, `object`, `owned_by`, `shutdown_date`
  ([Model object schema](https://developers.openai.com/api/docs/api-reference/models/list)).
  No modality/capability metadata. Confirmed: the preset surface cannot
  badge OpenAI models.
- **xAI**: same gap on `/v1/models` (adds pricing +
  `capabilities.reasoning_effort`), **but** the xAI-only
  [`GET /v1/language-models`](https://docs.x.ai/developers/rest-api-reference/inference/models)
  returns `input_modalities` (`"text"`, `"image"`) and
  `output_modalities` per model. Reachable with the same key and base
  (`api.x.ai/v1`), just not an OpenAI-compat path — a per-preset special
  case we can afford for exactly one provider.
- **Gemini-compat**: `GET …/v1beta/openai/models` is documented with a
  list-and-print-`model.id` example
  ([OpenAI compatibility page](https://ai.google.dev/gemini-api/docs/openai));
  whether it returns anything beyond the OpenAI shape is unverified. Treat
  as badge-less.
- **Ollama**: `/v1/models` (live) and
  [`/api/tags`](https://docs.ollama.com/api/tags) return no capability
  metadata; [`/api/show`](https://docs.ollama.com/api-reference/show-model-details)
  returns `capabilities` including `vision` — verified live: `llama3:8b` →
  `["completion"]`. One extra POST per model, so: fetch on selection, not
  on list.
- **LM Studio**: OpenAI-compat `/v1/models` is the wrong door; the native
  [`GET /api/v1/models`](https://lmstudio.ai/docs/developer/rest/list)
  (successor of `/api/v0/models`) returns
  `capabilities: { vision: bool, trained_for_tool_use, reasoning }`.
  Same host/port as the compat server in default setups — worth probing
  opportunistically, degraded silently when absent.
- **OpenRouter**: the gold standard — `architecture.input_modalities`
  (e.g. `["text","image"]`), `output_modalities`, pricing, and
  `supported_parameters` (incl. `response_format`, `structured_outputs`)
  on a keyless public endpoint (verified live, 465 models;
  [API reference](https://openrouter.ai/docs/api-reference/chat-completion)).

Net: metadata exists for 4 of 6 surfaces if we count Ollama's per-model
`/api/show` and xAI's side endpoint; OpenAI and Gemini-compat are
structurally badge-less. **No metadata source covers everything, so
metadata can decorate but never validate.**

### 3.3 Error shapes, verified where possible

- **OpenAI** (community-documented; not in the official reference): 400
  with `"Invalid content type. image_url is only supported by certain
  models."` when an `image_url` part reaches a non-vision model
  ([thread](https://community.openai.com/t/gpt-4o-search-preview-does-not-support-attachments/1143776),
  seen on `gpt-4o-search-preview`). Cleanly heuristic-matchable.
- **Ollama** (live experiment, 2026-10-07, Ollama 0.20.4, `llama3:8b`):
  `POST /v1/chat/completions` with the exact `vision_probe_messages()`
  body → **HTTP 200**, `"content":"Ready"`,
  `usage {prompt_tokens: 23, completion_tokens: 2}`. The image part is
  silently dropped; there is no error to map. (Cross-provider anecdote:
  switching a UI to a non-vision model with images in history is the
  classic 400 source — [open-webui discussion](https://github.com/open-webui/open-webui/discussions/10594)
  — but that is a discussion, not provider docs.)
- **xAI, Gemini-compat**: unverified. xAI's docs document image constraints
  but no rejection behavior; nothing authoritative found for either. Do
  not design against guessed messages.

Critical corollary: **the shipped probe is a wire-shape probe, not a
capability probe.** `vision_probe_messages()`
(`crates/flintshot-core/src/ai/mod.rs`) asks "Reply with the single word:
ready" — a text-only model that ignores images answers "ready" and the
test passes (proven live above). The manual "Test connection" therefore
proves the request is *accepted*, never that the model *looks*.

### 3.4 Design options, ranked

**(a) Typed error mapping at transform time — S, necessary, not sufficient.**
Add `AiError::VisionUnsupported { status, message }` to the enum in
`crates/flintshot-core/src/ai/mod.rs`; in `from_status_and_message`,
classify when the (lowercased) message mentions image/vision/modality
phrases near "not supported"/"only supported"/"invalid content type"
(the verified OpenAI message matches). The AI tab error card
(`crates/flintshot-editor-egui/src/ai_panel.rs`) renders it as "Model
`X` rejected the image — it looks text-only. Change it in Configure AI…"
with the deep-link the 09-26 doc already specified for 401s. Honest
limits: heuristics rot as providers reword; Ollama never errors, so (a)
alone fixes the lesser failure mode only.

**(b) Automatic background probe with a content-checking upgrade — S–M,
the universal validator.** Keep the manual button; additionally fire the
probe when the user **saves** provider/model/base-url in `ai_config.rs`
(and only then — ADR-0069's "every network call user-initiated" boundary
stays intact: save *is* the user action; do not probe on AI-tab open).
Verdict caches in-memory keyed by `(base_url, model)`; the config page
shows an inline banner, the AI tab a small glyph. The upgrade that makes
it meaningful: probe **v1** asks about the image's content — a small
solid-color PNG plus "What is the dominant color of this image? Reply
with only the color word."; pass iff the answer names the color. A
silent-ignore model cannot pass (it guesses or pleads blindness); a
rejection (case a) fails immediately. Cost is one tiny round-trip
(measured 25 tokens total against a local model; the PNG is
`PROBE_IMAGE_URL`-class). False negatives are possible but benign — the
banner says "may not accept image input", never blocks Run, and
re-probes on the next save. Reuses `vision_probe_messages()`'s shape and
the `spawn_chat_worker` channel pattern (`crates/flintshot-app/src/ai/worker.rs`).

**(c) Model picker + best-effort vision badges — M, follow-on.** A
`list_models()` on `OpenAiCompatClient` (`crates/flintshot-app/src/ai/mod.rs`,
same auth/base-url handling) populating a dropdown in `ai_config.rs`
kills the typo'd-model-id class of transform-time errors everywhere (the
endpoint exists on all six surfaces, §2). Vision badges attach where
metadata exists (OpenRouter `input_modalities`; xAI
`/v1/language-models` when preset = `xai`; Ollama `/api/show` on
selection; LM Studio native endpoint opportunistically); elsewhere the
(b) probe verdict is the badge. Also fold in preset hygiene: the xAI
`model_hint` (`grok-2-vision-1212`) predates the current xAI docs, whose
vision examples run `grok-4`/`grok-4.7`
([docs](https://docs.x.ai/developers/model-capabilities/images/understanding)) —
stale hints are cheap to refresh and mislead exactly the users (b) protects.

**Recommendation: (a) + (b) as one small effort now, (c) next.** The
probe is the only universal validator across OpenAI-compatible endpoints
— metadata helps only where it exists (§3.2), and errors help only where
providers emit them (§3.3). Design (b) so its verdict feeds (c)'s badge
and the effort composes instead of repeating.

## 4. Remaining candidates in detail

### 4.1 Structured extraction — S–M

**What.** Per-transform `response_format` (json_object/json_schema +
schema), so "Extract text" can become "screenshot → typed table → CSV
row/code block", with a parse-and-retry loop on schema failure.

**Evidence.** All six surfaces verified (§2). Caveats worth carrying into
the spec: LM Studio warns models <7B "may not be capable" of structured
output ([docs](https://lmstudio.ai/docs/app/api/structured-output));
Ollama Cloud lacks it and free-text string fields under constraint can
loop ([#15502](https://github.com/ollama/ollama/issues/15502));
OpenAI's JSON mode wants the literal word "json" in the prompt
([guide's JSON-mode section](https://developers.openai.com/api/docs/guides/structured-outputs)).

**Mapping.** `AiRequest` (`crates/flintshot-core/src/ai/mod.rs`) grows
`response_format: Option<serde_json::Value>` with
`skip_serializing_if = "Option::is_none"` — the exact pattern `stream`
already uses, so the non-structured golden request body stays
byte-identical (its test lives in
`crates/flintshot-app/src/ai/mod.rs`). `AiTransform`
(`crates/flintshot-core/src/ai/transform.rs`) grows an optional
schema/shape field; `ai_dialog.rs` gains a "structured output" toggle
fed by a few built-in shapes (table, list-of-links, key-value).

**Risks.** Streaming + structured output interact (partial JSON renders
badly in markdown; consider disabling per-card markdown for structured
transforms or buffering). Ollama's compat layer exposes JSON mode; the
native `format`-schema path is richer — verify json_schema-through-
compat behavior per model at build time (marked unverified).

### 4.2 Token/cost surfacing — S

**What.** Parse `usage`; show tokens under each answer card; cost only
where the provider returns or prices it.

**Evidence.** §2 row: tokens everywhere (live on Ollama, schemas for the
rest); money only from OpenRouter (`usage.cost`, `cost_details`) and
computable-from-API on xAI (token prices in `/v1/models`); OpenAI
explicitly returns no cost.

**Mapping.** `AiResponse` grows `usage: Option<AiUsage>` (serde-default,
so old fixtures keep passing); `ChatWorkerEvent` carries it — either
widen `Done(String)` to `Done(String, Option<AiUsage>)` or emit a
`Usage` event before `Done` (`crates/flintshot-app/src/ai/worker.rs`).
Streaming caveat: usage arrives with the final SSE chunk only when the
provider supports `stream_options.include_usage` (Ollama documents it
([compat page](https://docs.ollama.com/openai)); the OpenAI streaming
equivalent is unverified here — check at build time). `ai_panel.rs`
renders a one-line footer.

**Risks.** Cost math from baked-in price tables is rejected (§5): prices
churn; ship only provider-returned cost (OpenRouter) and live-fetched
prices (xAI), otherwise tokens only.

### 4.3 Model picker — M

Covered as §3.4(c). Demand is intrinsic (config UX + typo class), not
issue-tracker-driven; effort is one client method + one dropdown +
per-provider badge adapters.

### 4.4 Multi-image: compare two shots; re-attach after annotating — M

**What.** Attach a second image to the first turn ("what changed between
these two?"), and the deferred follow-up shape: re-attach the canvas
after annotating so the model sees v2 next to the thread.

**Evidence.** The compare-two use case is a documented example in
Google's own vision guide (["What is different between these two
images?"](https://ai.google.dev/gemini-api/docs/vision)); caps are
generous everywhere (OpenAI 1,500, Gemini 3,600, xAI unlimited — §2).
Risk evidence: Ollama's qwen vision path can silently merge same-size
images ([#17321](https://github.com/ollama/ollama/issues/17321),
[#17814](https://github.com/ollama/ollama/issues/17814)) — UI copy
should not promise more than the backend guarantees.

**Mapping.** The wire already supports it: `ChatMessage::User { content:
Vec<ContentPart> }` with multiple `ContentPart::ImageUrl`s serializes
cleanly (`crates/flintshot-core/src/ai/mod.rs`). The v1 non-goal was UI,
not wire: `ai_dialog.rs` grows "attach second image" (from the recents
index); `ai_panel.rs`'s follow-up pill grows an explicit "attach current
canvas" action. The 2048-px longest-edge cap applies per image.

**Risks.** Token cost scales linearly with images (Gemini: 258
tokens/768×768 tile); double-shot requests should default to a smaller
cap. Ollama dedup bug above.

### 4.5 MCP `ask` tool beside the queued capture tool — S delta

**What.** When the queued RMCP capture-tool server lands, expose a second
tool: `ask` (prompt + latest capture or path → transform answer) using
the user's configured provider.

**Evidence.** [Peekaboo](https://github.com/openclaw/Peekaboo) (5,255★)
proves the exact shape on macOS: an `analyze` tool accepting
`.png/.jpg/.jpeg/.webp` ≤10 MiB or inline data, dispatched to the user's
configured AI provider ("Agent runs need a configured model provider" —
the same BYO stance as ADR-0069)
([docs/MCP.md](https://github.com/openclaw/Peekaboo/blob/main/docs/MCP.md)).
No Linux-native incumbent exists (REFERENCES.md landscape note, still
true).

**Mapping.** Pure addition to the queued MCP effort: the server process
reads the same `config.toml` `[ai]` + keyring entries; the transform
prompts can come from the same `TransformStore` (read-only). No new
wire code — `OpenAiCompatClient` is already `Send + Sync`.

**Risks.** Daemon-side requests outlive the editor; the 09-26 doc's
"background notifications" trigger applies. Keep the tool read-only
(capture + ask), never canvas-mutating.

### 4.6 Local-vision quickstart — S

**What.** First-class guidance for the privacy story: a documented
Ollama/LM Studio path (custom preset → `http://localhost:11434/v1` +
a current vision model), vision badges via §3.4(c), and refreshed
`model_hint`s.

**Evidence.** Ollama's current local vision catalog bottoms out around
9B (`clef-flash` 9B, `ornith-1.5:9b`; then 27B+: `qwen3.8`, `clef`,
`qwen3.6`, `nemotron3` 33B; docs' example vision model is `gemma4`) —
the 2024-era small standbys (`llava`, `minicpm-v`, `moondream`) no
longer headline it ([catalog](https://ollama.com/search?cid=vision),
[vision docs](https://docs.ollama.com/capabilities/vision)). LM Studio
exposes `capabilities.vision` per model. Demand-side: NormCap
(2,743★, "OCR powered… capture information instead of images") and
screenpipe (21,839★) prove the local-only constituency is large and
active.

**Mapping.** Docs (README/REFERENCES) + preset hint table + the §3 probe
(skipped cost: local probe is free and instant). One verification note:
Ollama's compat layer accepts base64 inline image content but "not
image URL" ([compat page](https://docs.ollama.com/openai)) — our data
URLs worked live, but keep the probe as the regression check.

**Risks.** Vision quality at 9B is extract/summarize-grade, not
fine-OCR-grade; set expectations in the docs, not the marketing.

### 4.7 Region-scoped ask — S

**What.** When the editor has an active selection, offer "ask about the
selection only" — crop, cap, send one image.

**Evidence.** Cost arithmetic from primary docs: Gemini tiles at 258
tokens per 768×768 tile, a ≤384px image is 258 total
([docs](https://ai.google.dev/gemini-api/docs/vision)); OpenAI's
`detail: "original"` is called out as best for OCR/fine detail but
isn't on all models ([guide](https://developers.openai.com/api/docs/guides/images-vision)).
A region vs a 2048-px canvas is roughly an order of magnitude fewer
image tokens, and focus beats context for explain-this-error.

**Mapping.** The crop already exists (export/blur paths crop the
selection); `ai_dialog.rs` gains a toggle when a selection is live; the
request builder swaps the flattened canvas PNG for the region PNG.
Longer-term: a per-transform `detail` field on `AiRequest`.

**Risks.** None material; purely additive.

### 4.8 Prompt templating + OCR synergy — M, deferred with a trigger

Raycast's `{selection}`/`{argument}` precedent stands (09-26 §3). The
planned synergy — once queued OCR lands, transforms that receive BOTH
the image and its OCR text — rests on an accuracy claim ("text+image
beats image-only for extraction") that is **unverified** from primary
sources: Google's prompting guidance only says instructions should
clearly reference each modality
([docs](https://ai.google.dev/gemini-api/docs/prompting-strategies)),
and no provider doc quantifies a text+image accuracy gain. The honest
case today is design-space, not evidence: OCR text in the prompt is a
transcription scaffold the model can check itself against. Trigger:
the queued OCR effort's spec adds a "transform with OCR text" variant
and we measure on our own captures before generalizing.

### 4.9 Conversation persistence — keep deferred

Unchanged from 09-26 §6: in-memory by spec; the constituency rewards
the opposite (NormCap's whole posture); only worth revisiting as
explicit opt-in paired with the searchable-history stretch goal.

## 5. Explicitly rejected

| Idea | Why rejected |
| --- | --- |
| **AI image generation/edits** (deferred v1 non-goal, re-raised) | Stays deferred: round-trips screen content to third-party generation endpoints (cost + the ADR-0069 no-extra-traffic boundary), and it needs a canvas-mutation model flintshot deliberately lacks (v1 posture: "answers as cards, canvas never touched"). When reconsidered: Apple/Google's modal-editor + Done-commit + revert pattern ([Clean Up](https://support.apple.com/guide/photos/remove-distractions-and-imperfections-pht5c38b77c5/mac), [Magic Eraser](https://support.google.com/photos/answer/6128850)) is the only sanctioned shape. |
| **Cost computation from baked-in price tables** | Prices churn per model per provider; stale cost numbers are worse than none. Ship tokens always; money only where the provider returns it (OpenRouter `usage.cost`) or serves it live (xAI `/v1/models` token prices). |
| **Auto-probe on AI-tab open / on app start** | Violates ADR-0069's "every network call user-initiated". Probe fires on Save and on the manual Test connection; that is both sufficient and compliant. |
| **Agent tool-loops / MCP *client* inside flintshot** | Re-rejected (09-26 §6): an annotating screenshot tool has nothing for an agent to drive; our asymmetry is the server direction (§4.5). |
| **Multiple concurrent providers** (deferred v1 non-goal) | Still no evidence of need; keyring schema + config + UI cost is real. Re-evaluation trigger: per-transform model override (a §4.1 follow-on) demanding a second provider. |
| **Fuller chatbox / web-chat surface** | Deferred v1 non-goal; egui + markdown cards is the ceiling (09-26 §6). |
| **Bundling a local vision model (or any OCR/vision engine)** | VISION non-goal ("no bundled OCR engine", single-binary ethos); §4.6 is presets + docs + probes, never artifacts. |
| **Chasing incumbent "AI" features** | Re-verified today: CleanShot and Shottr have nothing AI-shaped to chase (§2). The niche remains ours; the moat is doing it BYO-key and local-capable. |

## 6. Sources

Issue reaction counts pulled 2026-10-07 via the GitHub API
(`sort=reactions-+1-desc`). Live checks the same day: OpenRouter
`/api/v1/models` (keyless), Ollama 0.20.4 local (`/v1/models`, `/api/show`,
and the §3.3 chat experiment against `llama3:8b`).

- **OpenAI**: [Model object / list models](https://developers.openai.com/api/docs/api-reference/models/list) · [Vision guide](https://developers.openai.com/api/docs/guides/images-vision) (multi-image, 1,500 imgs, `detail` incl. `original`) · [Structured outputs](https://developers.openai.com/api/docs/guides/structured-outputs) (chat completions incl.; JSON mode) · [Chat create / usage schema](https://developers.openai.com/api/docs/api-reference/chat/create) · community: [image_url rejected by non-vision model](https://community.openai.com/t/gpt-4o-search-preview-does-not-support-attachments/1143776), [image_url type errors](https://community.openai.com/t/image-url-for-gpt-4o-api-giving-error-expected-an-object-but-got-a-string-instead/748188)
- **xAI**: [REST models reference](https://docs.x.ai/developers/rest-api-reference/inference/models) (`/v1/models` fields; `/v1/language-models` `input_modalities`) · [Image understanding](https://docs.x.ai/developers/model-capabilities/images/understanding) (20 MiB, no count limit, jpg/png) · [Structured outputs](https://docs.x.ai/developers/model-capabilities/text/structured-outputs) (response_format json_schema/json_object) · [Legacy chat completions](https://docs.x.ai/developers/model-capabilities/legacy/chat-completions) (marked legacy; Responses API gets new features first — wire-drift risk for ADR-0069)
- **Gemini**: [OpenAI compatibility](https://ai.google.dev/gemini-api/docs/openai) (models listing, response_format, image input) · [Vision](https://ai.google.dev/gemini-api/docs/vision) (3,600 imgs, 258 tokens/tile, the compare-two-images example) · [Prompting strategies](https://ai.google.dev/gemini-api/docs/prompting-strategies) (no text+image accuracy claim — basis for §4.8's "unverified")
- **Ollama**: [OpenAI compatibility](https://docs.ollama.com/openai) (endpoint list, `response_format`, `stream_options.include_usage`, "not image URL" note) · [Vision capability](https://docs.ollama.com/capabilities/vision) · [Structured outputs](https://docs.ollama.com/capabilities/structured-outputs) (native `format` schema; Cloud unsupported) · [`/api/tags`](https://docs.ollama.com/api/tags) · [Show model details](https://docs.ollama.com/api-reference/show-model-details) (`capabilities: [completion, thinking, vision]`) · [vision-tagged catalog](https://ollama.com/search?cid=vision) · issues: same-size image collapse [#17321](https://github.com/ollama/ollama/issues/17321), [#17814](https://github.com/ollama/ollama/issues/17814); constrained-JSON loop [#15502](https://github.com/ollama/ollama/issues/15502)
- **LM Studio**: [Native REST list models](https://lmstudio.ai/docs/developer/rest/list) (`capabilities.vision`) · [OpenAI-compatible endpoints](https://lmstudio.ai/docs/app/api/endpoints/openai) · [Structured output](https://lmstudio.ai/docs/app/api/structured-output)
- **OpenRouter**: [Chat completion reference](https://openrouter.ai/docs/api-reference/chat-completion) (usage `cost`/`cost_details`; response_format json_object/json_schema/grammar/python) + live `GET https://openrouter.ai/api/v1/models` (465 models, `architecture.input_modalities`, `supported_parameters`)
- **Peekaboo**: [openclaw/Peekaboo](https://github.com/openclaw/Peekaboo) (5,255★) · [docs/MCP.md](https://github.com/openclaw/Peekaboo/blob/main/docs/MCP.md) (`analyze` tool, 10 MiB, BYO provider)
- **Incumbents** (re-checked 2026-10-07): [cleanshot.com](https://cleanshot.com) · [shottr.cc](https://shottr.cc) — no AI beyond on-device OCR/QR
- **Reference projects**: [screenpipe](https://github.com/screenpipe/screenpipe) (21,839★) · [NormCap](https://github.com/dynobo/normcap) (2,743★) · [open-webui vision-switch discussion](https://github.com/open-webui/open-webui/discussions/10594) (cross-provider context only)
- **Upstream trackers**: flameshot — no AI-titled open issues (searched `ai|gpt|llm|chatgpt|vision` in title); OCR [#702](https://github.com/flameshot-org/flameshot/issues/702) 61👍 (queued). ShareX — [#8348](https://github.com/ShareX/ShareX/issues/8348) "No AI version" (closed, 11👍) · [#8587](https://github.com/ShareX/ShareX/issues/8587) FOSS AI plugins ask (open, 3👍)
- **Internal**: [VISION.md](../VISION.md) (non-goals) · [ROADMAP.md](../ROADMAP.md) (queued work) · [ADR-0069](../adrs/ADR-0069-ai-provider-client.md) (wire client, keyring, no-third-party-traffic) · `crates/flintshot-core/src/ai/mod.rs` (`AiError`, `AiRequest`, `AiResponse`, `vision_probe_messages`, `ProviderPreset`) · `crates/flintshot-app/src/ai/mod.rs` (`OpenAiCompatClient`) · `crates/flintshot-app/src/ai/worker.rs` (`spawn_chat_worker`, `ChatWorkerEvent`, `StopHandle`) · `crates/flintshot-editor-egui/src/ai_config.rs`, `ai_dialog.rs`, `ai_panel.rs` · [2026-10-05-feature-gap-analysis.md](2026-10-05-feature-gap-analysis.md) · [2026-09-26-ask-the-shot-ux-research.md](2026-09-26-ask-the-shot-ux-research.md)
