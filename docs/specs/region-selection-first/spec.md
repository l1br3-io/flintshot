---
status: draft
started: 2026-08-31
---

# Spec — Region selection-first editor (spotlight in the editor window)

> Feature effort. For product context, see
> [`../../VISION.md`](../../VISION.md); for sequencing, see
> [`../../ROADMAP.md`](../../ROADMAP.md). This effort stands on
> [Phase 1](../phase-1-capture-editor/spec.md) (capture + editor +
> dispatcher) and [Phase 5](../phase-5-live-overlay/spec.md) (the
> overlay + interaction modes), and revises one consequence of
> [ADR-0028](../../adrs/ADR-0028-capture-then-overlay.md): "Wayland
> users get the editor window until a layer-shell effort exists."

## Objective

Make `flintshot region` land in a **selection-first** editor session
whenever the editor window is the shell — i.e. the default
`interaction-mode = "editor"` on any platform, and the Wayland demotion
of `overlay`. The window opens with the frozen fullscreen capture, the
Select tool pre-armed, and a **spotlight dim mask** (the Phase 5
overlay's chrome) over everything outside the active selection. The
first drag draws the region; Enter crops (existing behaviour) and the
session drops into the normal editor on the cropped image; Esc before
any selection cancels the whole capture.

Two problems, one fix:

1. **`region` ≡ `fullscreen` today.** Both commands converge on the
   same `capture_and_dispatch` → plain editor; the only difference is
   `--delay-ms`. Nothing *presents* as "select a region".
2. **Wayland has no overlay.** ADR-0028 demotes config-set `overlay`
   to the plain editor window — the Flameshot experience is simply
   absent there. But a *decorated toplevel showing a frozen capture
   with a spotlight selection* needs no compositor cooperation at all.

**Success looks like:**

- A Wayland user clicks *Capture region* in the tray (or runs
  `flintshot region`): the editor opens on the frozen shot, everything
  dimmed, cursor inviting a drag. They drag, the spotlight follows the
  selection, they press Enter — the window snaps to the cropped image
  and the normal toolbar session continues. The flow *feels* like the
  overlay without a fullscreen window.
- An X11 user with `interaction-mode = "editor"` gets the same
  selection-first landing; with `interaction-mode = "overlay"`
  nothing changes (the true overlay still wins).
- `flintshot fullscreen` is byte-identical to today.

## Scope (in / out)

**In:**

- A *selection-first phase* on the editor-window shell
  (`flintshot-editor-egui`): `Tool::Select` pre-armed on open, dim
  mask outside the active selection (mask math reused from
  `overlay.rs`), a one-line hint ("drag to select · Enter crop · Esc
  cancel"), and the phase transition: first crop ends the phase
  permanently for that session.
- `EguiShell` builder entry: `.with_selection_first()` (mirrors the
  `with_pin_config` precedent — a concrete-method addition, no
  `EditorShell` trait change).
- `flintshot-app` wiring: the `region` → editor-window path (default
  editor mode on all platforms, and the Wayland `overlay` demotion)
  opens selection-first. `fullscreen`, `open`, and `pin` do not.
- Esc semantics during the phase: with **no selection yet**, Esc
  cancels the capture (exit 0, no side effects — mirroring the
  overlay's cancel contract). After a selection exists or the phase
  has ended, Esc keeps today's editor behaviour.
- Outputs during the phase: Copy / Save / Pin / Upload honour an
  active selection via the existing `compose_final` selection path —
  cropping first is a convenience, not a requirement.
- ADR-0035 (selection-first fallback revises ADR-0028's Wayland
  consequence), CHANGELOG entry, TEST_MATRIX rows.

**Out:**

- Wayland layer-shell / compositor-class overlays (still post-1.0).
- Any change to the true overlay shell or to `interaction-mode`
  resolution order (flag > config > platform fallback stands).
- Any `fullscreen` behaviour change; any new config key (v1: region
  always lands selection-first on the editor path — see Open
  Questions).
- Resize handles / move-drag / W×H badge / magnifier porting into the
  editor window (the Select-tool bay already shows live dims; chrome
  beyond the mask is deferred unless cheap).
- Core (`flintshot-core`) behaviour changes beyond none-at-all.

## Deliverables

1. `flintshot-editor-egui`: `EditorApp` selection-first phase — mask
   rendering, Select pre-arm, hint line, Esc-cancel, phase-end on
   crop — plus unit tests for the phase transitions and the reused
   mask math.
2. `flintshot-editor-egui`: `EguiShell::with_selection_first()`
   builder feeding a flag into the editor state construction.
3. `flintshot-app`: `run_editor_with_config` gains a region/selection
   flag; `run_region`'s editor path and the Wayland fallback set it;
   decision logs now name selection-first; `run_fullscreen` /
   `run_open` unchanged (tests pin this).
4. `docs/adrs/ADR-0035-selection-first-editor-fallback.md`,
   `CHANGELOG.md` under `[Unreleased]`, TEST_MATRIX rows (Wayland
   region flow, X11 editor-mode region flow, fullscreen unregressed).

## Exit criteria

1. On Wayland: tray *Capture region* / `flintshot region` /
   config-set `overlay` (demoted) all land selection-first — dim mask
   visible, first drag draws the selection, Enter crops and the
   window resizes to the cropped image, toolbar session continues.
2. Esc with no selection during the phase exits the process with code
   0 and no side effects (no file, no clipboard write, no worker).
3. Copy/Save with an active selection (uncropped) produces the
   selected region only — same pixels as crop-then-output.
4. `flintshot fullscreen` and `flintshot open` open the plain editor
   (no mask, no pre-armed Select) — pinned by unit tests on the CLI
   dispatch side.
5. `interaction-mode = "overlay"` on X11 still opens the true
   `OverlayApp` (unregressed).
6. All standing gates green: `fmt`, `clippy -D warnings`, workspace
   tests, doc tests, `no_gui_types_in_core.rs`.

## Technologies

Unchanged workspace stack. No new dependencies. The mask is four
filled rects (overlay.rs's existing approach) drawn by the editor's
egui painter; no texture work, no compositor features — which is the
point: it works anywhere a toplevel does.

## Commands

Standing gate (AGENTS.md). Phase-specific entry points:
`cargo run -- region` (editor mode), `cargo run -- region --overlay`
(Wayland: demoted + selection-first), `cargo run -- fullscreen`
(plain editor, unregressed).

## Boundaries

- `flintshot-core`: **zero changes.** The selection-first phase is
  shell-owned state in `EditorApp`, exactly like the overlay's chrome
  is shell-owned. `EditorState.tool` is set to `Tool::Select` at
  construction (a value the core API already exposes).
- All phase logic lives in `flintshot-editor-egui`; the app crate only
  decides *when* to request it (mirroring how the app already picks
  editor vs overlay).
- Daemon / tray / IPC / COSMIC code is untouched — workers already
  spawn bare `region` argv, so delegation needs no protocol change.

## Success criteria

- A Wayland user describing the region flow cannot tell it apart from
  "the overlay with a title bar" — same first-drag, same Enter-crop,
  same cancel contract.
- `region` and `fullscreen` are now observably different commands on
  every platform.
- No second dispatcher, no second mask implementation: the editor
  reuses the overlay's mask constant/math or a shared helper extracted
  beside it, with tests.

## Decisions

- **Selection-first is the editor-window experience of `region`**, not
  a third interaction mode — no new config key, no new CLI flag. The
  existing `--overlay`/`--editor` flags keep their meaning; they
  select the *shell*, and this changes how the editor *shell* opens
  for region captures. → ADR-0035.
- **Decorated, image-sized window** (as today) rather than maximized /
  undecorated: the capture already fills the monitor, so the window is
  effectively fullscreen; keeping decorations is honest Wayland UX and
  zero new window-management risk.
- **Phase ends on first crop**, not on "any interaction" — outputs
  must keep working with a live selection (uncropped), and ending on
  tool switches would fight the existing "Select then Copy" flow.
- **Mask only, not full overlay chrome**, in v1 — handles/badge/
  magnifier already exist on the true overlay; porting them into the
  editor is polish if the phase-2 tests reveal a need.

## Open questions

1. Should `fullscreen --overlay` on Wayland also demote to
   *selection-first* (today it demotes to the plain editor)? Lean no —
   fullscreen's contract is "the whole monitor is the shot" — but the
   demotion log should point users at `region` for the spotlight.
2. Should there be an opt-out (e.g. `[ui] region-selection-first =
   false`) for users who want the old plain editor on region? Deferred
   until someone asks; the Open Questions section of ADR-0035 records
   the escape hatch.
