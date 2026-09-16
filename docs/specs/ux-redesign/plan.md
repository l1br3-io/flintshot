# Plan: UX Redesign — five-zone UI + settings route

## Overview

Rebuild the egui UI around the five-zone design of record
(`reference/zone-map.html`): topbar (Capture/Copy/Open + overflow),
left tool rail, canvas, sections-only properties panel, statusbar —
plus the new settings route (`reference/settings.html`) with real
config plumbing, and the palette/type/shadow token refresh the mockups
define. Foundation (tokens, fonts, easing, recents) from
popup-book-ui is kept. Spec: [`spec.md`](spec.md).

## Architecture Decisions

- **Tokens first** — the palette-of-record refresh lands as its own
  first task so every later zone is built against final values; no
  task styles against retiring colors.
- **Shared `widgets.rs`** — ghost button, well rows, switch, collapsible
  section, kbd chip live in one module; topbar/rail/panel/statusbar/
  settings compose them. Keeps the token-consumption rule mechanical.
- **One topbar component, route-aware** — router draws it across
  Home/Editor/Settings/Loading; worker editor windows draw it
  themselves. Overflow contents vary by route (editor: Save/Upload/
  Paste; home/settings: About+note; all: Settings entry).
- **Settings = additive config + live apply** — new kebab-case fields
  with serde defaults (`capture.default-mode`, `ui.show-statusbar`,
  `ui.statusbar-size-mode`, `ui.animations`, `output.*`); changes
  persist via the existing config save + watcher round-trip; old TOML
  keeps loading. Readonly rows (hide-from-shot, default tool,
  shortcuts, display server, version) display real values.
- **Action redistribution, nothing orphaned** — Capture/Copy → topbar;
  Save/Upload/Paste/About/diagnostics/Settings → overflow; Delete →
  Selection section; undo/redo/magnifier stay floating (restyled,
  rearranged).
- **Draw order = demo z-order** — top → bottom → left → right panels;
  fresh panel ids.

## Task Index

### Phase 1: Foundation
- [ ] Task 1: Palette + token refresh to the reference of record → `tasks/001-palette-tokens.md`
- [ ] Task 2: Shared cutout widgets module → `tasks/002-widgets.md`
- [ ] Task 3: Statusbar + editor wiring → `tasks/003-statusbar.md`

### Checkpoint: Foundation
- [ ] Tokens/widgets landed; editor shows header/rail-era panel/statusbar; full gate green

### Phase 2: Structural pivot
- [ ] Task 4: Left tool rail (tools leave the right panel) → `tasks/004-tool-rail.md`
- [ ] Task 5: Topbar with hero actions on editor surfaces → `tasks/005-topbar-editor.md`
- [ ] Task 6: Properties panel — binding section map, sections only → `tasks/006-panel-sections.md`

### Checkpoint: Editor matches the mockup
- [ ] Five-zone editor; side-by-side with `reference/zone-map.html` reads as the same design; every control reachable

### Phase 3: Home + canvas polish
- [ ] Task 7: Topbar on home + welcome re-skin → `tasks/007-topbar-home.md`
- [ ] Task 8: Canvas paper dressing + floating-chip restyle → `tasks/008-canvas-dressing.md`

### Checkpoint: Home matches the mockup
- [ ] Home topbar enablement + re-skinned welcome; chrome polish landed

### Phase 4: Settings route
- [ ] Task 9: Settings config schema (additive fields + save/apply plumbing) → `tasks/009-settings-config.md`
- [ ] Task 10: Settings route + page UI → `tasks/010-settings-route.md`
- [ ] Task 11: Settings consumers (statusbar gate, size mode, animations gate, output save path) → `tasks/011-settings-consumers.md`

### Checkpoint: Settings live
- [ ] Every mapping-table row renders, persists, and applies; old configs load unchanged

### Phase 5: Close
- [ ] Task 12: Fidelity sweep + docs → `tasks/012-sweep-docs.md`

### Checkpoint: Complete
- [ ] All spec success criteria met; full gate green; ready for review

## Risks and Mitigations

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| egui panel draw order / id collisions change zone spans | Med | One zone per task with headless `Context` span assertions (header-test pattern); fresh panel ids |
| Worker-window resize geometry drifts as chrome heights change (44px topbar, 30px statusbar) | Med | Geometry seam updated in the task that changes each height; pinned-size tests kept green |
| Config round-trip breaks existing TOML or fights the watcher | High | Additive serde-defaulted fields only; round-trip + old-config tests in Task 9 before any consumer lands |
| Palette refresh touches every module at once | Med | Landed first (Task 1) as a mechanical value sweep with the token-scan test as the net |
| Large test suites churn in panel/app/header | Med | Tests travel with the code they test; coverage never deleted |
| Settings scope creep (hotkey editing, default-tool editing) | Med | Readonly rows locked by spec boundary; deviations require ask-first |
| Overlay regressions | Low | Overlay out of scope; shared-widget changes run overlay tests in every gate |

## Open Questions

- None blocking — spec §Open Questions carries the three build-time
  confirmations (after-grab vocabulary, relative-time thresholds,
  direct-save precedence).
