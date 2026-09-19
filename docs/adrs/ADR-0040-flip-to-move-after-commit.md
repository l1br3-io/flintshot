# ADR-0040: Flip to Move after every annotation commit

**Context.** The Move tool (effort `annotation-move`) shipped fully
functional but hidden: it sat second in the rail behind a glyph-only
chip, the editor opened on `Rect`, and once something was drawn the
fastest remedies for a bad placement were undo or redraw. Users coming
from any Flameshot-class editor expect "draw, then nudge it" as one
gesture chain — a discoverability gap, not a mechanics one.

Two constraints made the policy non-obvious:

- **Text commits can outlive the tool choice.** The editor window's
  toolbar-switch path flushes a pending text draft *after* the user
  has already picked the next tool (chip or hotkey). A naive "flip on
  every commit" would clobber that deliberate choice.
- **The editor is a separate process from the config watcher.** The
  daemon's config hot-reload covers `[hotkeys]` only, and the editor
  runs in a spawned worker — there is no live channel from a config
  edit to an open editor session.

**Decision.**

- `EditorState::commit_annotation` — the single commit funnel every
  eligible tool routes through (pointer-up releases and Text's modal
  commit alike) — switches the active tool to `Tool::Move` when the
  policy is on. `place_number` bypasses the funnel, exempting Number
  naturally (rapid sequential placement is its input model); Select
  and Move produce no pending annotations at all.
- The flip carries an **ownership guard**: it fires only when the
  pending annotation's owning tool still matches the active tool, so
  a draft flushed because the user already switched tools does not
  flip — the explicit choice wins.
- The policy is a **plain `EditorState` bool** (`flip_to_move`,
  default `true`), not a config type — the dispatcher stays testable
  and core stays free of config coupling. It is plumbed from
  `[ui] flip-to-move` (default on) at `state_from_config` (cli.rs),
  the single production construction site both shells share.
- Config is **read once per editor session** (the magnifier
  precedent): a toggled flag applies to the next capture's editor
  session. No cross-process live reload — it would need a new IPC
  channel for zero demonstrated need.
- Tool state stays out of history: the flip creates no undo entry;
  undo after a flip restores annotation state and leaves the tool on
  Move.

**Consequences.**

- Draw-then-adjust reads as one gesture chain: release lands on
  Move, the next press repositions what was just drawn. Re-pressing
  a tool's hotkey (`R`, `E`, …) was already the fastest draw path,
  so the escape hatch (`[ui] flip-to-move = false`, restoring sticky
  tools for rapid multi-draw) costs nothing.
- Degenerate commits (discarded zero-extent strokes) never flip —
  the flip fires only when an annotation actually lands in `anns`.
- The region-selection phase keeps its contract: captures still open
  on `Select` (region-first); the phase-end transition is shell-side
  (`app.rs`'s `end_phase_on_crop` flips `Select` → `Move` when the
  phase latches, unless the user picked a tool mid-phase), not a
  dispatcher concern.
- `Tool::default()` changed to `Move`, so every construction site
  relying on Rect-inheritance had to be audited; tests pin the
  default and the plain-editor entry tool.

**Superseded by** [ADR-0058](ADR-0058-sticky-tools-by-default.md):
the mechanism (policy bool, ownership guard, config knob) is retained;
only the default inverted — tools are now sticky unless
`flip-to-move = true`.
