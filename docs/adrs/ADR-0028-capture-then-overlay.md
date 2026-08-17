# ADR-0028: Capture-then-overlay (frozen frame) for the live overlay

**Context.** The vision commits to a Flameshot-style live overlay as
the second UX model ("editor-window first, overlay later"). Two
implementation families exist for "select a region on the live
screen":

1. **Live transparency.** A transparent fullscreen window floats over
   the desktop; the screen shows through it; capture happens *after*
   the user confirms a selection. On Linux this needs compositor
   cooperation: X11 can do transparent override-redirect windows, but
   Wayland has no global coordinates, no reliable raise, and no input
   grab for ordinary toplevels — the honest path there is layer-shell,
   which is compositor-class (works on wlroots/COSMIC, not a GNOME/KDE
   default). macOS and Windows would each need their own
   always-on-top-transparent-window dance.
2. **Capture-then-overlay.** Grab the screen *first* through the
   existing `xcap` path (exactly what editor-window mode already
   does), then show the frozen pixels fullscreen in a borderless
   viewport. The fullscreen window *is* the screen; the desktop is
   never actually visible beneath it.

Live transparency also couples capture timing to the UX: whatever is
under the overlay can change mid-selection, so the annotated output
and the pixels the user aligned their selection to can diverge. And it
would fork the capture path — a second, "capture later" pipeline
parallel to the Phase 1 flow every output (save / copy / pin /
upload) composes against.

**Decision.** The overlay is capture-then-overlay.

- `xcap` capture runs first, unchanged; `OverlayApp` (a new eframe
  app in `flintshot-editor-egui::overlay`, precedent: the Phase 3 pin
  window's borderless `ViewportBuilder`) opens fullscreen and
  undecorated over the active monitor, rendering the frozen capture
  through the same shared `render()` path as the editor window.
- The overlay is a *shell*, not a second editor: all input translates
  to `InputEvent`s and drives the unchanged `EditorState` dispatcher;
  the Flameshot chrome (dim mask, 8 resize handles, move-drag,
  W×H badge, hover magnifier, floating toolbar) is overlay-owned,
  with the handle/hit/placement math as pure unit-tested functions.
- Outputs reuse the Phase 1–4 pipelines verbatim (`compose_final`
  / `_region`, `rfd`, temp-PNG + detached pin/upload workers);
  **Enter = Copy** (Flameshot muscle memory), **Esc = Cancel** with
  no side effects. On X11 the process lingers briefly after an
  accept-with-copy so the clipboard owner survives to paste.
- **Wayland falls back to the editor window, honestly.** A plain
  fullscreen window cannot reliably be raised above other windows or
  grab input on all Wayland compositors, so session detection
  (`WAYLAND_DISPLAY` / `XDG_SESSION_TYPE`) demotes a config-set
  `overlay` to the Phase 1 editor window with an info log naming the
  fallback. An explicit `--overlay` flag is honored best-effort with
  a warning — the fallback protects the default path, not informed
  overrides. Layer-shell is explicitly out of scope, deferred until
  there's demand.

**Consequences.**

- Deterministic and portable: the overlay works anywhere eframe can
  open a fullscreen window (X11 / macOS / Windows), with zero new
  dependencies and zero core behaviour changes beyond the
  `interaction-mode` config field.
- The frozen frame is a feature, not a compromise: content cannot
  change mid-selection, and every output composites against exactly
  the pixels the user saw. WYSIWYG holds by construction (same
  `render()` path, same dispatcher).
- The desktop is not live: animations freeze, the cursor is egui's,
  and anything changing behind the overlay is invisible until
  accept/cancel. Users who need the live desktop (e.g. capturing a
  paused video frame is *easier*; capturing a menu that closes on
  focus change now can't) still have the editor-window mode — and
  menus/tooltips that vanish on capture are frozen *in* the shot,
  which is usually the desired outcome.
- One capture's worth of latency is paid before the overlay appears;
  the overlay itself must open in <200 ms beyond capture cost (met —
  the frozen frame is a single cached texture blit).
- Multi-monitor spanning is deferred: the overlay covers the active
  monitor only, mirroring the `flintshot region` default. Spanning
  windows are a future effort.
- Wayland users get the editor window until a layer-shell effort
  exists (post-1.0 candidate). The fallback is logged, never silent —
  "honest UX, not a hidden bug".
