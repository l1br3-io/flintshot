# ADR-0069: AI provider client — one OpenAI-compatible wire format, trait in core, reqwest in app

**Context.** The Ask-the-shot effort (spec
`docs/specs/ai-transforms/`) needs to send the flattened canvas to
a vision-capable chat model and render the markdown answer in the
editor. The provider landscape at decision time: OpenAI and xAI
speak OpenAI chat-completions natively; Gemini exposes an
OpenAI-compatible endpoint; local servers (Ollama, LM Studio,
OpenRouter) speak it too. The original spec draft assumed no HTTP
client existed and proposed `ureq`; planning found `reqwest` 0.12
(rustls) already in the tree for the Imgur backend (ADR-0026), and
`egui_commonmark` 0.24 as the only egui-0.35-compatible markdown
renderer.

**Decision.**

1. **One wire format.** A single chat-completions client covers all
   four provider presets (`openai`, `xai`, `gemini`, `custom`);
   per-provider differences reduce to a base-URL preset table
   (`ProviderPreset`) plus an optional base-URL override. An empty
   API key sends no `Authorization` header — local keyless servers
   work as first-class presets, hosted presets require a key
   (`resolve_ai` enforces the difference).
2. **The upload-backend split, mirrored** (ADR-0004 / ADR-0024):
   wire types, `AiError`, the `AiClient` trait, provider presets,
   and the transform store live in `flintshot-core` (no GUI types —
   `no_gui_types_in_core` stays green); the reqwest implementation,
   keyring helpers, and the request worker live in
   `flintshot-app/src/ai/`. Snake_case serde on the wire structs is
   scoped to `core::ai` with a module-doc note (the repo kebab-case
   convention applies to *our* TOML/JSON only).
3. **reqwest on a dedicated worker thread**, not the UI thread: each
   request spawns a plain `std::thread` whose current-thread tokio
   runtime block-ons the client call; the result travels over
   `std::sync::mpsc` and the egui loop polls it with a
   `request_repaint_after(100ms)` timer while pending — no
   cross-thread `Context` clone, no UI-thread `block_on`. Closing
   the editor mid-request is safe (the receiver drops; the send
   fails into the void).
4. **Keys in the keyring** (the `imgur:client-id` stance): entry
   `flintshot / ai:<provider>`; `config.toml`'s `[ai]` section holds
   only provider/model/base-url. The UI never echoes a stored key —
   a masked "Replace key" affordance only.
5. **Markdown via `egui_commonmark` 0.24** pinned to the egui 0.35
   line, `default-features = false`: `load-images` stays off so
   assistant markdown can never fetch remote content (the
   no-third-party-traffic boundary). The vendored
   minimal-renderer fallback stays documented as a one-branch switch
   at `assistant_markdown`.

**Consequences.**

- Adding a provider is a table entry, not a client; providers that
  drift from chat-completions can still be reached through the
  custom preset's base-URL override until a dedicated backend is
  warranted.
- One extra workspace dep (`egui_commonmark`; `base64` in core) and
  one current-thread runtime per in-flight request — bounded by the
  one-request-at-a-time conversation model.
- The trait-in-core split costs a small indirection layer but keeps
  `flintshot-core` GUI-free and lets the mockito-driven client tests
  live beside the imgur precedent with zero network in CI.
- If `egui_commonmark`'s 0.35-compatible line is ever rejected, the
  fallback renderer replaces one function without touching the
  pipeline.
