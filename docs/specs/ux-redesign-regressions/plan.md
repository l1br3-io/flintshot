# Plan: UX redesign regressions — home geometry, editor Copy, tofu icons

## Overview

Fix the three regressions reported against the in-flight `ux-redesign`
branch, plus two proven adjacency defects: home geometry still pinned
to the pre-redesign 360×240, the topbar's text-glyph icons rendering
as tofu (cmap-verified absent from the entire embedded font chain),
and Copy succeeding invisibly (no feedback surface, dead-looking
button) while the worker-window close-after-copy path provably loses
the Wayland clipboard on process exit (COSMIC does not adopt an exited
client's zwlr selection). Spec: `docs/specs/ux-redesign-regressions/spec.md`.

Implementation branch: `ux-redesign-regressions`, cut from `ux-redesign`
(the in-flight work must be committed there first — decide at /build start).

## Architecture Decisions

- **Painted vector glyphs, not a symbol font**: topbar / overflow /
  breadcrumb icons become `Glyph` fns (`fn(&Painter, Rect, Color32)`),
  following rail/panel/floating precedent — zero font dependence, no
  new dependency (spec decision, binding).
- **One geometry for every route**: `route_geometry` gives Home the
  same 1024×640 resizable batch as Editor/Loading/Settings;
  `run_window` builds its viewport from the same fn, so initial size
  and transitions fix together.
- **Copied feedback is time-windowed on the hero**: `EditorApp` keeps
  the copy instant; the hero renders the check glyph + "Copied" label
  while inside a ~2 s window, requesting repaints until it reverts. A
  fresh editor route resets it for free (new `EditorApp`).
- **Copy-click tests stay clipboard-free on CI**: the full click chain
  needs a display for arboard; CI asserts the rendering + time-window
  seams headlessly, the real clipboard stays on the manual protocol
  (`wl-paste --list-types`).
- **Worker windows stop closing after Copy on Linux** (Task 006,
  ask-first): the close kills the process and, per probe evidence, the
  Wayland selection with it. macOS/Windows semantics (OS-owned
  clipboard) keep closing.

## Task Index

### Phase 1: Foundations
- [ ] Task 1: Home route geometry matches the stable window geometry → `tasks/001-home-stable-geometry.md`
- [ ] Task 2: Topbar glyph kit — painted vector icons → `tasks/002-topbar-glyph-kit.md`

### Checkpoint: Foundations
- [ ] Full gate green (fmt, clippy -D warnings, workspace tests, doc tests)
- [ ] Manual: `cargo run -- app` opens Home at 1024×640, resizable

### Phase 2: Visible fixes
- [ ] Task 3: Topbar consumes painted glyphs + tofu-ban scan → `tasks/003-topbar-painted-icons.md`
- [ ] Task 4: Copy feedback — the hero's Copied state → `tasks/004-copy-copied-state.md`

### Checkpoint: Visible fixes
- [ ] Manual: all topbar icons render (no tofu), Copy shows "Copied"
- [ ] Manual clipboard protocol: copy in editor page → `wl-paste --list-types` lists `image/png`

### Phase 3: Correctness
- [ ] Task 5: Settings-route Capture dispatches (dead control) → `tasks/005-settings-capture-dispatch.md`
- [ ] Task 6: Worker Copy no longer closes the window on Linux → `tasks/006-worker-copy-lifetime.md`

### Checkpoint: Correctness
- [ ] Full gate green
- [ ] Manual: worker editor (`flintshot open <img> --standalone`) Copy leaves the window open, paste works after

### Phase 4: Closeout
- [ ] Task 7: CHANGELOG, ADR, full gate + manual protocol → `tasks/007-closeout.md`

### Checkpoint: Complete
- [ ] All spec success criteria met
- [ ] Ready for review / PR

## Risks and Mitigations

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| Hero/ghost_button signature churn collides with in-flight ux-redesign edits | Med | Build on the working tree as-is; small per-task commits; never revert parent-work files |
| Copy click-through can't run on CI (no display for arboard) | Med | CI-safe seams: copied-state rendering + time-window unit tests; real clipboard on the manual protocol |
| Worker no-close policy is a UX change (window lingers) — ask-first territory | Med | Gated on this plan's approval; ADR in Task 7 records the probe evidence either way |
| User's page-mode paste failure never reproduced in isolation | High | Retest with the manual protocol once feedback exists (failure may have been perception); if still failing, capture paste-target + `wl-paste` output as new evidence |
| Painted glyph aesthetics drift from the mockups | Low | Reuse panel/rail glyph vocabulary (g_copy, g_check, arrowhead); manual side-by-side with `reference/zone-map.html` |

## Open Questions

- Task 6 policy approval (this plan review is the ask-first gate):
  worker stays open after Copy on Linux — confirm.
- Home content column width at 1024+: full-width hero vs a centered
  ~640 column (mockup reads centered). Decide in Task 1's manual pass.
