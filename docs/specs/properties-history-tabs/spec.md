---
status: active
started: 2026-09-20
---

# Spec — Properties | History tabs in the editor panel

> Converts the docked properties panel into a two-tab surface
> (PROPERTIES | HISTORY) and gives the History tab a labeled,
> time-travelable edit timeline. Follow-on to
> `multi-select-properties`, which owns the panel's current
> inspector-mode contract. For product context see
> [`../../VISION.md`](../../VISION.md).

## Objective

Today the editor's undo history is invisible: `EditorState.undo` /
`redo` are bare snapshot stacks with no notion of *what* each entry
was, and the user's only windows into them are the floating
undo/redo chips and the `Ctrl+Z` / `Ctrl+Shift+Z` chords — one step
at a time, no overview.

Two linked changes fix this:

1. **Labeled history in core** — every undo push site records an
   `EditKind` on the snapshot (`Added arrow`, `Moved 2 items`,
   `Cropped`, `Edited colour`, …), and a new `restore_to` mutation
   jumps the undo/redo boundary to any position in one call.
2. **Tabbed panel in the shell** — the docked panel grows a tab
   header (PROPERTIES | HISTORY). The Properties tab is
   byte-for-byte today's panel (tool defaults / selection
   inspector). The History tab renders the timeline: past entries
   (click to jump back), the current-state marker, and dimmed
   future entries (click to jump forward).

**Success looks like:** draw a rect, place two numbers, crop, recolor
one number via the inspector — the History tab lists "Cropped",
"Added number ×2", "Added rect" with the current state at the
bottom; click "Added rect" and the canvas returns to that moment in
one action; click back down the list to redo forward. The floating
undo/redo chips and key chords keep working unchanged.

**User**: the existing Linux screenshot user annotating in bursts;
no new entry surface.

## Assumptions (validated in clarification)

1. "History" means the **edit history** (undo stack) — not the
   persistent recent-captures index (`recents.rs` stays home-only).
2. The History tab is **interactive time-travel**: clicking any
   entry restores that state; current position is marked.
3. Entries carry **human-readable labels** — this is core work
   (snapshots today have no kind).
4. The floating undo/redo cluster **stays**; the tab is additive.
5. Scope is the docked editor panel only — the `overlay.rs`
   floating-toolbar variant and the home page are untouched.

## Technologies

No new dependencies. `flintshot-core` (pure logic, no GUI types),
`flintshot-editor-egui` (immediate-mode UI). Existing crates'
versions per workspace `Cargo.toml`.

## Commands

From the repo root (per `AGENTS.md`):

```sh
cargo build                              # default-members
cargo test --workspace --all-targets     # unit + integration
cargo test --doc -p flintshot-core -p flintshot-editor-egui
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -- app                         # manual: editor → History tab
```

## Project Structure

| Change                                   | Where                                                        |
| ---------------------------------------- | ------------------------------------------------------------ |
| `EditKind` enum + `Display`              | `crates/flintshot-core/src/history.rs` (new module)          |
| `Snapshot.kind` field + constructors     | `crates/flintshot-core/src/editor.rs`                        |
| Label wiring at all push sites           | `editor.rs` (commit, place_number, move, delete, crop, text-commit, `end_style_gesture`) + `inspector.rs` (property bursts) |
| Label mirroring in `undo`/`redo` swap    | `editor.rs::undo` / `::redo`                                 |
| `restore_to(target_undo_len)` mutation   | `crates/flintshot-core/src/editor.rs`                        |
| Re-export `EditKind`                     | `crates/flintshot-core/src/lib.rs`                           |
| Tab state + header + timeline rendering  | `crates/flintshot-editor-egui/src/panel.rs`                  |
| Panel-call site (if signature changes)   | `crates/flintshot-editor-egui/src/app.rs`                    |
| Tests                                    | Inline `#[cfg(test)]` modules beside the code (repo idiom)   |
| This spec                                | `docs/specs/properties-history-tabs/`                         |

## Design

### Core: `EditKind`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, non_exhaustive)]
pub enum EditKind {
    Add { tool: Tool },
    AddNumber,
    Move { count: usize },
    Delete { count: usize },
    Crop,
    EditText,
    RestyleText,
    Property { kind: PropertyEditKind },
    /// `restore_to` and any future non-edit push; renders as "Edit".
    Generic,
}
```

- `Display` yields the row label: `Added arrow`, `Added number`,
  `Moved 3 items` (singular `item` at 1), `Deleted 2 items`,
  `Cropped`, `Edited text`, `Text style`, and for properties the
  `PropertyEditKind` name (`Colour`, `Stroke width`, `Fill`,
  `Text style`, `Blur`, `Number size`).
- `Snapshot` gains `kind: EditKind`; `anns_only` / `full` take it
  as a parameter. Every push site passes its kind; the ~10 test
  push sites pass `EditKind::Generic`.
- **Undo/redo label mirroring**: when `undo()` moves the popped
  entry's state aside and pushes `current` onto `redo`, the redo
  entry **keeps the popped entry's kind** — the same edit viewed
  from the other side. Same for `redo()`.
- `restore_to(&mut self, target_undo_len: usize)`: pops
  undo→redo / redo→undo until `undo.len() == target_undo_len`,
  clamping out-of-range targets. It reuses the exact swap
  semantics of `undo()` / `redo()` (pending-drop, selection-clear,
  image handling), so time-travel and chord-stepping can never
  diverge. One undo/redo **cycle** per call site remains true:
  `restore_to` is many pops, but each pop is exactly one existing
  entry move.

### Shell: the tabbed panel

- `PanelTab { Properties, History }` on `PanelState` (persists
  across frames; default Properties; per-window like the collapse
  flag, not persisted to disk).
- The tab header is the panel's first row: two `segmented_chip`
  glyphs — `PROPERTIES` | `HISTORY` — cutout at rest,
  ink-filled when active (the blur-kind control's existing
  idiom). It sits above where the contextual title lives today.
- **Properties tab**: unchanged content — the contextual title
  (`N selected · Properties` / `{Tool} · Properties`) renders
  beneath the header, then exactly `sections_for(tool)` or the
  inspector sections.
- **History tab**: no contextual title; the body is the timeline —
  - oldest entry at top, **current-state marker row** near the
    bottom, dimmed future (redo) entries below it — the
    Photoshop ordering.
  - Past row `i` (0 = oldest): label + click →
    `state.restore_to(i + 1)`.
  - Current row: an ink-dot marker + "Current" — not clickable.
  - Future rows: dimmed (half-alpha ink) but clickable →
    `restore_to` forward to that boundary.
  - Empty state (no undo, no redo): a quiet hint line ("No edits
    yet"), mirroring the Crop section's hint styling.
- The bottom of the panel is untouched: `show()` renders title +
  sections only today (the module doc's "bottom cluster" text is
  stale — the Copy hero lives on the topbar, the history chips on
  the floating cluster; fixing that stale doc is part of the work).
- Collapse rail, grain, panel width: unchanged.
- `panel_title` (pure, tested) stays; the tab header is additive.

## Code Style

Repo conventions apply (`AGENTS.md`, `.opencode/tech/rust/`);
representative shape of the new core code:

```rust
impl Snapshot {
    pub fn anns_only(kind: EditKind, anns: Vec<Annotation>, next_number: u32) -> Self {
        Self { kind, anns, image: None, next_number }
    }
}

impl EditorState {
    /// Jump the undo/redo boundary to `target_undo_len` entries
    /// remaining on `undo`. Clamped; a no-op when already there.
    pub fn restore_to(&mut self, target_undo_len: usize) {
        let target = target_undo_len.min(self.undo.len() + self.redo.len());
        while self.undo.len() > target { self.undo(); }
        while self.undo.len() < target { self.redo(); }
    }
}
```

- `#![forbid(unsafe_code)]`, `missing_docs` on new public core
  items, `#[non_exhaustive]` on `EditKind`, no `unwrap` outside
  tests, comments only for non-obvious *why*.

## Testing Strategy

- **Core (inline `#[cfg(test)]`)**:
  - every push site records the expected kind (one test per site:
    commit/move/delete/crop/number/text-commit/restyle/property
    burst);
  - label mirroring across an undo→redo round trip;
  - `restore_to`: jump back N, forward M, out-of-range clamp,
    no-op at current position, empty-stacks case;
  - burst coalescing still yields one entry (regression guard).
- **Shell (inline tests using the `Context::run_ui` + `RawInput`
  idiom from `floating.rs`)**:
  - tab header registers both chips; clicking swaps the rendered
    body (Properties sections vs timeline rows);
  - timeline row ids (`("history-row", i)` + future marker)
    register with the right counts after a scripted edit
    sequence;
  - clicking a past row calls through to `restore_to` (assert
    `state.undo.len()` after the click pass);
  - Properties-tab content is byte-identical today (existing
    `panel_title` / section tests keep passing unmodified).
- Manual: `cargo run -- app` → annotate → crop → History tab →
  click around the timeline.

## Boundaries

- **Always**: keep GUI types out of core (the
  `no_gui_types_in_core.rs` test must keep passing); run the four
  verification commands before declaring done; update
  `CHANGELOG.md` under `## [Unreleased]`.
- **Ask first**: changing `PANEL_WIDTH`; touching `overlay.rs` or
  the floating cluster; new dependencies; changes to the
  undo/redo swap semantics beyond carrying the label.
- **Never**: remove or degrade the floating undo/redo chips or
  key chords; persist panel tab state to disk; use `unsafe` or
  `unwrap()` outside tests; let the History tab mutate state
  through any path other than `restore_to` / existing mutators.

## Success Criteria

1. Panel shows a PROPERTIES | HISTORY tab header; Properties tab
   content and behavior are unchanged (all existing panel tests
   pass unmodified).
2. Every undo entry created by any gesture carries a
   human-readable label; a scripted mixed session (draw, move,
   delete, crop, recolor, text edit) shows one labeled row per
   entry, oldest first, current-state marker at the boundary.
3. Clicking a past row restores exactly that state in one action;
   clicking a dimmed future row redoes forward; chips and
   chords still work and the timeline reflects them live.
4. `restore_to` round-trips: jumping back then forward to the
   end reproduces the pre-jump annotation set exactly.
5. `cargo fmt --all --check`, `cargo clippy --workspace
   --all-targets -- -D warnings`, and the full test command set
   pass.

## Open Questions

None — both planning-time questions resolved by proposal (accepted
by silence at plan review): oldest-first-top timeline order, and a
plain "Current" marker row (forward edits are already named by the
dimmed redo rows).
