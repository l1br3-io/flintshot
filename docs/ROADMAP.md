# Roadmap

Thin index of directions and milestones for `flintshot`. Per-effort
requirements live in [`specs/`](specs/) (one directory per effort,
status-flipped in place when shipped). For the narrative "why", see
[`VISION.md`](VISION.md); for decision rationale, see
[`adrs/`](adrs/).

**CURRENT MILESTONE: Ask-the-shot shipped** (2026-09-25; see
[`specs/ai-transforms/`](specs/ai-transforms/)). All planned phases
through Phase 6 are shipped. Next up: Phase 7 (AI v2 — streaming
first). The `1.0.0` clock is running: tag once we have 30 days of
dogfooding (earliest 2026-09-14).

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

## Phase 7 — Ask-the-shot v2 (next)

Prioritized from the v1 non-goals list
([`specs/ai-transforms/spec.md`](specs/ai-transforms/)):

1. **Streaming responses** — SSE on the OpenAI-compatible endpoint;
   token-by-token render in the AI tab, with a **Stop** control to
   cancel mid-generation (table-stakes pairing in every reference
   chat app). Biggest perceived-latency win: vision round-trips run
   5–30 s and today land all at once. Implementation reference:
   `async-openai`'s SSE event loop (see
   [`REFERENCES.md`](REFERENCES.md)).
2. **Remember last-used transform** — pre-select it in the transform
   dialog; most users run the same 1–2 transforms repeatedly.
3. **Live-provider verification pass** — spec success criteria 2–5
   still need a manual pass against a real endpoint (real key,
   OCR round-trip, model-sees-the-annotation, follow-up thread).

Still deferred (re-prioritize after Phase 7): image references in
follow-ups, multiple concurrent providers, transform prompt
templating, AI-generated image edits, fuller chatbox.

## Versioning

- `0.x.y` throughout development; no API stability guarantees.
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
- GIF / short-video capture (probably out of scope; consider
  `gpu-screen-recorder` integration instead)
- Annotation presets (named combinations of color + width + font)
- Plugin system for `Backend` trait (dylib or WASM?)
- Flatpak packaging with `xdg-desktop-portal` integration baked in
- Windows packaging via MSI, macOS via notarized `.app`
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
SPEC     "Current effort requirements"    docs/specs/<effort-slug>/spec.md
```

### Each effort ships through the harness lifecycle

```text
DEFINE → /spec       spec-driven-development             → docs/specs/<slug>/spec.md (status: draft)
PLAN   → /planning   planning-and-task-breakdown         → docs/specs/<slug>/{plan.md, todo.md, tasks/00N-*.md}
BUILD  → /build      incremental-implementation + TDD    → source + tests (flips spec status: draft → active)
VERIFY → /test       debugging-and-error-recovery        → pnpm verify green
REVIEW → /review     code-review-and-quality             → sign-off
SHIP   → /ship       shipping-and-launch                 → merge PR; spec status flips to shipped (dir stays in place)
```

When an effort ships, its spec.md frontmatter flips to `status: shipped`
with the merge date. The directory stays at `docs/specs/<slug>/` forever
— no archive step, no move. Git is the full history; the filesystem is
the browsable index. The next effort starts on a fresh branch with a
fresh `docs/specs/<new-slug>/` directory.
