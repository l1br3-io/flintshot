---
status: shipped
started: 2026-09-04
shipped: 2026-09-04
---

# Spec — Prominent move tool: make repositioning discoverable

> Mini-effort spec. The move semantics shipped in
> [`../annotation-move/`](../annotation-move/spec.md); this effort is
> about making users *find* them. For product context, see
> [`../../VISION.md`](../../VISION.md).

## Objective

`Tool::Move` works but hides: it sits second in the rail behind a
glyph-only chip, the editor opens on `Rect`, and once you've drawn
something the fastest remedies for a bad placement are undo or
redraw. Users coming from any Flameshot-class editor expect
"draw, then nudge it" as one gesture chain.

Four measures, together making Move the resting state of the editor:

1. **Flip to Move after every commit (config-gated)** — releasing
   a drawn annotation (Rect / Ellipse / Line / Arrow / Marker /
   Pencil / Blur, and Text on modal commit) switches the tool to
   Move, so the very next press can reposition what was just drawn.
   Re-press the tool's hotkey (`R`, `E`, `A`, …) to draw again.
   Controlled by `[ui] flip-to-move` (default `true`); setting it
   to `false` restores sticky tools for rapid multi-draw.
2. **Move is the default edit-phase tool** — a plain editor window
   (and the edit phase after a region is confirmed) opens on Move,
   not Rect.
3. **Rail emphasis** — Move becomes the first chip and renders as a
   labeled chip (glyph + "Move" text, the Copy-primary's
   glyph+label pattern), in both the editor toolbar and the
   overlay's floating rail.
4. **Hover affordances** — under Move, hovering a committed
   annotation shows the `Move` cursor and a thin dashed bounding
   box (via `Annotation::bounding_box`, whose doc anticipated
   exactly this) in **both** shells; the editor window gains hover
   cursors at all.

**Success looks like:** draw a rect, release, drag it better — no
tool switch, no undo. A first-time user sees "Move" written in the
rail and, on hovering any annotation, gets a cursor + outline that
say "grab me".

## Assumptions

1. **Number is exempt from the flip.** Its input model is
   click-to-place-the-next-number; flipping would break rapid
   sequential labeling. Same for `Select` and `Move` itself.
2. **The flip is not history.** Undo restores annotation state,
   never the active tool. Flipping creates no undo entry; hovering
   never touches the dispatcher.
3. **The region-selection phase keeps its contract.** The capture
   flow still opens on `Select` (region-first, per
   `region-selection-first`); Move-default applies only at
   edit-phase entry / plain editor open.
4. **Flip is config-gated, default on** —
   `UiConfig::flip_to_move: bool` (`[ui] flip-to-move`,
   `#[serde(default = "default_flip_to_move")]` → `true`).
   Off = sticky tools, the pre-effort behavior. The flag plumbs
   into `EditorState` (a plain policy bool, not a config type) at
   `state_from_config` (cli.rs) — the single production
   construction site both shells share — following the magnifier
   precedent of read-once-per-editor-session. The daemon's config
   watcher is hotkeys-only and cross-process from the editor
   worker, so no live reload: a toggled flag applies to the next
   capture's editor session.
5. Degenerate commits (discarded zero-extent strokes) do not flip —
   flip only when an annotation actually lands in `anns`.

## Scope (in / out)

**In:**

- `flintshot-core`: `Tool`'s `#[default]` moves to `Move`; flip in
  the commit paths (`finish_pointer_up`, `commit_text_draft`);
  `UiConfig::flip_to_move` (+ `merge_into_existing` /
  `Default` plumbing); doc-comment updates.
- `flintshot-app`: `state_from_config` (cli.rs) copies
  `ui.flip_to_move` into `EditorState` — one site, both shells.
- `flintshot-editor-egui`: rail reorder (Move first) + labeled Move
  chip in `toolbar.rs` `TOOLS`/rail and the overlay's floating
  rail; editor-window hover cursor; hover bbox chrome in both
  shells' paint paths.
- Tests at every layer; `CHANGELOG.md` + `docs/TEST_MATRIX.md`
  rows; one ADR for the flip policy.

**Out:**

- Click-to-select chrome actions (delete, resize handles, z-order),
  arrow-key nudge, snapping.
- Config for the default tool or any other new setting — only
  `flip-to-move` joins the schema this effort.
- Overlay gesture-routing changes (annotation fall-through already
  shipped); the text-editing modal path beyond its commit flip.
- Editor-window hover cursors for tools other than Move (scope
  creep; only the Move grab cursor is on this effort's critical
  path).

## Deliverables

1. `crates/flintshot-core/src/editor.rs` — default flip + commit
   flips (gated on the policy bool), with unit tests.
2. `crates/flintshot-core/src/config.rs` — `flip_to_move` field,
   serde default, `merge_into_existing` entry, roundtrip tests.
3. `crates/flintshot-app/src/` — construction + watcher plumbing
   from `UiConfig` into `EditorState`.
4. `crates/flintshot-editor-egui/src/{toolbar,app,overlay,theme}.rs`
   — rail/chip changes, hover cursor + bbox chrome, with unit
   tests.
5. `docs/adrs/ADR-0040-flip-to-move-after-commit.md` + CHANGELOG +
   TEST_MATRIX entries.
6. This spec (+ plan/tasks via `/planning`).

## Exit criteria

1. `cargo run -- region` → drag a region → confirm → edit phase
   opens on **Move** (first, labeled chip is ember).
2. Press `R`, draw a rect, release → tool is Move; press-drag the
   rect repositions it; `Ctrl+Z` restores position while the tool
   stays Move.
3. `N` placements keep the Number tool active across clicks; `S`
   region selection is untouched.
4. Set `[ui] flip-to-move = false`, take a new capture → drawing
   commits keep the tool active (sticky, pre-effort behavior);
   re-enabling restores the flip on the next editor session
   (per-session read, matching the magnifier config).
5. Hovering an annotation under Move shows the Move cursor + dashed
   bbox in both the editor window and the overlay; hovering empty
   canvas shows neither; no hover produces history entries.
6. Undo/redo/crop still cancel in-flight state exactly as before;
   discarding a degenerate stroke does not flip the tool.
7. Standing gates green (`fmt --check`,
   `clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace --all-targets`, doc tests for the two
   lib crates).

## Technologies

No new dependencies. Policy lives in `flintshot-core` (both shells
inherit it); chrome is painter strokes in `flintshot-editor-egui`
reusing the existing selection-stroke visual language.

## Commands

```sh
cargo test -p flintshot-core                 # flip-policy tests
cargo test -p flintshot-editor-egui          # rail + hover tests
# standing gate (AGENTS.md)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
```

## Project structure

| Change                      | Location                                    |
| --------------------------- | ------------------------------------------- |
| Default + commit flips      | `crates/flintshot-core/src/editor.rs`       |
| `flip_to_move` config field | `crates/flintshot-core/src/config.rs`       |
| Config → state plumbing     | `crates/flintshot-app/src/cli.rs` (`state_from_config`) |
| Rail order, labeled chip    | `crates/flintshot-editor-egui/src/toolbar.rs` |
| Overlay rail + hover chrome | `crates/flintshot-editor-egui/src/overlay.rs` |
| Editor hover cursor + chrome| `crates/flintshot-editor-egui/src/app.rs`   |
| Visual constants            | `crates/flintshot-editor-egui/src/theme.rs` |
| Docs                        | `CHANGELOG.md`, `docs/TEST_MATRIX.md`, `docs/adrs/` |

## Code style

Per [`AGENTS.md`](../../../AGENTS.md): `#[must_use]` on pure
queries, doc comments on new public items, no `unwrap()` /
`expect()` outside tests, no GUI types in core, painter-drawn
glyphs only (no emoji, no icon fonts).

## Testing strategy

- **Core unit tests** (alongside dispatcher tests): flip on commit
  per eligible tool when the policy bool is on; no flip when off;
  Number / Select / Move exempt; degenerate commit does not flip;
  flip adds no undo entry; undo after flip leaves tool on Move;
  `Tool::default() == Move`.
- **Config unit tests** (`config.rs` test mod, following the
  magnifier/pin patterns): default `true` when absent; roundtrip
  through `save_to` / `load_from`; `merge_into_existing` preserves
  a user comment on the key.
- **Toolbar unit tests**: Move is rail-first; every variant still
  has a chip with a unique key; labeled-chip helper yields width ≥
  glyph-only chip.
- **Overlay / app unit tests**: hover-hit helper returns the right
  annotation index; chrome rect derives from `bounding_box`; cursor
  mapping per hover state.
- **Manual**: exit criteria 1–5 via `cargo run -- region` and the
  plain editor window.

## Boundaries

- **Always:** flip policy lives in core only, gated on a single
  policy bool (no shell-side duplication); hover paths are
  read-only queries; standing gate before declaring done.
- **Ask first:** new public API in core beyond the policy bool and
  re-using `bounding_box` / `hit_test`; any change to the
  region-selection phase's `Select` entry; new theme colors; any
  config field beyond `flip-to-move`.
- **Never:** history entries for tool transitions or hovers;
  emoji/icon-font glyphs; config types leaking into `EditorState`.

## Success criteria

- Draw-then-adjust reads as one flow, not two modes.
- The rail teaches the tool's existence without a tooltip.
- Zero dispatcher behavior change for anything that is not a
  commit, a hover, or the default.

## Decisions

- **Config-gated flip, default on** — the flip is the effort's
  point, so it ships enabled; a `[ui] flip-to-move = false`
  escape hatch keeps rapid multi-draw workflows (re-pressing the
  hotkey is already the fastest draw path either way). A plain
  bool on `EditorState` — not a config type — keeps the
  dispatcher testable and core free of config coupling.
- **Number exempt** — rapid sequential placement is its reason to
  exist; a flip per bubble would be hostile.
- **Tool state stays out of history** — undo semantics remain
  purely about annotations; mixing tool transitions in would make
  `Ctrl+Z` feel like it undoes the wrong thing.
- **Labeled chip, not a bigger glyph** — text is the only glyph
  replacement that survives font-constrained systems and locale
  differences; the Copy primary already establishes the
  glyph+label chip pattern.
- **Hover chrome from `bounding_box`** — the doc comment on
  `Annotation::bounding_box` has promised this use since Phase 1;
  dashed 1px in the existing selection-stroke language keeps it
  subordinate to real selection chrome.

## Open questions

1. Should confirming a crop (`Select` + Enter) also flip to Move?
   **Leaning no** — crop is usually terminal (Save / Copy / Pin
   follows); flipping there would churn the tool for no gesture to
   place. Revisit if dogfooding disagrees.
