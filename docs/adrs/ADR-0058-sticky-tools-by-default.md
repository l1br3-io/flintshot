# ADR-0058: Sticky tools by default

**Context.** ADR-0040 made flip-to-move the default: every eligible
annotation commit switched the active tool to `Tool::Move` so the next
press could reposition what was just drawn. In practice this reads as
the tool "resetting itself" — the dominant workflow is batching several
annotations of the same kind (three arrows, a run of labels), and the
flip forces a re-press of the tool (chip or hotkey) after every single
commit. The reposition case it served is already one keypress away
(`M` / the Move chip).

The mechanism was always policy-gated (`EditorState::flip_to_move`
plumbed from `[ui] flip-to-move`), so this is a default inversion, not
a redesign. Multi-selection of annotations — which would change the
calculus by making Move richer — is specced separately and must not
depend on which default ships first.

**Decision.**

- The default flips: `[ui] flip-to-move` now defaults to `false` —
  tools are sticky across commits. `EditorState::new` matches
  (`flip_to_move: false`) so core's plain-construction default and the
  config-plumbed default agree.
- Everything else from ADR-0040 stands unchanged: the single commit
  funnel, the ownership guard (a draft flushed by a tool switch never
  flips — the explicit choice wins), Number's exemption, no undo
  entries for flips, and read-once-per-session config plumbing.
- The editor still *opens* on `Tool::Move` (`Tool::default()` is
  unchanged); only the post-commit behavior changes.

**Consequences.**

- Drawing N annotations with one tool costs N drags instead of N
  drags + N re-presses; repositioning costs the same as before (press
  Move once, then drag freely).
- Users who preferred the old cadence set `flip-to-move = true`; the
  knob and its roundtrip tests are unchanged.
- Existing configs with an explicit `flip-to-move = true` keep the old
  behavior — the new default reaches only configs where the key is
  absent (or explicitly `false`).
- Tests that exercised the flip now opt in by setting
  `flip_to_move = true` explicitly; the sticky default is pinned by
  its own assertions.

Supersedes [ADR-0040](ADR-0040-flip-to-move-after-commit.md) (default
only; mechanism retained).
