---
status: active
started: 2026-09-16
---

# Spec: Pop-up Book UI — design-token system + editor/home redesign

## Objective

Redesign Flintshot's entire egui UI (home window + editor) around one
centralized design-token system with a **3D pop-up book / paper-cutout
aesthetic**, replacing the current dark graphite/ember instrument
panel. The Gradia *layout* patterns survive (welcome screen that is
never blank, sectioned side panel, contextual tool options, popovers
for dense options, floating on-canvas controls, grouped export
cluster); the *skin* is the user-selected pop-up-book style: cream
paper surfaces, cutout cards with thick white borders, dimensional
painted shadows, springy overshoot microinteractions, subtle paper
grain.

Only currently shipped features are redesigned — no roadmap features.
Structural seams (tab strip, Objects slot, AI entry, multi-select-
capable selection model) are reserved but inert.

**User**: Linux screenshot users on X11 and Wayland who capture,
annotate, and hand off images quickly.

**Hero feature** (decided): **instant annotate-and-copy** — Copy to
clipboard is the visually anchored, fastest-to-reach export action.

**Theme scope** (decided): **light-only** for this effort. The
original redesign prompt asked for light + dark; the user explicitly
deferred dark to a follow-up effort. `ThemePreference::Light` is
pinned.

**Typeface** (decided): **Plus Jakarta Sans** (OFL), one family —
ExtraBold for display, Regular/Medium for UI text. The pasted style
spec's "Chubbo" does not exist on Google Fonts; Kalam (hand-drawn
narrative font) was declined in favor of one-family prompt
compliance.

### Design decisions from clarification (binding)

| Decision    | Choice                                                    |
| ----------- | --------------------------------------------------------- |
| Hero        | Instant annotate-and-copy (Copy = primary export button)  |
| Skin        | Full pop-up-book everywhere, incl. editor chrome          |
| Dark theme  | Deferred (follow-up effort); light-only now               |
| Font        | Plus Jakarta Sans only, embedded (OFL)                    |
| Tools live  | Docked sectioned side panel (not floating toolbar)        |
| Recents     | Persistent path index in the config dir                   |

### Web-spec → egui feasibility translations (binding)

The pasted style spec is web-flavored; egui is immediate-mode 2D.
These adaptations are the spec of record:

| Web spec                                    | egui implementation                                                        |
| ------------------------------------------- | -------------------------------------------------------------------------- |
| `filter: drop-shadow` 10–30px @ 0.3–0.4 α    | `egui::Shadow` on frames/popups + painted layered translucent rects for pods |
| `cubic-bezier(0.34, 1.56, 0.64, 1)` springs  | Pure-Rust cubic-bezier easing helper (`ease` fn) + manual animation state   |
| Z-axis "explode" up to 600px, scroll scaling | Hover-lift microinteraction: shadow grows, card translates up ~4px, slight scale; popovers/pop-ins use overshoot easing. No scroll-linked animation (nothing scrolls except the recents grid). |
| Cardboard/paper grain on all surfaces        | Procedurally generated deterministic grain tile, painted at low alpha on panels/welcome — never on the canvas image or annotations |
| 8px white borders on cutout elements         | Thick white stroke token on every pod/card/chip                              |

## Technologies

- Rust (edition 2021, MSRV 1.95), egui/eframe 0.35 (workspace-pinned).
- `Plus Jakarta Sans` static TTFs (Regular, Medium, ExtraBold) added
  under `crates/flintshot-editor-egui/assets/fonts/` with the OFL
  license text; embedded via `include_bytes!` into `FontDefinitions`.
- No new crate dependencies. Recents persistence uses existing
  `dirs` + `serde` paths in `flintshot-core`.

## Commands

From repo root (all already in AGENTS.md):

```sh
cargo build                                    # build default members
cargo fmt --all --check                        # format check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets           # unit + integration
cargo test --doc -p flintshot-core -p flintshot-editor-egui  # doc tests
cargo run -- app                               # home/editor window for manual check
```

## Project Structure

```
crates/flintshot-editor-egui/src/
├── theme/            # NEW — the design-token system (replaces theme.rs)
│   ├── mod.rs        #   tokens: spacing scale, rounding, strokes, shadow,
│   │                 #   type ramp; `apply(ctx)`; light Visuals + Style
│   ├── palette.rs    #   color tokens only (forest/gold/cream ramp)
│   ├── grain.rs      #   procedural paper-grain tile (pure, deterministic)
│   └── ease.rs       #   cubic-bezier(0.34,1.56,0.64,1) evaluator + helpers
├── home.rs           # welcome screen: hero capture card, recents grid,
│                     #   open/paste/drop (HomeBridge contract unchanged)
├── panel.rs          # NEW — docked side panel: tool pods, contextual
│                     #   options sections, Objects slot (hidden), export
│                     #   cluster; absorbs current toolbar.rs role
├── toolbar.rs        # dissolved into panel.rs / popover.rs (removed)
├── popover.rs        # NEW — color/stroke picker bay as popover
├── floating.rs       # NEW — on-canvas undo/redo/magnifier/zoom chips
├── header.rs         # NEW — header bar: identity, tab-strip seam (single
│                     #   tab), AI-entry seam (hidden), overflow menu
├── shell.rs          # editor layout wiring (canvas center + panel + floating)
└── ...               # canvas.rs, chrome.rs, magnifier.rs, window.rs, app.rs
crates/flintshot-core/src/
└── recents.rs        # NEW — persistent recents path index (pure logic:
                      #   load/save/prune/cap; no GUI types — boundary test
                      #   no_gui_types_in_core.rs must keep passing)
docs/specs/popup-book-ui/
└── spec.md           # this file
```

## Design system of record

### Color tokens (light only; `palette.rs`)

| Token           | Value      | Role                                                |
| --------------- | ---------- | --------------------------------------------------- |
| `PAPER_CREAM`   | `#FFF9F0`  | panel/window base fill                              |
| `CUTOUT_WHITE`  | `#FFFFFF`  | card/pod fill + 8px cutout border stroke            |
| `FOREST_DEEP`   | `#1F5233`  | **primary accent**: active tool, selection, hero Copy fill, links |
| `FOREST_MID`    | `#2D7A3E`  | hover/secondary interactive, success                |
| `AUTUMN_GOLD`   | `#FFB347`  | warm highlight: hover-lift ring, focus, recents hover |
| `FOX_ORANGE`    | `#E67E22`  | annotation-swatch default, mid emphasis             |
| `BERRY_RED`     | `#C0392B`  | destructive (delete annotation), annotation swatch  |
| `INK`           | `#22301F`-ish (tuned in build) | primary text on cream            |
| `INK_SOFT`      | derived    | secondary text/hints                                |
| `HOLIDAY_*`     | derived    | hairlines/borders (cream-tinted neutral ramp)       |

Annotation swatch set re-anchors on the palette (Berry Red, Fox
Orange, Autumn Gold, Forest Mid, a sky, white, ink) — same seven-slot
shape as today, hues refreshed. The current `EMBER`/graphite tokens
retire with `theme.rs`.

### Non-color tokens (`theme/mod.rs`)

- **Spacing scale**: exactly `4 / 8 / 12 / 16 / 24` logical px.
  Pod and control padding grows ~30–50% vs current values (e.g.
  button padding 7×3 → 10×6; pod inner pad 3 → 6).
- **Rounding**: one pod/card radius (e.g. 10) + one chip radius
  (e.g. 7); single source of truth consumed by every pod, popover,
  and handle.
- **Cutout stroke**: 8px `CUTOUT_WHITE` border token on cards; hairline
  (`HOLIDAY_*`) 1px for inner chips.
- **Shadow**: two levels — resting (10px, α 0.30) and lifted
  (up to 30px, α 0.40) — via `egui::Shadow` + painted rects.
- **Type ramp**: Plus Jakarta Sans — display (ExtraBold) 22–28,
  title 16, body 13, caption/hint 11, all logical px, defined once.

### Layout breakdown

1. **Header bar**: Flintshot identity, **reserved tab-strip region
   rendering exactly one tab** today (the open document), reserved
   single **AI entry slot (hidden)**, overflow menu (backends
   diagnostics, about). Never gets tool pods.
2. **Welcome screen** (no image open): large "Take a screenshot"
   hero cutout card; **recents grid** (persistent path index,
   thumbnails decoded on demand); Open / Paste controls; drag-and-drop
   image target. Every state renders content — never blank.
3. **Editor**: canvas center (never grained/shadowed), **floating
   on-canvas chips** (undo/redo, magnifier, zoom) as cutout chips,
   **docked right side panel** with sections:
   - *Tools* — the ten current tools as grouped cutout pods.
   - *Tool options* (contextual) — only the selected tool's options.
   - *Objects* — reserved slot, hidden today (grouping seam).
   - *Export cluster* (panel bottom) — **Copy (hero, forest-filled)**,
     Save, Pin, Upload.
4. **Popovers**: color + stroke-width picker bay opens as a popover
   from a single pod button (pattern #4); fill toggle, number-bubble
   start, blur strength live in the contextual section.

### Always-visible vs popover/overflow

- **Always visible (tool pods)**: Move, Rectangle, Ellipse, Line,
  Arrow, Pencil, Marker, Text, Number, Blur — all ten stay visible;
  nothing is deleted.
- **Popover/overflow**: color/stroke bay, fill toggle, number start,
  blur strength, backends diagnostics, any future rare control.

### Forward-compatibility seams (inert only)

1. **Tabs**: header tab-strip region exists, renders one tab; each
   open image is already a document concept (state per window).
2. **Grouping**: selection model stays single-select but the
   EditorState chrome/API shape must not preclude multi-select
   (documented in code where `selected_index` is consumed); Objects
   panel slot reserved, hidden.
3. **AI**: exactly one hidden header slot; nothing attached to tool
   pods.

## Recents index (new plumbing, minimal)

- `flintshot-core::recents`: ordered list of image paths (+ mtime),
  capped at 20, stored as `<config_dir>/flintshot/recents.toml`
  (serde kebab-case per repo convention).
- Recorded on: capture-to-editor, open, paste-to-file (if any),
  editor save.
- Dead paths pruned at load; thumbnails never persisted (decoded +
  downscAled on demand, in-memory only).
- Opening a recent routes through the existing open-image path.

## Code Style

Repo conventions (AGENTS.md) apply: `#![forbid(unsafe_code)]`,
pedantic clippy, no `unwrap()`/`expect()` outside tests, no comments
restating code, `#[non_exhaustive]` on plausible-extension public
types, doc comments on public items in core.

Token consumption is the only style rule this effort adds:

```rust
// GOOD — tokens only, no literals at use sites
let card = egui::Frame::default()
    .fill(theme::palette::CUTOUT_WHITE)
    .stroke(theme::cutout_stroke())
    .corner_radius(theme::POD_RADIUS)
    .inner_margin(theme::space(12))
    .shadow(theme::shadow_lifted(t));
// t = hover progress eased with theme::ease::spring(t)

// BAD — ad-hoc Color32 at a widget site (rejected in review)
ui.style_mut().visuals.widgets.active.bg_fill = Color32::from_rgb(0x1F, 0x52, 0x33);
```

All `Color32` literals live in `theme/palette.rs` and nowhere else.

## Testing Strategy

- **Token module** (`theme/`): unit tests mirroring the current
  theme.rs suite — palette distinctness (primary accent never collides
  with neutrals), spacing scale exactly {4,8,12,16,24}, ramp ordering.
- **Easing** (`theme/ease.rs`): pure math — evaluator hits (0,0)/(1,1),
  overshoot stays within bounds, monotonic input time.
- **Grain** (`theme/grain.rs`): deterministic (same seed → identical
  tile), alpha stays below the spec ceiling.
- **Recents** (`flintshot-core::recents`): tempfile-backed tests for
  save/load round-trip, cap at 20, dead-path prune, corrupt-file
  recovery (start empty).
- **Regression**: all existing editor/shell tests must pass unchanged
  (shortcuts, chrome, routing); `no_gui_types_in_core.rs` must keep
  passing (recents is pure paths).
- **Manual**: `cargo run -- app` — welcome → capture → annotate →
  copy; hover-lift/popin feel; shortcuts unchanged.

## Boundaries

- **Always**: consume tokens (no ad-hoc `Color32` outside
  `theme/palette.rs`); run the full fmt/clippy/test suite before
  declaring done; keep every current keyboard shortcut working; keep
  every rarely-used control reachable (popover/overflow, never
  deleted); update CHANGELOG under `## [Unreleased]`.
- **Ask first**: new crate dependencies; any `flintshot-core` public
  API change beyond the additive `recents` module; deviating from
  light-only; changing window/close/capture choreography
  (ADR-0047 flow must not regress).
- **Never**: implement roadmap features (working tabs, grouping/
  multi-select behavior, any AI function); put grain/shadows over the
  canvas image; remove the no-Flintshot-in-the-shot guarantee;
  `unsafe`; regress shortcuts; scatter style literals at call sites.

## Success Criteria

1. `theme/` is the single style source: `rg "Color32::from_" crates/flintshot-editor-egui/src` matches only `theme/palette.rs`.
2. Style applied centrally via `Context::set_style`/`set_visuals`;
   spacing scale constants used; padding visibly larger than current.
3. Plus Jakarta Sans embedded; type ramp constants defined and used;
   OFL license file committed beside the TTFs.
4. Welcome screen shows hero capture card + recents grid + open/
   paste/drop in every state — never blank.
5. Recents persist across restarts (manual check), capped, dead paths
   dropped (unit-tested).
6. Side panel shows exactly the selected tool's options; color/stroke
   bay opens as a popover; all ten tools visible.
7. Undo/redo, magnifier, zoom render as floating on-canvas chips.
8. Export cluster: Copy is the forest-filled primary; Save/Pin/Upload
   grouped beside it.
9. Header renders one tab; AI slot and Objects slot exist in code but
   are hidden.
10. Hover-lift + popover pop-in use the spring easing helper
    (unit-tested); shadows match the two-level token.
11. Existing tests + new tests green under the full command list;
    clippy `-D warnings` clean.
12. Grain never paints over the canvas or annotation chrome.

## Open Questions

- Dark ("deep forest") variant — scheduled as its own follow-up
  effort after this lands.
- Exact `INK`/neutral ramp hues — tuned during build against real
  screenshots, within the tokens defined above.
- Recents thumbnail size/aspect handling for very tall/wide captures
  — decide during build (letterbox on cream is the default).
