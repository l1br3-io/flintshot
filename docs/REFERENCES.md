# Reference projects

Open-source projects we compare against and borrow ideas from,
curated by what each is a reference *for*. Tied to the directions in
[`ROADMAP.md`](ROADMAP.md): Phase 7 (Ask-the-shot v2), the OCR and
MCP stretch goals, and the capture/annotate core.

Repos are `owner/name` on GitHub. Locations and licenses verified at
time of writing (2026-09) — re-verify before copying code.

## Capture + AI (closest product comparables)

| Project                        | What it is                          | What we borrow                                  |
| ------------------------------ | ----------------------------------- | ----------------------------------------------- |
| `mediar-ai/screenpipe`         | 24/7 screen + audio capture, AI memory (Rust) | Capture pipeline at scale; vision ingestion UX |
| `dynobo/normcap`                | Screenshot → OCR → clipboard        | Region-select-to-text UX for our OCR stretch goal |
| `steipete/peekaboo` (macOS)      | Screen capture + AI analysis + MCP server | Product analog: shot → AI question; MCP surface |
| `geoffreylitt/screenotate`     | Screenshot + OCR + searchable history | "Every capture is text-searchable" framing     |
| `ShareX/ShareX` (Windows)      | Capture/annotate/upload powerhouse  | Feature-surface benchmark; OCR + after-capture actions |
| `flameshot-org/flameshot`      | Our direct ancestor (C++/Qt)        | The UX bar to clear; known pain points         |
| `ksnip/ksnip`, `KDE/spectacle` | Linux annotate / capture            | Native-Linux conventions                        |

## AI agents (the "goose" family)

| Project                          | What it is                        | What we borrow                          |
| -------------------------------- | --------------------------------- | --------------------------------------- |
| `aaif-goose/goose`               | Rust AI agent: desktop + CLI + API, 15+ providers, MCP (Apache-2.0) | Multi-provider client layer in Rust; desktop-agent UX; our future MCP client-side story |
| `All-Hands-AI/OpenHands`         | Software-dev agent                | Agent-loop + tool-use patterns          |
| `openinterpreter/open-interpreter` | Local code/agent runtime         | Minimal viable agent surface            |
| `cline/cline` (VS Code)          | IDE agent, screenshot-to-model flows | Vision attach UX                       |
| `Aider-AI/aider`                 | Terminal pair-programmer with image input | `--image` vision wire patterns        |
| `continuedev/continue`           | Open assistant IDE extension      | Provider-config UX                      |

## Chat / streaming UI (Phase 7 references)

| Project                        | What it is                        | What we borrow                          |
| ------------------------------ | --------------------------------- | --------------------------------------- |
| `janhq/jan`                    | Local AI app (Tauri), multi-provider | Streaming token render; provider settings + test-connection UX |
| `Bin-Huang/chatbox`            | Desktop LLM client                | Chat panel layout at desktop scale      |
| `open-webui/open-webui`         | Self-hosted chat UI               | Markdown/code-block rendering polish    |
| `danny-avila/LibreChat`         | Multi-provider chat               | Endpoint-compat matrix (OpenAI/xAI/Gemini/Ollama) |
| `enricoros/big-AGI`            | Web AI chat with vision           | Image-attach + follow-up threading UX   |

## Rust AI infra

| Crate / repo                   | What it is                        | What we borrow                          |
| ------------------------------ | --------------------------------- | --------------------------------------- |
| `64bit/async-openai`           | Canonical Rust OpenAI client      | SSE stream parsing (we stay on reqwest; crib the event-loop shape) |
| `0xPlaygrounds/rig`            | Rust LLM framework                | Provider-trait design vs our `AiClient` |
| `modelcontextprotocol/rust-sdk` | Official MCP Rust SDK (RMCP)      | The stretch-goal MCP server surface     |

## Landscape notes

- The screenshot-tool-as-MCP-server angle has no strong Linux-native
  incumbent yet: screenpipe (memory, not annotation) and peekaboo
  (macOS-only) point at it, but neither is an annotating screenshot
  tool. That's the opening behind our MCP stretch goal.
- Streaming is table stakes in the chat apps above; every one treats
  token-by-token render + cancellable requests as the baseline UX,
  which is why it leads Phase 7.
