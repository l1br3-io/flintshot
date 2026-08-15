---
status: draft
---

# Spec — Phase 5: Live overlay

> Draft effort spec. For product context, see
> [`../../VISION.md`](../../VISION.md); for sequencing, see
> [`../../ROADMAP.md`](../../ROADMAP.md); for the technical substrate
> this builds on, see [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md)
> (especially §Cross-platform concerns, §Known risks & mitigations).
> The shipped phases this effort stands on:
> [Phase 1](../phase-1-capture-editor/spec.md) (capture + editor +
> dispatcher), [Phase 2](../phase-2-daemon-tray-hotkeys/spec.md)
> (daemon + worker spawn model),
> [Phase 2b](../phase-2b-annotation-tools/spec.md) (all annotation
> tools), [Phase 4](../phase-4-upload/spec.md) (output pipelines).

## Objective

Add the **Flameshot-style live overlay**: after a capture, instead of
opening a separate editor window, show the *frozen* screen fullscreen
and let the user select and annotate in place — dim the unselected
area, drag out a region, draw on it, and dispatch outputs (copy /
save / pin / upload) from a toolbar floating beside the selection.

This is the second of the two UX models the vision commits to
("editor-window first, overlay later — robustness over flair"). The
editor window stays the default; the overlay is a user-selectable
interaction mode, per capture.

**Key architectural stance (pre-decided in ARCHITECTURE.md):**

1. **Capture-then-overlay, not live transparency.** The overlay
   displays the already-captured pixels (`xcap` runs first, exactly as
   in editor-window mode). No screen-read-from-compositor tricks, no
   `layer-shell`, noWLCS dance. The fullscreen window *is* the frozen
   screen; the desktop is never actually visible beneath it. This is
   deterministic across platforms and reuses the whole Phase 1
   capture path unchanged.
2. **Wayland falls back to editor-window.** A plain fullscreen window
   cannot reliably be raised above other windows / grab input on all
   Wayland compositors. Per the risk table, this is *honest UX, not a
   hidden bug*: on Wayland the daemon/worker logs a notice and opens
   the editor window. A compositor-class overlay (layer-shell via
   `sctk`/`ashpd`) is explicitly out of scope and deferred until
   there's demand.
3. **The dispatcher is the overlay's engine.** `EditorState` +
   `dispatch_input` already implement every tool, undo/redo,
   selection, and text editing toolkit-agnostically. The overlay is a
   new *shell* in `flintshot-editor-egui` — zero core behaviour
   changes are in scope beyond config plumbing and (at most)
   selection-handle hit-testing helpers.

**Success looks like:**

- A user sets `interaction_mode = "overlay"` (or passes `--overlay`),
  presses their region keybind, the screen "freezes", they drag a
  region, scribble an arrow, press Enter — the overlay closes and the
  annotated region is on the clipboard. Total flow under 3 seconds
  with no window-manager flicker.
- The same user on Wayland runs the same command and gets the Phase 1
  editor window, with a `tracing::info!` explaining why. Nothing is
  broken, nothing silently differs.

## Scope (in / out)

**In:**

- `[ui] interaction_mode` config (`"editor"` default | `"overlay"`) —
  the key ARCHITECTURE.md reserved — plus a per-invocation CLI
  override (`flintshot region --overlay` / `--editor`).
- `flintshot-editor-egui`: a fullscreen borderless `OverlayApp`
  (eframe viewport, precedent: the Phase 3 pin window's
  `ViewportBuilder`) rendering the frozen capture.
- Flameshot selection semantics on top of the existing dispatcher
  selection: dim mask outside the active region, 8 resize handles,
  move-by-drag inside the region, live `W × H` badge.
- A floating overlay toolbar anchored beside the selection:
  the existing tool rail (reuse `toolbar` chips/glyphs), the four
  output actions, and Enter-accept / Esc-cancel.
- Annotation of all ten tools inside the selection via the unchanged
  dispatcher + `render()` path; magnifier works on hover.
- Outputs reuse the Phase 1–4 pipelines verbatim: Copy
  (clipboard, X11 owner rules), Save (`rfd`), Pin / Upload
  (temp PNG + detached worker). **Enter defaults to Copy**
  (Flameshot muscle memory); Esc cancels with no side effects.
- Platform handling: X11 / macOS / Windows overlay; Wayland detection
  + editor-window fallback (config override to force either mode).
- Multi-monitor: the **active monitor** only (mirrors the current
  `flintshot region` default). Spanning overlays are a stretch goal.

**Out:**

- Wayland layer-shell / compositor-class overlays ( revisit post-1.0).
- Multi-monitor spanning or per-monitor overlay windows.
- New annotation tools, new core behaviours, COSMIC applet changes
  (the worker process picks the mode from config; IPC is unchanged).
- Overlay-specific settings persistence beyond `interaction_mode`.

## Deliverables

1. `flintshot-core::config`: `UiConfig::interaction_mode` (kebab-case
   serde, `"editor"` default, unknown values fall back with a
   `tracing::warn!` — same policy as `magnifier-modifier`).
2. `flintshot-editor-egui::overlay`: the `OverlayApp` + selection
   chrome (mask, handles, badge, floating toolbar) + accept/cancel
   flow, publishing the existing `EditorOutcome`s.
3. `flintshot-app`: `--overlay` / `--editor` flags on `region` /
   `fullscreen`; mode resolution (flag > config > platform fallback)
   in `capture_and_dispatch`; Wayland detection helper.
4. ADR-0028 (capture-then-overlay decision), CHANGELOG entries,
   TEST_MATRIX rows (per-OS overlay flows + Wayland fallback).

## Exit criteria

1. With `interaction_mode = "overlay"` on X11 (or macOS/Windows):
   `flintshot region` freezes the screen, selection drag + handles +
   move work, at least Rect / Arrow / Text / Number annotate
   correctly *inside* the selection, magnifier follows the cursor.
2. Enter copies the annotated region to the clipboard and closes;
   the toolbar's Save / Pin / Upload behave exactly as in the editor
   window (including the async upload worker + notification path).
3. Esc cancels: no file written, no clipboard write, no worker
   spawned, exit code 0.
4. On Wayland: same command opens the editor window; log line names
   the fallback; `--overlay` force-flag is still honored as
   best-effort (documented) or refused with a clear error — decided
   in Task 007, encoded in TEST_MATRIX.
5. `interaction_mode = "editor"` (or `--editor`): byte-identical
   behaviour to today. Editor-window mode is not regressed.
6. Overlay repaints only on interaction (no continuous CPU burn at
   60 fps while idle) — verified via `time cargo run -- region
   --overlay` idle CPU sampling.
7. All standing gates green: `fmt`, `clippy -D warnings`, workspace
   tests, doc tests, `no_gui_types_in_core.rs`.

## Technologies

Unchanged from the workspace stack (see ARCHITECTURE.md §Tech): eframe
viewports (fullscreen, undecorated — the pin window is the precedent
for borderless egui viewports), `xcap` capture-first, the shared
`render()` path, `arboard` / `rfd` / worker processes for outputs.
**No new dependencies.**

## Commands

Same as the standing gate (AGENTS.md); the phase-specific entry
points are `cargo run -- region --overlay`, `cargo run -- fullscreen
--overlay`, and the config round-trip test for `interaction_mode`.

## Boundaries

- `flintshot-core` gains **no GUI types, no overlay logic** — only the
  config field (+ its tests). Selection-handle hit-testing, if it
  needs sharing, lands as pure geometry in `core::geom` with tests.
- All overlay chrome lives in `flintshot-editor-egui` behind the
  existing crate boundaries; `EguiShell` may grow a `run_overlay`
  entry (trait addition is *not* required — a concrete method
  mirrors how `run_pin`'s config rides the shell).
- Daemon / tray / COSMIC applet code is untouched.

## Success criteria

- The overlay flow is indistinguishable-in-speed from Flameshot's on
  the same hardware (cold capture → interactive overlay < 200 ms
  beyond capture cost).
- Zero regressions in editor-window mode (full TEST_MATRIX re-run).
- The codebase does not grow a second dispatcher: overlay input goes
  through `EditorState::dispatch_input` or a documented, tested
  exception exists.

## Decisions

- **Capture-then-overlay** (frozen pixels, no live transparency) —
  ADR-0028 will record context/decision/consequences.
- **Enter = Copy** (Flameshot muscle memory); Save / Pin / Upload are
  explicit toolbar actions.
- **Active monitor only** for v1; multi-monitor spanning deferred.
