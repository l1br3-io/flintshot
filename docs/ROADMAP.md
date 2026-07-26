# Roadmap

Directions and milestones for `flintshot`. Per-phase requirements live
in `SPEC.md` (current phase) or `tasks/archive/` (history); this file is
the thin index. For the narrative "why", see
[`VISION.md`](VISION.md); for decision rationale, see
[`DECISIONS.md`](DECISIONS.md).

**CURRENT MILESTONE: Phase 1 — Capture + Editor Window** (in flight).

## Milestones

| Milestone                                     | State        |                                                                             |
| --------------------------------------------- | ------------ | --------------------------------------------------------------------------- |
| Phase 0 — Scaffolding                         | Shipped      | [`archive`](../tasks/archive/2026-07-20-phase-0-scaffolding/)               |
| Phase 1 — Capture + editor window             | **In flight**| [`SPEC.md`](../SPEC.md) · [`tasks/plan.md`](../tasks/plan.md)               |
| Phase 2 — Tray, hotkeys, multi-monitor, polish| Planned      |                                                                             |
| Phase 3 — Pin-to-desktop + COSMIC applet      | Planned      |                                                                             |
| Phase 4 — Upload                              | Planned      |                                                                             |
| Phase 5 — Live overlay (Flameshot-style)      | Planned      |                                                                             |
| Phase 6 — Wayland global hotkeys via `ashpd`  | Planned      |                                                                             |

## Versioning

- `0.x.y` throughout development; no API stability guarantees.
- `1.0.0` once Phase 4 (upload) lands and we have at least 30 days of
  dogfooding.
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
SPEC     "Current phase requirements"     SPEC.md  (root, one per phase)
```

### Each milestone ships through the harness lifecycle

```text
DEFINE → /spec       spec-driven-development             → SPEC.md
PLAN   → /planning   planning-and-task-breakdown          → tasks/plan.md + tasks/todo.md
BUILD  → /build      incremental-implementation + TDD     → source + tests
VERIFY → /test       debugging-and-error-recovery         → pnpm verify green
REVIEW → /review     code-review-and-quality              → sign-off
SHIP   → /ship       shipping-and-launch                  → tasks/archive/<date>-<slug>/{SPEC.md, plan.md, todo.md}
```

When a phase ships, its SPEC.md + plan.md + todo.md archive **together**
under `tasks/archive/YYYY-MM-DD-<slug>/`, and the next phase starts fresh
at `/spec`. A plan never outlives its phase; a spec never outlives its
plan.
