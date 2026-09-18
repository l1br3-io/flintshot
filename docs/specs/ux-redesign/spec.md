---
status: active
started: 2026-09-17
---

# Spec: UX Redesign — five-zone UI + settings route

## Objective

Re-layout Flintshot's egui UI (editor, home, **and a new settings
route**) to match the reference mockups in
[`reference/`](./reference/) — `zone-map.html` (home + editor,
five-zone) and `settings.html` (settings route) — which are the
**design of record**, superseding `demo/` for layout and tokens.
The structure: **topbar, left tool rail, canvas, right properties
panel, statusbar**, replacing the popup-book-ui arrangement (header +
tool pods in right panel + panel-bottom export cluster).

This is a structural + visual effort on top of the popup-book-ui
foundation: token architecture, embedded Plus Jakarta Sans, grain,
spring easing, and recents plumbing are kept; the palette is refreshed
to the mockup's values and the layout is rebuilt around the five zones.
The Gradia-surviving patterns (never-blank welcome, contextual options,
popovers, floating canvas controls) carry forward.

**User**: Linux screenshot users on X11 and Wayland who capture,
annotate, and hand off images quickly.

**Hero actions** (decided): the topbar carries exactly three
`[icon][text]` ghost buttons — **Capture**, **Copy**, **Open** —
styled like the mockup (Capture + Copy carry the gold-tinted
`hero-key` border). Copy remains the instant-annotate-and-copy hero.

**Settings** (decided): a new routed surface exposing real
preferences — capture defaults, editor chrome toggles, output
defaults — with read-only rows for guarantees and environment facts.
No fake controls anywhere: every enabled control maps to a real
config field or action; read-only rows are visually dashed/locked.

### Decisions from clarification (binding)

| Decision     | Choice                                                                     |
| ------------ | -------------------------------------------------------------------------- |
| Layout       | Full re-layout to the five-zone structure (`reference/zone-map.html`)       |
| AI surfaces  | **Keep hidden** (popup-book-ui decision stands); seams stay inert           |
| Left rail    | Real tool set only — all ten current tools, no Crop/Eraser/Settings stubs   |
| Topbar       | Brand + breadcrumb + **Capture / Copy / Open** + `⋯` overflow               |
| Scope        | Editor, home, **and the settings route**                                    |
| Fonts        | Plus Jakarta Sans only (mockups comply)                                     |
| Pin          | Removed feature — does not exist, not part of this effort                   |
| Palette      | Refreshed to the mockup's token values (table below) — approved via mockup  |
| Floating chips | Restyled to cutout chips; undo/redo top-left, magnifier + view zoom +/− bottom-right (the chips step the canvas view's zoom multiplier over the fit) |

### Assumptions (surface-correction invited)

1. **Undo/Redo/Magnifier stay on-canvas** as floating chips
   (restyled + rearranged per mockup); the topbar holds only the three
   hero actions.
2. **Save / Upload / Paste move to the topbar `⋯` overflow** (editor
   routes; home overflow holds only About + diagnostics note — per
   mockup), plus a **Settings** entry as the route's entry point. The
   overflow also keeps the status line + `flintshot backends`
   diagnostics hint.
3. **Every control maps to a real `EditorState` field, config field,
   or action** — demo/mockup affordances without engine backing are
   dropped (zoom dropdown/badge, Fit chip, paper shadow switch),
   not stubbed.
4. **Statusbar is editor-only**; home and settings have no statusbar
   (per mockups).
5. **Worker editor windows** (no router) draw the same topbar/statusbar
   themselves; the overlay is out of scope.
6. **Delete lives in the panel's Selection section** (danger-styled,
   disabled without a selection) — always-visible section per the
   mockup's SEC_MAP.
7. **Settings entry point**: `⋯` overflow item on every route (the
   mockups show the route but no entry affordance — overflow is the
   minimal, non-cluttering home for it).
8. **Save keeps its file dialog**; the Output settings feed a
   direct-save path (template + dir + format) used when a filename
   template is configured, falling back to the dialog otherwise.

### Web-mockup → egui feasibility notes

The popup-book-ui translation table remains the spec of record
(shadows via `egui::Shadow` + painted rects, springs via
`theme::ease`, grain as deterministic tile, cutout stroke token).
New translations this effort adds:

| Mockup pattern                        | egui implementation                                            |
| ------------------------------------- | -------------------------------------------------------------- |
| `hero-key` gold-tinted border on Capture/Copy ghost buttons | Ghost-button builder with a gold-border variant token |
| Rail `tool-pop` bounce on activation  | Scale-pop driven by `theme::ease::spring` on tool switch        |
| Collapsible sections (`max-height` chevron rotate) | Custom section widget: chevron rotates, body height animates with spring easing |
| Wells (segmented value picks)         | Chip-row widget — the existing stroke-preset pattern, tokenized |
| Switches (pill + knob)                | Custom toggle widget, gold when on, locked variant at 55% opacity |
| Statusbar `kbd` chips                 | Styled labels; content from the real binding table              |
| Breadcrumb `▸` stages                 | Painted text row, current stage gold + bold                     |
| Flame brand badge                     | Painted gold rounded square + flame glyph (existing glyph style) |

## Technologies

- Rust (edition 2021, MSRV 1.95), egui/eframe 0.35 (workspace-pinned).
- **No new crate dependencies.**
- Reuses: `theme/` module architecture, `flintshot-core` editor engine,
  config load/save + watcher (`flintshot-core::config`,
  `flintshot-app` config_watcher), recents, embedded fonts.

## Design system of record (from `reference/`)

### Color tokens (light only; `palette.rs` — refreshed values)

| Token           | Value      | Role                                                |
| --------------- | ---------- | --------------------------------------------------- |
| `FOREST_DEEP`   | `#1F5233`  | topbar + statusbar fills, home hero card            |
| `FOREST_MID`    | `#2C6B45`  | left rail fill, well-selected fill                  |
| `FOREST`        | `#3A8058`  | hovers, borders                                     |
| `FOREST_INK`    | `#16381F`  | deepest shade, text on gold                         |
| `GOLD`          | `#E4B54A`  | active tool, brand badge, statusbar hint, switches  |
| `GOLD_DEEP`     | `#C99A2E`  | gold border/pressed, annotation accents             |
| `PAPER`         | `#F6EFDE`  | canvas desk, panel base                             |
| `PAPER_CREAM`   | `#FBF6EA`  | cards, controls, chips                              |
| `PAPER_DIM`     | `#EDE4CE`  | wells, insets, disabled                             |
| `INK`           | `#2A2620`  | primary text on paper                               |
| `INK_SOFT`      | `#6E6754`  | secondary text, labels                              |
| `CREAM_TEXT`    | `#F3EDDD`  | text on forest fills                                |
| `CREAM_DIM`     | `#C9C2AC`  | muted text on forest fills                          |
| `LOCK_GREY`     | `#9A947F`  | gated tools, disabled                               |
| `BERRY_RED`     | `#C0392B`  | destructive accents (selection delete tint)          |
| annotation swatches | refreshed set per mockup (berry, gold, forest, sky, ink, cream-white) | Stroke swatch row |

The `EMBER`/graphite-era and popup-book-ui v1 gold (`#FFB347`) values
retire; all consumers move to this table in the palette task.

### Non-color tokens (`theme/mod.rs`)

- **Type ramp**: xs 10.5 · sm 12 · md 13.5 · lg 16 · xl 21 ·
  hero 30 logical px (Plus Jakarta Sans; display weight for hero/title).
- **Radii**: cutout 10 (cards, bars, sections) · pod 12 (chips,
  buttons) · pill (switches).
- **Shadow levels (four)**: resting (chips/sections) · lift (hover) ·
  pop (active tool, menus) · paper (window/canvas paper, hero card).
- **Spacing scale**: `4 / 8 / 12 / 16 / 24` (kept).
- **Zone dimensions (logical px)**: topbar height 44 · rail width 48 ·
  properties panel width 236 · statusbar height 30 · rail tool 36×36 ·
  floating chip 36×36 · topbar flame badge 24.

## Layout model

```
┌──────────────────────────────────────────────────────────────────┐
│ 1 Topbar  🔥 Flintshot | Home ▸ Editor       ✚Capture ⧉Copy 🗀Open ⋯ │ 44px forest-deep
├────┬──────────────────────────────────────────────┬──────────────┤
│ 2  │                                              │ 4 Properties │
│Rail│  3 Canvas — paper cutout (white ring,       │ {Tool} ·     │
│48px│     paper shadow), corner handles,           │ Properties   │ 236px
│    │     floating chips (undo/redo TL,            │ sections     │
│    │     magnifier BR)                            │ (collapsible)│
├────┴──────────────────────────────────────────────┴──────────────┤
│ 5 Statusbar  Image: 1920×1080 px · 2.4 MiB • hint (gold)  kbd…   │ 30px forest-deep
└──────────────────────────────────────────────────────────────────┘
```

### Zone contracts

1. **Topbar** (44px, forest-deep, cutout shadow, cream text): flame
   badge + `Flintshot` + `|` separator + breadcrumb (`Home ▸ Editor`;
   past stages are links home — gold + underline on hover, a click
   routes Home / closes worker editors like Esc; the current stage is
   gold+bold, static; Loading stays inert); right: Capture / Copy /
   Open ghost buttons (Capture + Copy `hero-key` gold-tinted border;
   disabled state cream-dim) + `⋯` overflow. Editor overflow: Save
   (`Ctrl+S`), Upload, Paste (`Ctrl+V`), separator, About, note (status
   + `flintshot backends`), Settings. Home overflow: About, note,
   Settings. Copy disabled on home/settings with tooltip "No image is
   open — copy is available in the editor".
2. **Rail** (48px, forest-mid): the ten real tools as 36×36 cutout
   chips, grouped by the existing five semantic pods with hairline
   separators; active = gold fill + white border + pop shadow + spring
   bounce; locked = lock-grey, hover-only; tooltips `name · key`
   (+ lock clause).
3. **Canvas**: paper desk (`PAPER` + grain, never over the image); the
   capture as a paper cutout — white ring + forest hairline + paper
   shadow, with 12 px of padding between the ring box and the panel
   edge (the dressing never sits flush against the window chrome);
   floating chips restyled as cutouts (undo/redo top-left, magnifier
   + view zoom bottom-right).
4. **Properties panel** (236px, paper + grain): `{Tool} · Properties`
   uppercase title; contextual collapsible sections (chevron + spring
   height); section map below is **binding**.
5. **Statusbar** (30px, forest-deep): dims + byte estimate (cream
   bold) · per-tool hint (gold, centered, from the engine's
   `tool_hint` table) · `kbd` chips (Ctrl+Z, Ctrl+Y, Ctrl+S, Del).

### Per-tool section map (binding, from mockup SEC_MAP)

| Tool(s)                    | Sections                          |
| -------------------------- | --------------------------------- |
| Move                       | Selection                         |
| Rectangle, Ellipse, Pencil | Stroke, Fill, Selection           |
| Line, Arrow, Marker        | Stroke, Selection                 |
| Text                       | Stroke, Text, Selection           |
| Number                     | Stroke, Number, Selection         |
| Blur                       | Effects, Selection                |
| Select (crop phase)        | existing select/crop bay as a section |

Section inventories (all real `EditorState` fields): **Stroke** =
swatch row + width wells; **Fill** = fill-shapes switch; **Text** =
size wells + weight/style toggles + family (existing controls, mockup
styling); **Effects** = blur kind wells (Pixelate/Smooth) + strength
wells; **Number** = next-number stepper; **Selection** = danger Delete
button (disabled without selection).

## Settings route (new)

Routed page in the single window (`Route::Settings`; worker windows
never route there). Topbar on settings: Capture enabled, Copy/Open
disabled, breadcrumb `Home ▸ Settings`. Content: centered 640px column
of cutout section cards; controls are wells / switches / readonly
wells per mockup. Field mapping (kebab-case serde, additive with
defaults):

| Mockup row                   | Plumbing                                                    |
| ---------------------------- | ----------------------------------------------------------- |
| Capture · Default mode       | **new** `capture.default-mode` (region/fullscreen/window)    |
| Capture · Delay              | existing `capture.default-delay-ms` (0s/3s/5s wells)         |
| Capture · Include cursor     | existing `capture.include-cursor`                            |
| Capture · Hide from shot 🔒  | no plumbing — locked-on switch, guarantee display            |
| Editor · Tool when editor opens | readonly this effort (current default, Pencil)             |
| Editor · Show statusbar      | **new** `ui.show-statusbar` (gates Task 2's statusbar)       |
| Editor · Byte-size estimate  | **new** `ui.statusbar-size-mode` (heuristic \| exact-png)    |
| Editor · Spring animations   | **new** `ui.animations` (gates `theme::ease` microinteractions) |
| Output · Format              | **new** `output.format` (png \| jpeg)                        |
| Output · JPEG quality        | **new** `output.jpeg-quality` (80/90/100 wells)              |
| Output · Filename template   | **new** `output.filename-template` (`{app} {date} {time}`)   |
| Output · Copy after capture  | existing `capture.after-grab` (clipboard value)              |
| Output · Save location       | **new** `output.save-dir` (Choose… directory picker)         |
| Shortcuts                    | readonly display of the real binding table (`hotkeys` config values shown) |
| System · Display server      | readonly, detected at startup                                |
| System · Version             | readonly, `CARGO_PKG_VERSION`                                |

Settings changes **apply live and persist** through the existing
config save + watcher round-trip; `Save`/`Open`/editor honor the new
fields (direct-save path when `filename-template` is set; JPEG encode
+ quality in the save pipeline; statusbar/animations gates).

## Commands

From repo root (unchanged from AGENTS.md):

```sh
cargo build                                    # build default members
cargo fmt --all --check                        # format check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets           # unit + integration
cargo test --doc -p flintshot-core -p flintshot-editor-egui  # doc tests
cargo run -- app                               # home/editor/settings for manual check
```

## Project Structure

```
crates/flintshot-editor-egui/src/
├── theme/palette.rs    # palette-of-record refresh (values above)
├── theme/mod.rs        # ramp/radii/shadow-level tokens; animations gate
├── widgets.rs          # NEW — shared cutout controls: ghost button,
│                       #   well rows, switch, section (chevron+spring),
│                       #   kbd chip — consumed by panel/topbar/settings
├── topbar.rs           # NEW — top bar: brand, breadcrumb, hero actions,
│                       #   overflow menu (route-aware contents)
├── rail.rs             # NEW — left tool rail (tool vocabulary moves here)
├── statusbar.rs        # NEW — footer: dims+size, hint, kbd chips
├── panel.rs            # SLIMS — sections-only properties panel
├── settings.rs         # NEW — settings route page
├── shell.rs / window.rs# routing: Route::Settings; geometry seams
├── home.rs             # topbar integration + welcome re-skin (forest
│                       #   hero card, three hcards, recents grid w/
│                       #   relative times)
├── floating.rs         # restyle + rearrange (undo/redo TL, magnifier BR)
├── app.rs              # zone composition, action routing, geometry
└── header.rs           # RETIRED (deleted)
crates/flintshot-core/src/
└── config.rs           # additive fields: capture.default-mode,
                        # ui.{show-statusbar,statusbar-size-mode,animations},
                        # output.{format,jpeg-quality,filename-template,save-dir}
docs/specs/ux-redesign/
├── spec.md             # this file
├── reference/          # design of record mockups (zone-map, settings)
└── ...
```

## Code Style

Repo conventions (AGENTS.md) apply unchanged. Token-consumption rule:
all `Color32` literals live in `theme/palette.rs`; all controls come
from `widgets.rs`; zone modules compose them:

```rust
// GOOD — zone chrome built from tokened widgets
widgets::ghost_button(ui, id, Glyph::Capture, "Capture", widgets::Ghost::HeroKey, enabled);

// BAD — ad-hoc styling at a zone site
ui.painter().rect(fill, …, Color32::from_rgb(0xE4, 0xB5, 0x4A), …)
```

## Testing Strategy

- **Palette/tokens**: distinctness tests, ramp/radii/shadow-level
  assertions, token-scan test (`rg "Color32::from_"` matches only
  `palette.rs` — existing suite).
- **Widgets**: headless `Context` tests per widget (ghost enable/
  disable/hero-key variants, switch toggle + locked, section collapse
  state, well selection).
- **Topbar**: per-route enablement matrix (Home/Editor/Settings/
  Loading × Capture/Copy/Open/overflow contents).
- **Panel**: binding section map per tool; every section control maps
  to a real field; Delete gating.
- **Settings/config**: serde round-trip + defaults (new fields absent
  → defaults, old configs load unchanged), live-apply semantics,
  readonly rows render values.
- **Statusbar**: formatters (dims, humanized size heuristic vs exact),
  hint = `tool_hint`.
- **Regression**: shortcuts, routing, overlay, recents tests pass;
  `no_gui_types_in_core.rs` untouched and passing.
- **Manual**: `cargo run -- app` — five-zone editor, home, settings;
  side-by-side with `reference/*.html`; full gate (fmt → clippy →
  workspace tests → doc tests) before done.

## Boundaries

- **Always**: consume tokens (no literals outside `palette.rs`) and
  widgets (no ad-hoc controls); keep every current shortcut working;
  keep all ten tools reachable; keep Save/Upload/Paste/diagnostics/
  about/settings reachable (overflow); every rendered control wired to
  real behavior (readonly rows visually distinct); additive config
  fields kebab-case with serde defaults (old configs load unchanged);
  run the full gate; update CHANGELOG under `## [Unreleased]`.
- **Ask first**: deviating from the reference palette/ramp/dimensions;
  new dependencies; non-additive `flintshot-core` changes; window/
  close/capture choreography changes (ADR-0047); making any readonly
  row editable.
- **Never**: render AI surfaces; fake controls; roadmap features
  (working tabs, grouping/multi-select, hotkey *editing* in settings —
  display only); grain/shadows over the canvas image; remove the
  no-Flintshot-in-the-shot guarantee; `unsafe`.

## Success Criteria

1. Editor renders the five zones in the reference arrangement with
   reference dimensions (44/48/236/30); side-by-side with
   `reference/zone-map.html` reads as the same design.
2. Topbar per route: brand + breadcrumb + Capture/Copy/Open (+ `⋯`);
   Capture+Copy `hero-key`; Copy disabled off-editor with the mockup
   tooltip; overflow contents per zone contract; Settings entry present.
3. Rail: ten real tools, five pod groups with separators, gold active
   + spring pop, lock-grey gated tools; shortcuts unchanged.
4. Properties panel: `{Tool} · Properties` title; binding section map
   honored; collapsible with spring; every control maps to a real
   field; Delete in Selection section.
5. Floating chips restyled + rearranged (undo/redo TL, magnifier BR);
   magnifier/loupe behavior unchanged.
6. Statusbar: real dims + size (heuristic by default, exact-png when
   configured), gold per-tool hint, real kbd chips; hidden when
   `ui.show-statusbar` is false; absent on home/settings.
7. Home: topbar (Copy disabled) + re-skinned welcome (forest hero card,
   three hcards, recents grid with relative times); no statusbar.
8. Settings route: reachable from `⋯`; all mapping-table rows render;
   every editable row persists + applies live; readonly rows distinct;
   config round-trip keeps old TOML loading.
9. Palette/type/radii/shadow tokens match the reference table; token
   scan (`rg "Color32::from_"`) still matches only `palette.rs`; style
   applied centrally.
10. Existing tests pass with updated routing; new component/config
    tests green; clippy `-D warnings` clean; fmt clean.
11. CHANGELOG updated under `## [Unreleased]`; ADR records the
    five-zone rework + settings route.

## Open Questions

- `capture.after-grab` value vocabulary — confirm the exact
  clipboard-value string during build (settings switch writes it).
- Recents relative-time formatting thresholds (<1m "now", minutes,
  hours, "yday", dates) — tuned in build.
- Direct-save path: exact precedence when `output.save-dir` is set but
  `filename-template` is empty (default: dialog) — confirm in build.
