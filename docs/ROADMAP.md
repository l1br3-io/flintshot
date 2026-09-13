# Roadmap

Thin index of directions and milestones for `flintshot`. Per-effort
requirements live in [`specs/`](specs/) (one directory per effort,
status-flipped in place when shipped). For the narrative "why", see
[`VISION.md`](VISION.md); for decision rationale, see
[`adrs/`](adrs/).

**CURRENT MILESTONE: all planned phases shipped** (Phase 6, Wayland
global hotkeys, shipped 2026-08-20; see
[`specs/phase-6-wayland-hotkeys/`](specs/phase-6-wayland-hotkeys/)).
With Phase 4 landed, the `1.0.0` clock is running: tag once we have
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

## Versioning

- `0.x.y` throughout development; no API stability guarantees.
- `1.0.0` once Phase 4 (upload) lands and we have at least 30 days of
  dogfooding. Phase 4 shipped 2026-08-15; earliest `1.0.0` is
  2026-09-14, gated on no P1 regressions during dogfooding.
- Post-1.0 semver: breaking crate API / user-visible behavior changes
  bump minor; fixes bump patch.

## Stretch goals (post-1.0)

- OCR via `tesseract` or `rusty-tesseract`
- GIF / short-video capture (probably out of scope; consider
  `gpu-screen-recorder` integration instead)
- Annotation presets (named combinations of color + width + font)
- Plugin system for `Backend` trait (dylib or WASM?)
- Flatpak packaging with `xdg-desktop-portal` integration baked in
- Windows packaging via MSI, macOS via notarized `.app`
- Alternative editor shells: `flintshot-editor-iced`,
  `flintshot-editor-slint` (exercise the abstraction)

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
