# ADR-0056: The five-zone layout and action redistribution

## Context

The ux-redesign effort (spec `docs/specs/ux-redesign/`) replaced the
popup-book arrangement — quiet header bar, right panel with a tools
card + export island + `+` popup, collapse-to-rail mode — with the
mockup's five zones: topbar, left tool rail, canvas, properties
panel, statusbar. Three seams deserved a recorded decision: retiring
`header.rs` (the tab-strip and AI seams died with it), redistributing
every action the old chrome hosted, and painting canvas dressing
without polluting composed output.

## Decision

1. **Five zones, fixed draw order.** Topbar (44) → statusbar (30) →
   left rail (48) → right properties panel (236) → central canvas.
   egui reserves in that order; every zone got a fresh panel id.
2. **Action redistribution, nothing orphaned.** Capture / Copy / Open
   became topbar heroes; Save / Upload / Paste moved to the `⋯`
   overflow; Delete moved into the properties panel's always-present
   Selection section; undo / redo / magnifier stayed floating
   (rearranged top-left / bottom-right). The `+` chip + popup, the
   export island, the copy hero/square, and the collapse toggle were
   deleted — no fake controls remain.
3. **One route-aware topbar.** The router draws it across
   Home / Editor / Settings / Loading; worker editor windows draw the
   same component themselves. Overflow contents vary by route; the
   enablement matrix is a pure, unit-tested function.
4. **Canvas dressing is chrome-only.** The paper cutout (shadow +
   cream ring + hairline + corner handles) paints in the app's chrome
   pass — never in `render()` / saves — and hides during the
   selection-first phase.

## Consequences

- `header.rs` is gone; the tab-strip and AI seams retired with it
  (neither had consumers; the settings route superseded the AI slot).
- The `+` popup's page actions route through the topbar dispatcher,
  so worker and routed captures share one code path.
- Zone dimensions are pinned by span-assertion tests; layout changes
  now require touching those contracts, which is the point.
- The floating cluster hosts both corner islands in one
  click-transparent area; pointer misses still pass through to the
  canvas, preserving the ADR-0055 gating model.
