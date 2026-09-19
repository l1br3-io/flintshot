---
status: active
started: 2026-09-19
---

# Spec: Select / Crop / Hand tool refactor

## Objective

The editor's tool palette conflates three distinct jobs. Today `Tool::Select`
is a region-crop tool and `Tool::Move` is an annotation select-and-move tool.
This effort splits the jobs properly, matching the mental model of every
mainstream image editor:

1. **`Tool::Select` becomes an object-selection tool** — click selects an
   annotation, Shift/Ctrl+click toggles membership, drag on empty space draws
   a marquee that selects every intersecting annotation, and dragging a
   selected annotation moves the whole selection set.
2. **`Tool::Crop` is new** — inherits the old Select behavior exactly: drag a
   region, Enter or the Crop button crops the image to it, and Copy honors
   the active region.
3. **`Tool::Move` is renamed `Tool::Hand`** and becomes a view-pan tool —
   plain drag pans the image when zoomed in, the "hand" of photo editors.
   Pan-only by design (mainstream contract: selection and moving objects
   belong to Select alone); Hand never touches annotation state.
4. **Pan/zoom shortcuts work from any tool**: mouse wheel zooms centered on
   the cursor, middle-mouse drag pans.

Who it's for: anyone annotating a screenshot — particularly zoomed-in work
(pixel-precise arrows, text, blur), where the absence of panning today makes
the 0.5–4.0× view zoom nearly unusable.

### User-visible acceptance criteria

- With Select active: click on an annotation selects it; drag on empty space
  rubber-band-selects all annotations whose bounding boxes intersect the
  marquee; drag on a selected annotation translates the selection set;
  Shift/Ctrl+click toggles membership; Del deletes the set; the inspector
  panel reflects the selection.
- With Crop active: drag draws the region; Enter or "Crop · Enter" crops;
  Copy copies just the region while one is active. Byte-identical with
  today's Select behavior.
- With Hand active: drag pans the view. Hand never selects, moves, or
  otherwise touches annotations (mainstream hand-tool contract).
- From any tool: wheel zoom keeps the image point under the cursor fixed;
  middle-drag pans; the Hand tool, wheel, and middle-drag all clamp pan so
  the image never leaves the viewport entirely.
- The fullscreen capture overlay and the editor's selection-first phase
  behave exactly as before, now pre-arming Crop instead of Select.
- Hotkeys: V = Select, H = Hand, C = Crop (S is freed).
- Committing an annotation with `flip-to-select = true` flips the tool to
  Select.

## Technologies

Pure-Rust Cargo workspace, unchanged: `flintshot-core` (state machine,
toolkit-free), `flintshot-editor-egui` (egui shell), `flintshot-app` (CLI +
daemon). No new dependencies. egui provides wheel/middle-button events
already; view math stays in `CanvasView` / `EguiCanvas`.

## Commands

```sh
cargo build                                  # default members
cargo fmt --all --check                      # format check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets         # unit + integration
cargo test --doc -p flintshot-core -p flintshot-editor-egui
cargo run -- capture                         # manual: overlay flow (Crop pre-arm)
cargo run -- app                             # manual: editor + pan/zoom/hand
```

## Project Structure

| Change                                   | Where                                                        |
| ---------------------------------------- | ------------------------------------------------------------ |
| `Tool` enum: add `Crop`, rename `Move`→`Hand`, new Select semantics | `crates/flintshot-core/src/editor.rs` |
| Crop-region state: rename `selection` field → `crop_region`, `confirm_selection` → `confirm_crop` | `flintshot-core/src/editor.rs` (ripples to `render.rs`, `inspector.rs`, `shell.rs` docs) |
| Marquee state + routing: `marquee: Option<Rect>`, marquee release → bbox-intersection merge into `selected` | `flintshot-core/src/editor.rs` |
| Selection-owning tools become `Select \| Text \| Hand` (`set_tool` clearing, dispatcher branches) | `flintshot-core/src/editor.rs` |
| Config rename `flip-to-move` → `flip-to-select` (+ legacy-key migration) | `flintshot-core/src/config.rs` |
| Rail pods: selection pod = `Select, Hand, Crop`; hotkeys V/H/C; crop glyph | `flintshot-editor-egui/src/rail.rs` |
| `ToolGate::SelectOnly` → `ToolGate::CropOnly`; lock clause copy | `flintshot-editor-egui/src/rail.rs`, `app.rs` |
| Pan state: `pan_offset` on `EditorApp`, pan-clamp + zoom-to-cursor math as pure fns | `flintshot-editor-egui/src/app.rs` (+ pure helpers in `canvas.rs` if they belong there) |
| Wheel/middle-drag + Hand-plain-drag pan routing; Hand modifier drags → dispatcher | `flintshot-editor-egui/src/app.rs` (`logic`) |
| Overlay pre-arm + hotkeys + press-allowlists: Select→Crop | `flintshot-editor-egui/src/overlay.rs` |
| Selection-first phase: pre-arm Crop, latch flips to Select | `flintshot-editor-egui/src/app.rs`, `shell.rs` |
| Panel sections per tool (`Crop` gets Crop section; Select/Hand get Selection) | `flintshot-editor-egui/src/panel.rs` |
| Chrome: selected-outline + marquee + crop-region rendering updates | `flintshot-editor-egui/src/{chrome,app}.rs`, `flintshot-core/src/render.rs` |
| Statusline hints per tool | `flintshot-editor-egui/src/panel.rs` |
| CLI help text mentioning the region tool | `flintshot-app/src/cli.rs` (docs only) |
| CHANGELOG + ADR (tool semantics remap)   | `CHANGELOG.md`, `docs/adrs/`                                 |

## Code Style

Repo conventions (AGENTS.md) apply: `#![forbid(unsafe_code)]`,
`#![warn(clippy::pedantic)]`, `#![warn(missing_docs)]` in core, thiserror in
libs, no `unwrap()`/`expect()` outside tests, doc comments on every public
item, `#[non_exhaustive]` where plausible, kebab-case serde fields, comments
only for non-obvious "why". Example of the dispatch routing shape this
effort follows (existing style, `editor.rs`):

```rust
InputEvent::PointerDown { pos, modifiers } => {
    if self.tool == Tool::Crop {
        self.begin_crop_region(*pos);
    } else if self.tool == Tool::Select {
        self.route_select_press(*pos, modifiers);
    } else if self.tool == Tool::Hand {
        // Pan-only contract: the shell owns panning and never
        // dispatches Hand presses; core no-ops them defensively
        // so synthetic state can't mutate annotations.
    }
    DispatchOutcome::Continue
}
```

Key naming decisions: `Tool::Hand` (not `Pan` — the user-facing idiom),
`crop_region` (the field formerly known as `selection`), `marquee` (the
in-flight Select-tool rubber band), `pan_offset` (shell view state).

## Testing Strategy

Same levels as the repo today; every behavior change lands with tests in the
same file per repo practice:

- **Core unit tests** (`editor.rs` `mod tests`): Select click/toggle/marquee/
  drag-move routing; marquee bbox-intersection merge; Hand modifier routing
  and plain-press no-op; Crop tool region lifecycle is the old Select test
  set renamed; `set_tool` selection clearing across the new owning triple;
  `flip_to_select` commit flip + ownership guard.
- **Config tests** (`config.rs`): `flip-to-select` roundtrip; legacy
  `flip-to-move` key silently migrates; comment preservation on renamed key.
- **egui shell tests** (`app.rs`, `rail.rs`, `panel.rs`, `overlay.rs`,
  `canvas.rs`): rail pod partition + hotkey map; `CropOnly` gate; wheel
  zoom-to-cursor math (pure fn: new pan keeps cursor anchor fixed); pan
  clamping at image edges; pan resets at fit zoom; overlay pre-arms Crop;
  selection-first latch flips to Select; Copy-with-region follows
  `crop_region`.
- **Verification**: the full AGENTS.md finish sequence (fmt, clippy
  `-D warnings`, workspace tests, doc tests).

## Boundaries

- **Always**: keep `flintshot-core` GUI-toolkit-free (the
  `no_gui_types_in_core.rs` test must stay green); route every input through
  `dispatch_input`; keep pan as shell-owned view state (never in
  `EditorState`); preserve overlay/selection-first UX byte-for-byte from the
  user's perspective; update CHANGELOG under `## [Unreleased]`; add an ADR
  for the tool remap and the config migration table.
- **Ask first**: any new dependency; changing `EditorOutcome` /
  `EditorShell` trait signatures; touching the capture backends or IPC;
  renaming any public API beyond the ones listed above; changing CI config.
- **Never**: `unsafe`; `unwrap()`/`expect()` outside tests; silencing
  clippy without a justifying comment; GUI types in core's public API;
  changing upload/capture behavior in this effort.

## Success Criteria

1. `cargo test --workspace --all-targets` + doc tests green; clippy
   `-D warnings` clean.
2. Every acceptance criterion in the Objective section demonstrably works
   in `cargo run -- app` (manual checklist) and is covered by an automated
   test where input-level (core dispatcher, config, rail, view math).
3. No public API still references the old meanings: `grep -r "flip.to.move"`
   finds only the migration code; `Tool::Move` no longer exists.
4. Overlay (`cargo run -- capture` on Wayland portal path or fallback) opens
   with Crop pre-armed and crops on Enter exactly as before.
5. An ADR records the tool-semantics remap (including the config key
   migration: old `flip-to-move` values map to `flip-to-select`).

## Open Questions

1. **Wheel zoom granularity** — spec assumes the same discrete 0.25 steps
   as the floating +/− chips (keeps `at_fit_zoom` == 1.0 exactness and the
   worker resize policy sound). Switch to continuous factors instead?
2. **Marquee modifier semantics** — Shift+marquee and Ctrl+marquee: add to
   the existing selection set (spec assumes yes, mirroring click toggling
   being additive) or always replace?
3. **Crop tool + selection-first hint copy** — the hint line and lock clause
   ("locked — Enter crops to unlock") may need rewording for a world where
   the locked tool is named Crop. Spec assumes minimal rewording only.

**Decision log**: Hand originally carried Shift/Ctrl+drag as a secondary
select-and-move path (user request); dropped in favor of the mainstream
pan-only contract — selection lives on Select alone, and the shell never
dispatches Hand presses to core.
