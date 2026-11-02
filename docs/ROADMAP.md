# Roadmap

Thin index of directions and milestones for `flintshot`. Per-effort
requirements and outcomes live in git history — the trail of any
effort is `git log --grep='<effort-slug>'` (see AGENTS.md "Git as
docs"). For the narrative "why", see [`VISION.md`](VISION.md); for
decision rationale, see [`adrs/`](adrs/).

**CURRENT MILESTONE: screen recording shipped** (2026-11-01; the
`record-engine` → `record-cli` → `record-ui` efforts, ADR-0070).
Phase 7's buildable items shipped even earlier — 2026-10-03, effort
`ai-streaming` (streaming + Stop, remember-last-transform); only the
live-provider verification pass remains (§ Phase 7 below). Next up:
the post-recording feature batch (export depth, session resume, CLI
scripting, OCR, replay buffer, window capture, …), specced
in-session 2026-11. The `1.0.0` clock is running: tag once we have
30 days of dogfooding (earliest 2026-09-14).

## Milestones

| Milestone                                      | State         |
| ---------------------------------------------- | ------------- |
| Phase 0 — Scaffolding                          | Shipped       |
| Phase 1 — Capture + editor window              | Shipped       |
| Phase 2 — Daemon, tray, hotkeys, IPC           | Shipped       |
| Phase 2b — Annotation tools, magnifier, polish | Shipped       |
| Phase 3 — Pin-to-desktop (pin later removed, ADR-0049) | Shipped |
| Phase 3b — COSMIC panel applet                 | Shipped       |
| Phase 4 — Upload                               | Shipped       |
| Phase 5 — Live overlay (Flameshot-style)       | Shipped       |
| Phase 6 — Wayland global hotkeys via `ashpd`   | Shipped       |
| Ask-the-shot — AI transforms over the canvas (`ai-transforms`, ADR-0069) | Shipped |
| Phase 7 — Ask-the-shot v2: streaming + Stop, remember-transform (`ai-streaming`) | Shipped (live-provider pass pending) |
| Screen recording — Linux MP4 engine + `record` verb + every UI surface (`record-*`, ADR-0070) | Shipped |
| Flathub — `io.l1br3.Flintshot` in the COSMIC Store | Shipped |

## Phase 7 — Ask-the-shot v2

Items 1–2 below shipped 2026-10-03 (effort `ai-streaming`;
`git log --grep='ai-streaming'`): SSE streaming with token-by-token
render, a Stop pill that aborts within ~100 ms keeping the partial,
invisible non-streaming fallback for providers that reject
`stream: true`, and remember-last-transform preselect. Remaining:

3. **Live-provider verification pass** — the v1 spec's success
   criteria 2–5 still need a manual pass against a real endpoint
   (real key, OCR round-trip, model-sees-the-annotation, follow-up
   thread), now extended with the streaming-era rows (token render,
   Stop mid-stream, partial-on-mid-stream-failure).

Still deferred (re-prioritize after Phase 7): image references in
follow-ups, multiple concurrent providers, transform prompt
templating, AI-generated image edits, fuller chatbox.

Post-Phase-7 order (evidence-backed, see
[`research/2026-09-26-phase7-adaptation-research.md`](research/2026-09-26-phase7-adaptation-research.md)):
**MCP capture tool** first (RMCP server is ~20 LOC + IPC plumbing;
no Linux-native incumbent), then **OCR region → clipboard**
(flameshot #702: 60👍 open since 2018; CLI-wrapper approach, no new
link deps), then the action pipeline and searchable history that
build on OCR.

## Versioning

- `0.x.y` throughout development; no API stability guarantees.
- First tagged release: `v0.1.1` (2026-10-17) via the cargo-dist
  ring (ADR-0068).
- `1.0.0` once Phase 4 (upload) lands and we have at least 30 days of
  dogfooding. Phase 4 shipped 2026-08-15; earliest `1.0.0` is
  2026-09-14, gated on no P1 regressions during dogfooding.
- Post-1.0 semver: breaking crate API / user-visible behavior changes
  bump minor; fixes bump patch.

## Stretch goals (post-1.0)

- OCR via `tesseract` or `rusty-tesseract`
- Searchable capture history — OCR-index the session capture store
  (ADR-0067) so every past shot is text-searchable, local-only
  (screenpipe / Screenotate precedent)
- Configurable after-capture actions — user-defined pipeline per
  capture: copy / save / upload / AI transform / OCR (ShareX
  pattern; our upload + AI pieces already exist)
- GIF output for recordings (probably out of scope; consider
  `gpu-screen-recorder` integration instead)
- Audio and live-casting in recordings — deferred with ADR-0070:
  both force fMP4 finalize + a new muxer; the `Recorder` contract
  stands as shipped
- Annotation presets (named combinations of color + width + font)
- Plugin system for `Backend` trait (dylib or WASM?)
- Release-channel activation — the winget manifest PR, the AUR
  `flintshot-bin` push, and macOS notarization (MSI/zip + DMG
  already build via the cargo-dist ring, ADR-0068; runbook in
  [`packaging/RELEASE.md`](../packaging/RELEASE.md))
- Alternative editor shells: `flintshot-editor-iced`,
  `flintshot-editor-slint` (exercise the abstraction)
- MCP server surface: expose `capture` as an MCP tool so agents
  (goose, Cline, OpenClaw…) can request screenshots from flintshot

## Reference projects (open source)

Consolidated in [`REFERENCES.md`](REFERENCES.md): product
comparables (screenpipe, NormCap, peekaboo…), the "goose" agent
family, chat/streaming UI references for Phase 7, and Rust AI infra
(async-openai, rig, RMCP).

## How we plan

### Scope decomposes top-down

```text
VISION   "What problem are we solving?"   docs/VISION.md
  │
ROADMAP  "What directions / milestones?"  docs/ROADMAP.md  (this)
  │
SPEC     "Current effort requirements"    git log (enriched commit messages)
```

### Each effort ships through the harness lifecycle

```text
DEFINE → /spec       spec-driven-development             → approved spec (presented in session, no files)
PLAN   → /planning   planning-and-task-breakdown         → task list with acceptance + verification
BUILD  → /build      incremental-implementation + TDD    → source + tests (one commit per task)
VERIFY → /test       debugging-and-error-recovery        → just ci green
REVIEW → /review     code-review-and-quality             → sign-off
SHIP   → /ship       shipping-and-launch                 → merge PR
```

When an effort ships, its knowledge lives in git: the completion commit
carries a Context / Decisions / Deviations / Outcome block (see AGENTS.md
"Git as docs"), and every commit carries a scope-prefixed subject, so the
full spec trail of any effort is `git log --grep='<effort-slug>'`. Git is
the full history; the log is the browsable index. The next effort starts
on a fresh branch.
