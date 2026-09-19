# ADR-0064: The Select / Crop / Hand tool remap

**Context.** The editor's tool palette carried three distinct jobs
on two tools: `Tool::Select` was a region-crop tool (drag a
rectangle, `Enter` crops, Copy honors it) while `Tool::Move` was an
annotation select-and-move tool (click selects, Shift+click toggles,
drag moves). Every mainstream image editor (Photoshop, GIMP, Figma,
Excalidraw) splits these jobs differently: a marquee **select**
tool for objects, a dedicated **crop** tool for the region, and a
**hand** tool that pans the view. The conflation also made the
0.5–4.0× view zoom nearly unusable — there was no way to pan a
zoomed-in view at all, which pixel-precise work (arrows, text,
blur) needs.

An earlier draft of this effort carried Shift/Ctrl+drag on Hand as
a secondary select-and-move path (user request). It was dropped in
favor of the mainstream pan-only contract: object manipulation
belongs to Select alone, so selection state can never be touched
from a view tool.

**Decision.**

- **`Tool::Select` is the object-selection tool.** Click selects an
  annotation; Shift **or Ctrl**+click toggles membership (the core
  `Modifiers` carries both; macOS Cmd/Ctrl translation stays the
  shell's job); a drag on empty space draws a marquee that
  bbox-intersection-selects every annotation it touches — plain
  release replaces the set, Shift/Ctrl release merges (union, never
  toggle-out); dragging a selected annotation moves the whole set
  (the old Move routing, `route_select_press` +
  `update_move`, reused verbatim with the modifier widened). The
  selection-owning pair in `set_tool` clearing and the inspector /
  chrome gating is `Select | Text` (Hand is not an owner).
- **`Tool::Crop` inherits the old Select behavior byte-for-byte.**
  `EditorState.selection` renames to `crop_region`
  (`confirm_selection` → `confirm_crop`); the dispatcher's region
  branches, the Enter path, Copy's `compose_selection_aware`, the
  panel's region bay, and the overlay's gesture system all key on
  Crop.
- **`Tool::Hand` (ex-`Tool::Move`) is pan-only.** The shell owns
  panning (`pan` state on `EditorApp`, composed into the canvas's
  `content_offset` as `letterbox + pan`; core never sees it) and
  never dispatches Hand presses; core no-ops Hand pointer events
  defensively so synthetic state can't mutate annotations.
  Middle-drag pans from any tool; the wheel steps the view zoom
  (0.25-quantized, cursor-anchored, clamped, reset at the 1.0 fit
  so the worker resize policy's trigger stays exact).
- **Hotkeys / pre-arms:** V = Select, H = Hand, C = Crop; `S` is
  freed. The default tool is Select; the fullscreen overlay and
  the editor's selection-first phase pre-arm Crop (the phase latch
  flips a still-Crop tool to Select — the edit-phase default).

**Migration table.**

| Old | New | Notes |
| --- | --- | --- |
| `Tool::Move` | `Tool::Hand` | Behavior changed: pan-only (no annotation selection under Hand) |
| `Tool::Select` (region) | `Tool::Crop` | Behavior byte-identical |
| `Tool::Select` (name) | `Tool::Select` (object selection) | New semantics |
| `[ui] flip-to-move` | `[ui] flip-to-select` | Same value; serde alias reads the legacy key, the toml_edit save path rewrites it (key decor / comments ride along) |
| `S` hotkey (Select) | `V` (Select), `C` (Crop) | `S` freed; `H` = Hand (was `V`) |
| Default tool `Hand` | Default tool `Select` | `flip-to-select` now arms Select on commit |

**Consequences.**

- Selection lives on exactly one tool; every other tool (including
  Hand) is stateless with respect to annotations, which keeps the
  `set_tool` clearing invariant (`Select | Text`) and the inspector
  gating simple.
- Pan is shell-owned view state and never enters `EditorState`,
  history, or saves; undo/redo are unaffected by navigation. All
  mapping sites compose it for free because they already read the
  shared `CanvasView` (`content_offset`).
- The public API renames (`selection` → `crop_region`,
  `confirm_selection` → `confirm_crop`, `flip_to_move` →
  `flip_to_select`) are breaking for any shell built against
  `flintshot-core`; there is exactly one shell in-tree and it moved
  with the rename.
- The config migration is silent and one-way: after the first save
  the legacy key is gone. Users who downgrade after upgrading lose
  the knob (it falls back to the default, `false`).
