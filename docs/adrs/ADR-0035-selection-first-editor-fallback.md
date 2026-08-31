# ADR-0035: Selection-first editor phase for `region` captures

**Context.** Two gaps converged. First, `flintshot region` and
`flintshot fullscreen` were the same command in practice — both ran
`capture_and_dispatch` into the plain editor, differing only in
`--delay-ms`; nothing *presented* as "select a region". Second,
[ADR-0028](ADR-0028-capture-then-overlay.md) demotes a config-set
`overlay` to the plain editor window on Wayland ("Wayland users get
the editor window until a layer-shell effort exists"), so the
Flameshot spotlight experience was simply absent there — and
layer-shell remains compositor-class, post-1.0.

But a *decorated toplevel showing a frozen capture with a spotlight
selection* needs no compositor cooperation at all. The overlay
already proved the chrome (dim mask, first-drag selection,
Enter-crop, Esc-cancel) runs fine over a frozen frame through the
unchanged `EditorState` dispatcher; the only thing Wayland can't
host is the fullscreen borderless window around it.

Alternatives considered:

- **Port the full overlay chrome into the editor** (handles, W×H
  badge, magnifier). Rejected for v1: the Select-tool bay already
  shows live dims, and the chrome exists on the true overlay where
  it belongs.
- **A third interaction mode / config key / CLI flag.** Rejected:
  `--overlay` / `--editor` select the *shell*; this changes how the
  editor shell *opens* for region captures. No new surface.
- **Layer-shell on Wayland now.** Rejected: compositor-class scope,
  unchanged from ADR-0028's deferral.

**Decision.** The editor window opens a **selection-first phase**,
requested by the app crate and owned by the shell:

- `EguiShell::with_selection_first()` (mirrors `with_pin_config`; no
  `EditorShell` trait change) arms `Tool::Select` and renders the
  dim mask outside the live selection plus a one-line hint
  ("drag to select · Enter crop · Esc cancel"). The mask is the
  overlay's own four-rect math + `MASK_ALPHA` via shared
  `mask_rects` / `mask_color` helpers — one implementation, two
  shells. Zero core changes; `EditorState` is only seeded.
- The app crate asks for it at **one decision point** beside
  interaction-mode resolution: selection-first iff the capture is
  `region` **and** the resolved shell is the editor window —
  covering the default `editor` mode on every platform, `--editor`,
  and the Wayland demotion of config-set `overlay`. `fullscreen`
  (even Wayland-demoted) and `open` stay plain. Daemon workers
  inherit via their bare `region --standalone` argv.
- The phase **ends permanently on the first crop**, latched from
  either crop site (Enter dispatch, toolbar button) by keying on the
  undo stack gaining a full snapshot — the only mutation that pushes
  one — so undo cannot resurrect the chrome.
- **Esc keeps the editor's standing close-as-`Cancelled` path**
  (exit 0, no file, no clipboard write, no worker); the spec's
  no-selection/has-selection split collapses onto it by design, and
  a pre-published copy outcome wins over the later Esc close.
- **Outputs honour a live selection** during the phase: Copy / Pin /
  Upload were already selection-aware; Save joins them via
  `compose_for_save(selection_aware, state)`. After the phase ends —
  or in `fullscreen` / `open` — Save keeps its full-image Phase 1
  contract.

**Consequences.**

- Revises ADR-0028's Wayland consequence: Wayland `region` now
  *feels* like the overlay with a title bar — same first-drag,
  same Enter-crop, same cancel contract — without a fullscreen
  window. The true overlay stays X11 / macOS / Windows.
- `region` and `fullscreen` are observably different commands on
  every platform.
- The window stays decorated and image-sized (not maximized /
  undecorated): the capture already fills the monitor, and
  decorations are honest Wayland UX with zero window-management
  risk.
- Chrome is mask-only in v1; handles / badge / magnifier remain
  overlay-exclusive polish unless field use asks for them.
- Live-selection Save is pixel-identical to crop-then-Save for
  annotations fully inside the region; straddling annotations
  legitimately differ (geometric clip after crop vs pixel clip
  before) and sit outside the parity claim.

**Open questions.**

1. Should `fullscreen --overlay` on Wayland demote to
   selection-first too? **No** — fullscreen's contract is "the whole
   monitor is the shot"; its demotion stays the plain editor, and
   the demotion log points users at `region` for the spotlight.
2. Opt-out (e.g. `[ui] region-selection-first = false`) for users
   who want the old plain editor on `region`? Deferred until
   someone asks; this entry records the escape hatch.
