# Changelog

All notable changes to `flintshot` are recorded here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Version `0.x.y` is in effect during development — no API stability
guarantees until `1.0.0` (see `docs/ROADMAP.md`).

Categories used below:

- **Added** — new features, crates, subcommands, public types
- **Changed** — changes to existing functionality
- **Deprecated** — soon-to-be-removed features
- **Removed** — removed features
- **Fixed** — bug fixes
- **Security** — vulnerability fixes

## [Unreleased]

### Added

- **Session captures: unsaved captures survive navigation until app
  exit** (effort `session-captures`; ADR-0067). Every capture that
  lands in the app window's editor is tracked — image plus committed
  annotations — in a temp-dir session store
  (`$XDG_RUNTIME_DIR/flintshot/session/`, advisory, swept on startup,
  cleared on graceful exit, never resumed across restarts).
  - *Navigate away, come back*: going Home (or Settings, or an
    external capture superseding the editor) no longer loses
    in-session annotations — Home shows a **"This session"** section
    above the recents grid with resumable cards (live thumbnails,
    "Unsaved" caption); clicking one reopens the editor with every
    committed annotation intact (undo/redo history resets by
    design). Mid-edit supersedes stash the outgoing capture as a
    card instead of discarding it.
  - *Lifecycle*: annotations flush per-commit (crash-safe sidecar
    rewrites) and at every editor end (route swaps, image
    replacements, window-closing exits); **saving** graduates the
    capture into recents and removes its card; **uploads, copies,
    and Esc deliberately keep it** (no disk file is guaranteed for
    an upload); the card's two-step **Discard** chip is the only
    user destroy; closing the app clears the scratch dir.
  - *Hidden-close restores now keep annotations* when the store
    entry lives (the documented "Replace policy's loss" is gone);
    the bare pre-edit stash remains only as the store-miss fallback.
  - New surface: `flintshot_core::{SessionStore, SessionId,
    SessionCapture, SessionSummary, session_dir}`,
    `RouteHost::editor_route_left`,
    `EditorPageBridge::annotations_committed`,
    `HomeBridge::{open_session_capture, session_captures,
    discard_session_capture}`, `SessionCard`.

### Changed

- **Tray / applet *Open app window* now routes the open window to its
  home page** (mirrors the *Settings* ping's choreography). The click
  always showed + focused an already-running window, but never
  navigated — a window sitting on the settings page (or an already
  focused one) made the item look dead. An idle `AppFocus` ping now
  also arms the route-home intent: the open window swaps home in
  place, wherever it sat. Mid-capture pings stay fully suppressed
  (the capture owns the screen; rerouting a closed window's restore
  home could drop an in-progress editor session). (Pinned by
  `app_focus_pings_arm_the_route_home_intent`,
  `app_focus_pings_do_not_route_home_mid_capture`,
  `route_home_intent_returns_the_settings_route_home`.)

### Added

- **Recent Captures v2: standard storage, richer cards,
  clipboard-copy persistence** (effort `recent-captures-v2`).
  - *Standard, configurable storage*: an empty `[output] save-dir`
    now resolves at runtime to `~/Pictures/Flintshot` (platform
    pictures dir; created on first write) instead of silently
    writing to the process CWD — the fallback the docs promised
    but never shipped. The Settings *Save location* row became a
    real picker: the effective path display (never a placeholder),
    a **Browse** folder dialog, and a **Use default** reset. The
    plain Save dialog prefills the resolved directory.
  - *Richer recents cards*: each card shows the filename over a
    metadata line (`1920×1080 · 240 KB · 5 mins ago`, one stat per
    file at decode); hovering reveals cutout chips — **Copy**
    (file → clipboard, X11/Wayland keep-alive rules honored),
    **Reveal** (parent folder in the file manager), **Delete**
    (two-step "Sure?" confirm on the chip; removes file + index
    entry). Clicking elsewhere still opens in the editor. The grid
    scrolls once rows overflow; still 4 per row. The
    `recents.toml` schema is untouched (path + mtime; zero
    migration).
  - *Clipboard copies are persisted*: with `[output]
    save-clipboard-copies = true` (default), the editor's **Copy**
    button and `after-grab = "clipboard"` captures also write the
    image to the resolved save dir — template-or-`flintshot-{date}-
    {time}` naming, `-2` collision suffixing, `[output] format`
    encoding — and land in recents. A save failure never fails the
    copy.
  - *Configurable capacity*: `[ui] recent-captures-limit`
    (default 50, clamped 1–200) drives the index truncation and
    the grid; Settings exposes it as preset wells. Legacy
    20-entry indexes load unchanged and grow to the new limit on
    the next record.
- **History tab in the editor panel** (effort
  `properties-history-tabs`). The docked panel grew a
  `PROPERTIES | HISTORY` tab header; the new History tab renders
  the edit timeline — one labeled row per gesture (`Added arrow`,
  `Moved 3 items`, `Cropped`, `Edited text`, …), oldest first, with
  a current-state marker and dimmed future (redo) rows. Each row
  also shows its elapsed time since the session start (`0:14`,
  `1:02:03`) right-aligned — stable across time travel, and the
  stack stays sorted oldest-first (the Photoshop convention).
  Click any row to jump straight to that moment; clicking a
  dimmed row redoes
  forward. The floating undo/redo chips and `Ctrl+Z` /
  `Ctrl+Shift+Z` chords keep working, and the timeline reflects
  them live. The Properties tab (tool defaults / selection
  inspector) is unchanged.

### Fixed

- **Settings edits to `[output]` (and three `[ui]` keys) never
  reached disk** (effort `recent-captures-v2`). Saving into an
  existing `config.toml` merged only the early `[ui]`/`[capture]`/
  `[hotkeys]`/`[upload]` keys — the whole `[output]` table plus
  `show-statusbar`, `statusbar-size-mode`, and `animations` were
  silently dropped from the rewrite, so Settings-driven changes to
  the save format, template, or directory did not survive a
  re-save. The merge now writes every known key in all five
  tables (comments preserved, pinned by
  `save_into_existing_file_writes_output_and_late_ui_fields`).

- **The worker editor's Settings entry closes behind its handoff —
  no more duplicate windows** (effort `fix-tray-window-lifecycle`).
  The worker's topbar ⋮ → Settings used to fire the delegate and
  *stay open*, leaving the freshly-spawned app window beside the
  still-visible editor (which then landed in the next shot). The arm
  now mirrors the ADR-0065 `NavigateHome` choreography exactly:
  delegate while alive to ask, publish `Cancelled`, close the
  viewport. Hook-absent (tests, headless) keeps the same publish +
  close contract. (Pinned by
  `settings_with_hook_fires_it_then_cancels_and_closes`,
  `settings_publishes_cancelled_on_worker_windows`,
  `page_settings_with_hook_still_publishes_and_closes`.)
- **A slow prepare no longer ghosts the tray for 30 s.** When the
  worker's `PrepareCapture` reply exceeded its 2 s client bound, the
  client fell back to a second, tray-only handshake — but the daemon
  had already `begin_capture`d for the prepare, so the fallback began
  *again* and the worker's single `ReportCaptured` left the tray
  hidden until the 30 s safety restore. A reply timeout is now
  understood as "armed but slow" (registered, unconfirmed, exactly
  one report owed — no second handshake); only an explicit
  `unsupported` reply from a known-old daemon keeps the legacy
  tray-only fallback. (Pinned by
  `prepare_capture_timeout_does_not_run_the_tray_fallback` and
  `prepare_capture_falls_back_to_tray_on_unsupported`.)
- **Tray helper loss is never silent, and churn backs off.** A
  failed helper spawn used to leave the tray gone until the next
  capture's hide/show cycle, `TraySupervisor::is_present`
  unconditionally claimed a tray existed (so the gate waited on
  pixels that were never there), and an IPC-less daemon produced a
  ~0.5 s infinite respawn churn. Spawns now retry with capped
  exponential backoff (first fast exit free — it may be a real
  capture/quit click; stable uptime resets the curve), presence
  reports the real child state, and the helper's daemon watch
  retries its connect inside a bounded ~2 s startup-race window
  before following a dead daemon. (Pinned by
  `helper_backoff_delay_curve`, `spawn_failures_retry_with_growing_backoff`,
  `consecutive_fast_self_exits_back_off`,
  `stable_uptime_resets_the_churn_backoff`,
  `late_listener_is_accepted_within_the_window`,
  `absent_daemon_marks_gone_after_the_window`.)
- **The no-daemon fallback spawns from worker editors carry
  `--standalone`.** The direct `flintshot app` / `app --route
  settings` spawns (Settings/Home entries with no daemon answering)
  used to die on the "stale `flintshot app` process?" error against
  a lock holder instead of exiting quietly; they now route through
  the daemon's own `worker::app_argv` builder. (Pinned by
  `fallback_app_spawns_carry_standalone`.)

### Changed

- **The app window is the liveness anchor: daemon death self-heals**
  (effort `fix-tray-window-lifecycle`, ADR-0066). Losing the daemon
  subscription no longer degrades the window permanently — the pump
  keeps answering capture commands with in-process grabs while it
  reconnects on a capped exponential curve (250 ms base, 5 s cap),
  respawns the daemon through a 30 s rate limiter (a crash-looping
  daemon cannot storm process spawns), and on reconnect re-announces
  presence — so the fresh daemon routes the next capture to the
  window instead of spawning a worker editor beside it — and returns
  the status to "connected". The old terminal banner ("daemon
  connection lost — restart `flintshot app`") is now a
  *reconnecting* status; there is no terminal degraded mode anymore.
  (Pinned by
  `disconnect_status_says_reconnecting_not_terminal`,
  `reconnect_delay_grows_from_the_base_and_caps`,
  `daemon_spawn_limiter_rate_limits_respawns`,
  `reconnecting_loop_reconnects_when_the_daemon_returns`, and
  `reconnecting_loop_rate_limits_respawns_and_exits_on_close`; see
  ADR-0066 for the runtime matrix.)

- **Settings page no longer paints red duplicate-id debug overlays
  over the rows and the topbar while scrolling (prior entry).** The section
  widget's invisible measure pass used to run 10 000 px offscreen,
  so every row's widget id registered at two distant rects per
  frame — and egui's duplicate-id detector (debug builds only)
  answered with a red stroke around each control plus a red
  "🔥 First/Second use of widget ID" label painted on the unclipped
  debug layer, which read as page elements floating over the topbar
  while the page scrolled. The probe now lays out at the real
  body's position, so both registrations share the same rects and
  the detector stays quiet. Also drops a leftover `eprintln!` from
  the settings link widget.
  (Pinned by
  `settings_page_never_trips_eguis_duplicate_id_paint`.)

### Added

- **Resize image: destructive, undoable downscale from the editor's
  topbar overflow menu.** A new **Resize** item (editor surface only)
  opens a popover with 25% / 50% / 75% preset chips — hover shows the
  resulting `WxH` — and a custom-width field with a live
  aspect-derived `× H` label (Enter applies). The whole composition
  scales down uniformly: image, annotation geometry, stroke widths,
  font sizes, arrow heads, and number diameters, with blur strength
  kept ≥ 1 so blurs stay visible. Never upscales (presets and the
  custom field clamp to below the current width; the entry dims at
  ≤ 1 px).   Ctrl+Z restores the original pixels and annotations
  bit-for-bit; Esc closes the popover without leaving the editor.
  Each resize lands on the History tab as its own labeled `Resized`
  row (elapsed-timestamped, clickable like every other edit).
  (Pinned by the `resize*` unit tests across `flintshot-core` and
  `flintshot-editor-egui`; effort spec:
  `docs/specs/editor-resize-image/`.)

- **Text selection & highlight while editing** (effort
  `text-selection-highlight`). In-edit text now behaves like real
  text in both shells (editor window and fullscreen overlay, fresh
  drafts and re-edit sessions alike): **Shift+arrows / Home / End**
  extend a selection char- or line-wise, **Ctrl+Shift+arrows**
  word-wise, **Ctrl+A** selects all, **dragging** across glyphs
  selects, **double-click** selects a word, **triple-click** the
  line. Typing **replaces** the selection in one stroke;
  **Backspace / Delete** delete the range (and `Delete` is no
  longer dead mid-edit — without a selection it forward-deletes).
  Selected glyphs carry a translucent blue highlight; the block
  caret is retired in favour of the industry **thin bar caret**.
  Plain caret moves collapse the selection; Enter / Esc /
  tool-switch commit semantics and the one-undo-per-re-edit-session
  bracketing are byte-for-byte unchanged. Click-away mid-edit now
  commits-and-moves-on (previously clicks were ignored while
  typing).

### Changed

- **New Crop tool glyph.** The rail and overlay chips for Crop swap
  the four corner brackets — visually a cousin of Select's dashed
  marquee — for the classic overlapping-right-angles crop idiom, so
  the two selection-region tools are unmistakable at chip size.
  (Same `ToolSpec`/`TOOL_PODS` vocabulary; no behaviour change.)

- **`just` recipes are quiet by default.** All cargo invocations in the
  justfile now pass `-q` (nextest: `--cargo-quiet`): compile chatter
  and per-test lines are suppressed on success, while errors, test
  failures, and remaining warnings still print in full — keeping
  output (and token cost, when an agent runs these) minimal.
  `just watch` / `just dev` stay verbose for visible rebuild feedback.
- **Tool palette remapped to Select / Crop / Hand**
  (ADR-0064). The editor's conflated tools are split along the
  mainstream image-editor lines: **Select** (V) is now the
  object-selection tool — click selects an annotation,
  Shift/Ctrl+click toggles membership, a drag on empty space draws a
  marquee that bbox-intersection-selects every annotation it
  touches (plain replaces the set, Shift/Ctrl merges), and dragging
  a selected annotation moves the whole set. **Crop** (C) inherits
  the old Select behavior exactly: drag a region, `Enter` or the
  Crop button crops, and Copy honors the active region. **Hand**
  (H) renames Move and becomes pan-only — it never selects, moves,
  or otherwise touches annotations (the pan-only mainstream
  contract; see the ADR's dropped-draft note). The default tool is
  Select; the fullscreen overlay and the editor's selection-first
  phase pre-arm Crop; the old `S` hotkey is freed. The commit-flip
  config key renames `flip-to-move` → `flip-to-select` (same value;
  legacy files migrate silently on first save, comments riding
  along).

### Added

- **View pan + cursor-anchored wheel zoom.** From any tool, the
  mouse wheel steps the view zoom (the same 0.25 quantization as
  the floating +/− chips) keeping the image point under the cursor
  fixed; a middle-button drag pans; a Hand primary drag pans
  (Grab/Grabbing cursor). Pan clamps so the image never leaves the
  viewport entirely and resets at the 1.0 fit zoom. Every mapping
  site (pointer, chrome, magnifier, mask) composes pan through the
  shared canvas view.

### Fixed

- **The worker editor's Home breadcrumb link now navigates instead
  of closing the window** (ADR-0065). From a tray capture (or any
  worker fallback editor), clicking the topbar's `Home › Editor`
  breadcrumb published `Cancelled` and killed the window — a close
  in navigation clothing. The link now delegates the daemon's
  `OpenApp` choreography: an already-running app window focuses
  (keeping its current route); otherwise the daemon spawns
  `flintshot app` on Home, and with no daemon at all the worker
  spawns it itself. The worker editor then closes via the standing
  Esc contract. The app window's own Home routing is unchanged.
  Plumbing: `EguiShell::with_open_home` mirrors `with_open_settings`
  (`EditorApp` fires it on the Home action before the close).

- **The topbar overflow's Settings entry now works from worker editor
  windows** (`flintshot capture` / `flintshot open`). It rendered but
  was a dead no-op there — worker windows never route to Settings
  in-window (spec §Settings route). The entry now fires the daemon's
  `OpenSettings` choreography: an already-running app window focuses
  and routes itself to Settings; otherwise the daemon spawns one
  opened on Settings. With no daemon at all, the worker spawns the
  settings window itself. Plumbing: `EguiShell::with_open_settings`
  hands the editor a delegate hook (`EditorApp` fires it on the
  Settings action; the routed window still handles the entry itself,
  unchanged).

### Added

- **Multi-select annotations + selection inspector.** Under the Move
  tool, Shift+click toggles annotations into a selection set (plain
  click replaces it with the hit annotation, a miss clears it; the
  set survives release). The whole set moves together — one drag,
  one undo entry — and `Del` removes every member in a single
  undoable step. With a selection live, the right panel becomes a
  **selection inspector**: title `N selected · Properties`, showing
  the properties shared by every selected annotation; edits apply
  live to all of them (one undo entry per gesture) and teach the
  session tool defaults. Marker keeps its translucency through
  colour edits. Per-annotation dashed outlines render for every
  selected member in both the editor window and the overlay, and a
  live multi-drag shows the solid grab outline on each member.
  With no selection, the panel is unchanged (tool-defaults mode).
- **Committed text is now a live object under the Text tool**
  (ADR-0062). A single click on a committed text selects it — dashed
  selection outline, and the properties panel's Text section plus the
  colour pod show *that annotation's* values instead of session
  defaults. A double-click re-opens content editing (caret at end;
  type, arrows, Home/End, Shift+Enter newlines, Backspace). While
  selected (or mid-re-edit), panel style changes write through to
  the annotation live — colour, size, family, weight, style, align,
  line-spacing, outline, background — and also become the defaults
  for the next placed text; one `Ctrl+Z` reverts a whole re-edit
  session or one discrete style change (a size-slider drag is a
  single step). `Del` deletes the selected text from the Text tool
  (no Move detour); `Esc` deselects without closing the editor, and
  mid-edit `Esc` reverts the whole edit session. Hovering a
  committed text shows a dashed affordance + I-beam cursor. Both
  shells (editor window and overlay) behave identically; fresh
  drafts and every non-Text tool are unchanged.
- **Settings is the single preferences surface** (settings-improvement).
  The tray and the COSMIC applet offer a *Settings* item that opens
  (or focuses) the app window directly on its settings page — the
  same open-or-focus flow as *Open app window*, carrying the
  destination route over the new `OpenSettings` / `SettingsFocus`
  IPC pair (appended; wire-compatible). `flintshot app --route
  settings` cold-starts there too.
- **Config-file links on the settings page**, the `VSCode`
  *User Settings* / *Default Settings* pair: *Edit config file*
  opens `config.toml` in `$EDITOR` (fallback `$VISUAL`; the path is
  printed when neither is set), materializing the file from a
  commented template first when missing — never clobbering an
  existing one. *View default config* expands the embedded,
  fully-commented default TOML read-only in-app; a test pins the
  asset field-by-field equal to the code defaults.
- **Rebind shortcuts… in Settings** on Wayland sessions — the same
  portal bind dialog the tray item drove (`flintshot hotkeys bind`
  unchanged).

### Fixed

- **Tray *Settings* (and *Open app window*) no longer spawn an
  endless chain of app processes when the window is already open**
  (ADR-0063). The lock-held `flintshot app` child delegated
  `OpenApp` back to the daemon, whose dispatch spawns the next
  child — which delegated again — so one tray click kept forking
  workers and re-raising the window forever, and closing the window
  let the next child in the chain pop a new one open.
  Daemon-spawned app workers now carry `--standalone` (the capture /
  open loop-breaker rule) and exit quietly: the focus/route
  broadcast the daemon sends *before* spawning already did the work.
  A user-run `flintshot app` keeps the focus ping.
- **A tray *Settings* click while the settings page is already open
  now answers with a toast.** The ping used to be silently consumed;
  the window still focuses/raises, and an in-window toast
  ("Settings is already open") confirms it for ~2.5 s. On any other
  page the click still routes to Settings in place; with the window
  closed it still opens directly on Settings.
- **The settings page's scrolling.** Its `ScrollArea` drew without
  a `CentralPanel`: no panel containment for the body and no desk
  background. The body now lives in a cream `CentralPanel` with a
  properly-constrained `ScrollArea`, and a machine-checked
  regression test pins that scrolled content never renders under
  the 44 px topbar.
- **The app window now closes for captures started while the
  settings page is open.** The Settings route consumed no hide
  directives, so an external capture's close request stranded the
  window on screen — in the shot. The router owns the directive
  poll there now.
- **Sticky tools now reach every editor entry, not just captures.**
  The app window's Open / recent-captures / Paste paths built their
  `EditorState` without the `[ui]` session config, so they silently
  diverged from the capture paths (which read it): a config with
  `flip-to-move = true` flipped to Move after commits on captures but
  not on opened images — and every other `[ui]` default (color,
  stroke, font) was skipped on those paths too. All entries now apply
  the same read-once config. Note for configs predating ADR-0058: a
  settings re-save may have materialized `flip-to-move = true` (the
  old default) — set it to `false` to get sticky tools.

### Removed

- **The tray's *Open config* and *Bind shortcuts…* items** (and the
  applet's *Open config*): their features live in Settings now, and
  their menu ids (`flintshot.open-config`,
  `flintshot.bind-shortcuts`) are pinned dead — never reused.

### Changed

- **Sticky tools by default** (ADR-0058, superseding ADR-0040's
  default): committing an annotation keeps the active tool instead of
  flipping to Move — rapid multi-draw no longer needs a re-press per
  commit. The flip cadence remains available via
  `[ui] flip-to-move = true`; the editor still opens on Move.

### Added

- **Adjustable Number-bubble size** (spec `docs/specs/number-tool/`):
  the properties panel's Number section gains a **Size** stepper
  (16–96 px, default 28) beside "Start at", bound to
  `EditorState::number_diameter` and clamped to core's new pub
  `NUMBER_MIN/MAX_DIAMETER`. Session tool state — the next placement
  picks it up, committed bubbles never change, no history entries.
- **Five-zone UI + settings route** (spec `docs/specs/ux-redesign/`):
  the single window re-laid out around the mockup's five zones —
  44 px topbar (flame brand, breadcrumb, Capture / Copy / Open hero
  buttons, `⋯` overflow with Save / Upload / Paste + Settings), a
  48 px left tool rail (forest-mid, gold active chip with a spring
  pop, lock-grey gated tools), a 236 px properties panel
  (`{Tool} · Properties` with per-tool collapsible sections —
  Stroke / Fill / Text / Number / Effects / Crop / Selection —
  binding real editor fields), a 30 px statusbar (dims + byte
  estimate, gold per-tool hint, real kbd chips), and the canvas as a
  paper cutout (shadow + cream ring + corner handles, chrome-only).
- **Settings route**: reachable from every route's `⋯` overflow;
  cutout cards for Capture / Editor / Output & saving / Shortcuts /
  System; wells, switches, and dashed readonly rows (hide-from-shot,
  default tool, shortcuts, display server, version). Edits apply live
  and persist through the existing config save + watcher round-trip
  (debounced).
- **Settings config schema** (additive, serde-defaulted):
  `capture.default-mode`, `ui.show-statusbar`,
  `ui.statusbar-size-mode`, `ui.animations`, and `output.*` (format,
  jpeg-quality, filename-template, save-dir). Filename templates
  substitute `{app}` / `{date}` / `{time}` and sanitize to one path
  component; unknown enum values warn and resolve to defaults.
- **Direct save**: when `output.filename-template` is set,
  Save writes `save-dir/template.ext` in the configured format
  (JPEG at the configured quality) with `-2`-style collision
  suffixes; an empty template keeps the file dialog.
- **Shared cutout widgets**: ghost button (hero-key variant), well
  rows, the gold pill switch (locked variant at 55% opacity),
  collapsible sections with spring unfold, kbd chips — all behind a
  single animation gate (`ui.animations`).

### Changed

- The topbar identity reads `Flintshot | Home ▸ Editor` (the old
  `Flintshot | Editor` + a second `Home ▸ Editor` breadcrumb
  duplicated the route and read as two confusing labels). The
  breadcrumb's past stages are real links: gold + underline on
  hover, a click routes the single window Home in-place and closes
  worker editor windows like Esc.
- The floating +/− chips zoom the **canvas view** (a 0.25-step
  multiplier over the fit, clamped 0.5×–4×) instead of stepping
  the magnifier loupe's invisible factor — they visibly did
  nothing. Zoomed worker views stop chasing the image-sized
  window: the fit letterboxes inside the stable window, and
  returning to exactly 1.0× restores the image-sized policy.
- The captured image's paper frame keeps 12 px of padding from the
  canvas panel edge (ring + shadow penumbra no longer sit flush
  against the window chrome; the fit and the worker resize both
  budget it).
- The editor's byte-size readout follows `ui.statusbar-size-mode`
  (`heuristic` `w × h × 4` by default, `exact-png` for a real PNG
  encode, cached per image dims).
- The palette of record moved to the mockup's token table (forest /
  gold / paper families, ink + cream text) with cutout 10 / pod 12 /
  pill radii and four shadow levels (resting / lift / pop / paper);
  the v1 gold and graphite-era neutrals are retired and a source scan
  keeps them out.
- `capture.default-mode` seeds the topbar Capture exit (fullscreen
  skips the selection; window falls back to region).

### Fixed

- **Number tool's digit is centered in its bubble** (spec
  `docs/specs/number-tool/`): the shared render path passed the
  bubble's center to `draw_text`, whose contract anchors a text run
  top-left — the digit sat toward the bubble's bottom-right, in the
  live editor and the saved/copied PNG alike. The label is now
  measured and drawn at center minus half its size, in the shared
  arm, so both canvases center together (WYSIWYG).
- Every topbar / overflow / breadcrumb icon is now a painted vector
  glyph (rail's `Glyph` fn pattern), never font-chain text: the
  cmap scan proved U+22EF / U+29C9 / U+25B8 / U+2913 / U+21E7 /
  U+2713 absent from every embedded font, so those icons rendered
  as tofu boxes. Heroes (Capture's plus-in-ring, Copy's sheets,
  Open's folder), overflow rows (Save / Upload / Paste / Settings),
  the About row, and the breadcrumb chevron all paint now, and a
  tofu-ban scan test fails the build if any banned codepoint
  re-enters `flintshot-editor-egui`'s source.
- The topbar Capture hero was a dead control on the Settings route
  (the enable matrix kept it live; the router's dispatch arm only
  matched Home). A Capture click on Settings now fires the same
  bridge request + close choreography, and the settings page
  restores after the grab resolves.
- The editor's Copy hero now answers: for ~2 s after a successful
  copy it paints the gold check glyph and reads "Copied", then
  reverts on its own (a repaint lands the revert without input).
  The state is time-windowed, not latched, so a fresh editor can
  never inherit a stale "Copied". Both lifecycles feed the hero —
  routed pages (the router queries the live editor) and worker
  windows.
- Worker-editor Copy "worked sometimes" on Linux: a successful copy
  auto-closed the Wayland window, ending the process — and the
  compositor does not reliably adopt an exited client's
  `zwlr_data_control` selection (COSMIC empties it; arboard serves
  the selection from an in-process thread, which is why `wl-copy`
  forks). A worker Copy now never auto-closes on Linux (either
  display server) — the user closes via Esc / X after pasting.
  macOS/Windows keep the close (OS-owned clipboard).
- The home window kept the pre-redesign compact geometry (360×240,
  non-resizable) — the redesigned home (topbar + hero + hcards +
  recents) could not fit and clipped. Home now shares the stable
  page geometry with Editor / Loading / Settings (1024×640,
  resizable); the initial viewport and every Home↔Editor transition
  batch fix together.
- The topbar's `⋯` overflow chip rendered as tofu — U+22EF is
  absent from every embedded font's cmap. The more glyph is now
  painted as three vector dots (rail's `pt` glyph space).
- Six alignment/UX regressions reported against the editor (all
  regression-tested headlessly):
  - the canvas `CentralPanel`'s default 8 px inner margin anchored
    draws at the panel's inner rect while pointers/chrome mapped
    against the outer one — annotations landed 8 px off the cursor
    and the dressing sat 8 px off the image (both now share one
    zero-margin rect);
  - the paper shadow (`Shadow::as_shape`) fills its whole rect with
    a translucent tint, so painted over the capture it read as an
    overlay — it now paints under the render pass, leaving only the
    penumbra;
  - the dressing (ring + hairline) bled over the rail / properties
    panel / statusbar when the canvas matched the image — the fit
    and the worker-window resize now budget the 8 px ring, and the
    dressing is clipped to the canvas panel;
  - the decorative corner handles read as broken controls — removed;
  - the Stroke section's swatches and S/M/L chips stacked one per
    row (the section body is a justified column) — they flow in
    wrapping rows again;
  - the section collapse chevron pointed left/up — apex right when
    closed, rotating to down when open.
- The app-window clipboard-marker test asserted the pre-grab
  clipboard snapshot was empty — it now tolerates a session
  clipboard that carries an image (dev machines fail it otherwise).
- The editor hid the captured image: the task-008 paper-cutout ring
  filled the whole expanded rect (image included) with opaque cream,
  and a second inside stroke ate the capture's border. The ring is
  now a stroked band strictly outside the image rect, masked by a
  regression test (`canvas_dressing_never_covers_the_image`).
- `flintshot-core` re-exported the task-011 config helpers
  (`estimate_size_bytes`, `SizeMode`, …) only in the working tree —
  the committed tree did not compile. The re-exports are committed.
- Vacuous leftovers from the ux-redesign effort: the statusbar
  hide-the-footer test lost its assertion to debug prints, and the
  floating-chip module carried a `DBG` eprintln plus an
  assertion-less debug test. Both are real tests again.

### Removed

- The quiet header bar (tab-strip + AI seams), the side panel's tools
  card, export island, copy hero/square, `+` capture popup, and the
  collapse-to-rail mode — every action they hosted moved to the
  topbar, overflow, rail, or Selection section.

### Added

- **Collapsible editor side panel**: a `«`/`»` toggle in the
  panel's action row collapses the docked panel to a 44 px glyph
  rail (every tool, `+`, and the export/copy actions stay live;
  tooltips and ids unchanged) and back. The worker window's
  image-fit resize follows the live panel width, so a collapsed
  rail never keeps re-issuing the expanded size.
- **Pop-up book UI — design-token system + editor/home redesign**
  (spec `docs/specs/popup-book-ui/`): one centralized theme
  (`theme/` — cream paper, white cutout cards with 8px borders,
  two-level painted shadows, forest accents, Plus Jakarta Sans
  embedded under OFL, spring easing, procedural paper grain,
  light-only). The editor's top toolbar dissolves into a docked
  right side panel (hover-lift tool pods, contextual per-tool
  options, color/stroke picker popover, Copy-hero export cluster)
  plus floating on-canvas chips (undo/redo, magnifier toggle,
  loupe zoom). The home window becomes a welcome screen: forest
  hero capture card, persistent recents grid
  (`flintshot-core::recents` — paths+mtimes in
  `<config_dir>/flintshot/recents.toml`, capped at 20, dead paths
  pruned, thumbnails never persisted), Open/Paste cards, and
  drag-and-drop. A quiet header bar spans every surface with an
  inert one-tab strip, a hidden AI slot, and an overflow menu.
  Shape tools gain a fill toggle (`EditorState::fill_shapes`,
  default outlined-only). All shortcuts, the ToolGate phase flow,
  and the ADR-0047 capture choreography are preserved unchanged.

### Changed

- **Editor side panel reads as one docked sidebar** (revises
  `popup-book-ui` task 005's per-pod cards): the five tool pods
  are now rows of a **single** white cutout card separated by
  hairlines, replacing the seven disjointed shadowed islands; the
  per-pod hover-lift microinteraction is gone with them.
- **One window for everything — `flintshot app` routes instead of
  spawning** (ADR-0053/0054, specs `docs/specs/single-window/`,
  `docs/specs/editor-capture-ux/`): the app window is a page router
  (three-button home ↔ fitted editor ↔ capture-progress loading),
  captures run in its own process (no worker spawn, no image relay),
  and external triggers — hotkeys, tray, `flintshot capture`,
  `flintshot open`, tray *Open last* — forward into the running
  window when one exists (`Event::CaptureTriggered` /
  `OpenRequested`). With no window running, every path behaves
  exactly as before (worker editors, byte-identical argvs).
- **Page transitions keep the window** (ADR-0054): home → editor
  (Open…/Paste), editor → home (`Esc`), image swaps, and the
  loading → image hand-off all swap routes inside one window
  lifetime — same surface identity to the WM (no taskbar / pinning /
  open-close churn), one resize per home ↔ editor hop. Window
  destruction now happens only where it must: every capture
  (not-in-the-shot, ADR-0047) and user quit.
- **The editor opens at a stable size**: a fixed 1024×640 logical
  default (clamped to small monitors at open) instead of resizing to
  every image's pixel dimensions; the image renders fitted (never
  upscaled) and centered inside, refitting live on window resizes
  and crops. Annotations, the magnifier, and move-chrome track the
  fit through one shared view transform.
- **Capture progress indicator**: after confirming a selection in
  the OS UI, the editor window is on screen immediately — spinner,
  "retrieving capture…" — while the portal's clipboard write lands
  (observed over 1 s); the image swaps in-place the moment it
  delivers. Direct grabs (image in hand) skip the spinner; a wedged
  read-back surfaces its timeout error on the page and `Esc`
  returns home — never a hang, never a stranded spinner.
- **The editor `+` popup mirrors home**: Capture / Open… / Paste
  with the home-parity labels and the Replace-semantics hints on
  Open/Paste (the retired Region/Fullscreen entries and the
  duplicate chrome strip are gone — OQ-1; `C` / `O` keyboard
  accelerators stay, `Ctrl+N` keeps the popup's Capture). On the
  dev Wayland+portal session `+` → Capture opens the compositor's
  own selection UI (`backends` reports portal available and
  auto-selected; the xcap monitor-grab fallback runs only when the
  portal is genuinely unavailable).
- **Paste**: the home's third button (and the editor chrome's) reads
  the system clipboard image straight into the editor page; an empty
  or text clipboard is a status-line note, not an error.
- **The editor stays open after Save / Copy / Upload**: side effects
  apply (file written, clipboard set, upload spawned + reported) and
  the page keeps the image — Capture / Open… / Paste are reachable
  from the editor chrome (`C` / `O`; `P` stays the Pencil tool key),
  `Esc` returns to the home, window X exits the app (daemon
  unaffected). New content replacing an image drops unsaved
  annotations (the existing Replace/supersede semantics).
- **CLI `open` delegates**: `flintshot open <path>` hands off to the
  daemon (forwarded into the window, or the open worker when none).
  The daemon-spawned open worker carries `--standalone` — the same
  loop-breaker rule capture argvs always had; humans can pass it as
  the debugging escape hatch.
- **Breaking — one Capture action replaces `region` / `fullscreen`**
  (ADR-0051, spec `docs/specs/native-capture-ux/`): `flintshot
  capture` is the single Capture action on every surface — the OS's
  own selection UI where the selected backend supports it (portal
  *interactive* on Wayland; Enter returns exactly the selection,
  Escape is a quiet exit), else today's monitor-grab → editor/overlay
  fallback, byte-identical. Removed with no aliases (0.x, all
  consumers ship together): the `region` / `fullscreen` subcommands
  (now "unrecognized subcommand", pinned by test), `--monitor`
  (selection is the OS UI's or the editor's job), the
  `Request::CaptureRegion` / `CaptureFullscreen` IPC variants
  (replaced by `Request::Capture { delay_ms }`), the tray's
  *Capture region* / *Capture fullscreen* items and per-monitor
  submenu (one **Capture** item), and the COSMIC applet's
  per-monitor entries (one **Capture** action).
- **Editor Copy closes the window on Wayland sessions**: the
  stay-open-after-Copy rule existed solely for X11's owner-based
  clipboard (the process must keep serving the selection); under
  Wayland's compositor-owned `zwlr_data_control` the selection
  survives the process, so the editor closes immediately after a
  copy (Flameshot/Win11 semantics) instead of lingering for a rule
  the session never needed. X11 behavior is unchanged.
- **Breaking — `[hotkeys]` collapses to one binding**: `capture =
  "Ctrl+Shift+PrintScreen"` (the former `region` default) replaces
  the `region` / `fullscreen` pair on X11/macOS/Windows and as the
  portal bind suggestion (one shortcut, id `capture`). Legacy
  `region` / `fullscreen` keys in an existing config load with a
  warning naming the rename and bind nothing; pre-collapse portal
  bindings need one re-bind (`flintshot hotkeys bind`). The daemon's
  Wayland advice text now points at `flintshot capture`.

### Added

- **Post-grab routing** (`after_grab`): `[capture] after_grab =
  "editor" (default) | "clipboard" | "upload"` (CLI `--after`) —
  native-path grabs can land on the clipboard (`clipboard` copies
  immediately — notification, paste-ready — then opens the
  editor/overlay pre-loaded with the same grab, so every backend
  lands in the editor after a grab) or upload via the default
  backend (window-less; URL printed + copied). Unknown values warn
  and resolve `editor`. The fallback path ignores it: selection
  happens in the editor there, and a raw monitor dump to the
  clipboard is never right.
- **Home window — *Open* control** (`flintshot app`): a native file
  dialog picks an image and opens it in a `flintshot open` worker
  (its own process — native event loops do not nest); the home
  window stays open. The two capture buttons + monitor picker
  collapse into one prominent **Capture** control (`C`
  accelerator); a fully-built *Paste* control stays hidden behind a
  compile-time const for the future clipboard→editor feature.
- **`--refresh` flag on `just daemon` / `just dev` / `just install[-user]`**:
  full-refresh dev loop. `daemon` / `dev`: rebuild with the tray
  feature (`FEATURES=` overrides, `FEATURES=''` minimal, same
  convention as `just install`), then restart any running daemon
  (`--replace` appended when not already given; in `dev`, only when
  the target is `daemon`). `install[-user] --refresh` is the
  single-command update: release build + redeploy + daemon restart
  from the installed binary (detached, logging to
  `~/.local/state/flintshot/daemon.log`; under elevation the restart
  is skipped with the command to run from the session instead).
  Plain invocations are unchanged.

- **Capture backend selection**: the capture subcommand carries
  `--backend <BACKEND>` (`auto`, `xcap`, `portal` on Linux; unknown
  ids are a hard CLI error naming the valid set), and config gains
  `[capture] backend` (default `"auto"`; unknown values load and
  re-save verbatim, warning at selection time). Resolution is CLI >
  config > auto-detect by session kind, with `xcap` as the
  never-failing fallback; the chosen backend id is logged on every
  grab. Selection groundwork for the native Wayland portal backend
  (spec `docs/specs/native-capture-backends/`).
- **Portal interactive selection** (Linux): `Capturer` gained a
  selection capability (`supports_selection` / `capture_selection`);
  the portal backend implements it with `interactive: true` — the
  compositor's own select-and-confirm UI (Spike C-verified on COSMIC:
  Enter returns exactly the selected region, Escape surfaces the
  existing `"cancelled"` error as a quiet exit; a long Response
  budget with a quiet hard timeout covers the COSMIC frontend's
  wedged-session caveat, and grab files are read-then-deleted
  wherever the frontend writes them).
- **Portal capture backend** (Linux): the XDG Screenshot Portal
  backend is live — prompt-free on Wayland/COSMIC (~75–120 ms
  end-to-end), so `auto` now resolves to `portal` before `xcap` on
  Wayland sessions when the portal is up. Bounds every D-Bus call
  (  5 s) so a wedged portal errors instead of hanging; response code
  1 surfaces as a `"cancelled"` capture error. Grabs are always
  cursor-less (the portal protocol has no cursor option).
- **`flintshot backends`**: new diagnostic subcommand — lists every
  registered capture backend with its probe status (and failure
  detail), plus what `auto` selects right now. Probes only, never
  grabs; always exits 0.

- **New capture from the editor** (Win11 Snipping Tool-style): a
  `+` chip leading the editor/overlay toolbar rail (popup: *Region
  · Ctrl+N* / *Fullscreen*) and the `Ctrl+N` chord start a fresh
  capture from inside a session. The editor window closes first —
  it can never be in the new shot — and the same worker process
  re-captures and reopens (region re-enters selection-first), so
  an app window hiding for the chain restores only when the whole
  chain ends (ADR-0046, spec `docs/specs/editor-new-capture/`).

- `flintshot app` — a Win11-Snipping-Tool-style app window: *Capture
  region* / *Capture fullscreen* controls (plus a monitor picker and
  a daemon status line). The window is an IPC client that
  auto-starts the daemon when none is running, hides itself for the
  grab, and restores when the capture session ends — including
  crashed workers (`SessionEnded` is sourced from the daemon's
  worker reaper). A second `flintshot app` (or the tray's new *Open
  app window* item) focuses the existing window instead of stacking
  another (ADR-0045, spec `docs/specs/app-window/`).
- IPC: `Request::OpenApp`, `Event::SessionEnded`, `Event::AppFocus`
  (appended enum variants; postcard wire bytes unchanged for
  existing variants).

### Fixed

- **Bottom cluster layout bug in the editor side panel**: the
  export pod island inside the panel's bottom-up layout stretched
  to fill all remaining height (up to a third of the window,
  overlapping the history island) — an `egui::Frame` in a
  `bottom_up` layout grabs the whole remaining column. Delete /
  Save / Upload now share one exact-height island row with the
  Copy hero pinned below, pinned by a regression test at multiple
  window heights.
- **Confirming the OS screenshot UI with Copy no longer kills the
  capture** (spec `docs/specs/portal-clipboard-uri/`): COSMIC's
  portal answers a Copy-confirmed grab with `clipboard:///` — the
  image is on the system clipboard, no file exists — which
  flintshot rejected as a non-file-uri error, ending the capture
  invisibly (the failure only reached the daemon log), so the
  editor never opened. Two stacked causes, both fixed: (1) the
  portal sends its `clipboard:///` response *before* its own
  clipboard-write task runs (queued behind the overlay teardown —
  observed landing ~1 s later), so flintshot snapshots the
  clipboard beforehand and waits up to ~5 s for an image that
  *changed* (a stale image already on the clipboard is never
  mistaken for the grab; a byte-identical one at the deadline is
  accepted); (2) **arboard had no native Wayland support** —
  without the `wayland-data-control` feature it is X11-only and
  rode XWayland's clipboard bridge, whose Wayland→X11 sync is
  lazy/focus-gated, so reads failed nondeterministically even
  after the write landed. The workspace now enables
  `wayland-data-control` (and declares `image-data`, which the
  app's clipboard code always needed — `--no-default-features`
  builds compile again). Every post-grab action (`--out`, editor,
  clipboard, upload) proceeds as with a file-delivered grab;
  Save/Enter-confirmed captures are unchanged. The tray also
  restores at pixel-time now — the pre-grab guard no longer spans
  the clipboard wait.
- **App-window capture races** (ADR-0052, spec
  `docs/specs/fix-app-window-capture-races/`) — the "window in the
  shot" guarantee now holds for every trigger and every app-window
  lifecycle: (1) the pre-grab handshake learns about
  **directly-launched** app windows (`flintshot app` auto-starts the
  daemon, so the registry never saw them) via a parked presence
  connection (`Request::AnnounceAppWindow`) whose liveness **is**
  the signal — the handshake waits for their hide-ack instead of
  blessing the grab on a 250 ms grace alone; (2) the app window's
  ack now reads *current* window state (`window_open`) instead of
  consuming a one-shot "destroyed" mark that a later handshake
  could consume stale while a fresh window was open.
- **A dropped window-close no longer wedges the capture flow**: the
  home window latches its Capture click and re-issues
  `ViewportCommand::Close` after 500 ms (warned, idempotent) if the
  compositor dropped the first one — previously the armed capture
  intent sat stuck until the user closed the window by hand.
- **The tray icon returns the moment pixels are taken on the native
  path**: the prepare guard's scope is now the grab only (the
  fallback path's shape), not the whole editor session — annotating
  for minutes no longer hides the tray (and the cancel affordance
  with it) while no capture is in flight.

- **No flintshot surface lands in a capture anymore, no matter who
  triggered it** (ADR-0048): a grab now clears *every* flintshot
  window first — the daemon's pre-grab handshake
  (`PrepareCapture` → `CaptureReady`) closes the app window
  (externally triggered captures included, closing the ADR-0047
  gap), closes and re-spawns pinned images at their exact
  position/size/opacity after the grab, and drives the tray hide
  (ADR-0036). Previously only the *initiating* surface hid, so a
  tray/hotkey capture with an editor, overlay, app window, or pin
  on screen froze that window into the shot.
- **Overlapping capture triggers no longer stack editor windows**
  (ADR-0048): the daemon tracks live capture sessions
  (process-exit truth, kill handles) and every new trigger
  **supersedes** the in-flight one first — the old editor/overlay
  closes gracefully (cancelled, no partial saves), a bounded wait
  falls back to a hard kill, and only then does the new capture
  spawn. One annotation surface at a time; newest intent wins.
- **App-window captures no longer include the app window on
  Wayland** (ADR-0047): winit's Wayland `set_visible` is a no-op,
  so the hide-based strategy never worked there (reported live on
  COSMIC). The window now **closes** for the capture — surface
  destruction is causal on every compositor — and the process
  reopens it when the capture session ends (status line preserved,
  uniform across all platforms).

### Changed

- **Both toolbars are organized into grouped pods** (spec
  `docs/specs/toolbar-groups/`): the editor rail's flat chip row and
  the overlay's floating toolbar become semantic islands — capture
  `+` (editor only) · selection (Move, Select) · shapes (Rect,
  Ellipse, Line, Arrow) · freehand (Pencil, Marker) · labels (Text,
  Number) · redact (Blur) · history (Undo, Redo, Delete) · outputs
  (Save, Pin, Upload, Copy) — each a rounded graphite container one
  step off the panel, separated by gaps wider than the intra-pod
  chip spacing; the overlay's accept row (✓ / ✕ / outputs) reads as
  two islands. Pure layout: same chips, glyphs, shortcuts,
  tooltips, and actions; chip behavior is unchanged. The overlay
  toolbar re-measures and re-anchors per frame, so the wider pod
  rail still clamps fully on-panel beside narrow selections.
- `Event::EditorOpened` is now a **real window-open signal**: the
  worker that runs an editor/overlay window reports it just before
  the window opens (one event per actual window — fresh capture,
  recapture reopen, `open`; never for `--out` headless captures).
  Previously the daemon broadcast it optimistically at worker
  spawn, before any window existed (ADR-0048). IPC subscribers
  (COSMIC applet) see the same event, at the truthful moment.
- The `.desktop` launcher entry now runs `flintshot app` (was
  `flintshot daemon`) — the window auto-starts the daemon, so
  launching Flintshot from the app menu opens the capture home.
  `StartupWMClass` updated to the app window's class
  (`flintshot`). The autostart entry still launches the daemon.

### Fixed

- **The tray menu no longer appears in tray-launched screenshots on
  COSMIC** (ADR-0042): the tray icon now lives in a supervised
  helper process (hidden `flintshot tray` subcommand). Capture/quit
  clicks — and every capture's hide handshake — **exit the helper**,
  whose process exit is the only causal SNI unregistration, and the
  one event that makes the panel destroy the open menu popup
  (COSMIC never closes it on activation; upstream
  `TODO: Close menu?`). The daemon respawns the helper right after
  the grab. Timing semantics are unchanged (ADR-0036/0037 graces);
  the hide action changed meaning from a `set_visible` flip to
  stopping/respawning the helper. The daemon no longer links the
  GTK/GLib tray path — the helper owns registration, the SNI
  watchdog, and GLib pumping, and exits when the daemon dies.
- **COSMIC applet popup no longer lands in applet-launched
  captures** (ADR-0041): clicking *Capture region* / *Capture
  fullscreen* / a *Fullscreen on monitor* entry in the applet popup
  now destroys the popup at the click and waits a 250 ms settle
  window before the request reaches the daemon, so the worker grabs
  a popup-free frame (click-to-grab ≈ 600 ms, matching the tray
  path). Non-capture clicks (pin/open/quit) keep the popup open for
  their live status line.

### Added

- **Select-then-delete for a single annotation** (annotation-delete
  effort, ADR-0044): with the Move tool, a click on a committed
  annotation selects it — a dashed ember bounding-box outline that
  stays after pointer release (click or post-move alike; a
  re-grab of another annotation replaces it). `Del` (and
  `Backspace` outside text editing) or the rail's new **Delete**
  button — beside Undo/Redo on both the editor toolbar and the
  overlay's floating rail, greyed until something is selected —
  removes exactly that annotation: an `anns`-only undo snapshot,
  so `Ctrl+Z` restores it and `Ctrl+Shift+Z` re-deletes, with
  every other annotation and the number counter untouched.
  Selection drops on empty-canvas click, `Esc` (first press
  deselects, a second cancels the editor), tool switch, undo /
  redo, and crop; the outline is editor chrome and never reaches
  Save / Copy / Pin / Upload output. Deleting during a text draft
  is impossible (the modal text mode owns `Backspace`), and the
  selection-first phase leaves `Del` inert. See
  `docs/specs/annotation-delete/`.
- **The selection phase now locks the annotation tools until the
  region is committed** (select-first-tool-lock effort, ADR-0043,
  revising ADR-0035): in the editor window's selection-first
  phase, every rail chip except Select renders greyed and
  unclickable with a "locked — Enter crops to unlock" tooltip,
  tool-switch hotkeys are inert, and annotation presses anywhere
  on the canvas are denied — the region is the phase's only job.
  Cropping (Enter or the toolbar's *Crop*) unlocks the full rail,
  hotkeys, and input permanently. In the overlay, pressing a
  non-Select tool key before any usable selection now does
  nothing (previously it invisibly armed the tool); once a
  selection exists, keys and the floating rail work exactly as
  before. `fullscreen`, `open`, `pin`, and post-crop editor
  sessions are unregressed.
- **Flip to Move after every annotation commit** (prominent-move
  effort, ADR-0040): releasing a drawn annotation (Rect / Ellipse /
  Line / Arrow / Marker / Pencil / Blur, and Text on modal commit)
  switches the tool to Move, so the very next press can reposition
  what was just drawn — draw, release, drag it better, no tool
  switch, no undo. Re-press the tool's hotkey (`R`, `E`, `A`, …) to
  draw again. Number is exempt (rapid sequential labeling), Select
  and Move produce no commits, degenerate (discarded zero-extent)
  strokes never flip, and the flip itself creates no undo entry —
  `Ctrl+Z` after a flip restores annotation state while the tool
  stays Move. Controlled by the new `[ui] flip-to-move` config key
  (`true` by default; `false` restores sticky tools for rapid
  multi-draw). The flag is read once per editor session (magnifier
  precedent): a toggle applies to the next capture's editor, not a
  live session. An ownership guard keeps the flip from clobbering a
  deliberate tool switch: a text draft flushed *because* the user
  already changed tools does not flip.
- **Hover affordances under Move, in both shells**: hovering a
  committed annotation with Move active shows the Move cursor plus a
  thin dashed bounding-box outline (from `Annotation::bounding_box`,
  whose doc promised exactly this); a live grab adds a solid ember
  outline tracking the dragged annotation. Hovering empty canvas
  shows neither; hovers are read-only queries that never touch the
  dispatcher or history. The editor window gains hover cursors at
  all (the Move grab cursor is its first). See
  `docs/specs/prominent-move-tool/`.
- **Move tool** (`V`, a dedicated toolbar chip in both the editor
  window and the fullscreen overlay): pressing on a committed
  annotation grabs it and dragging repositions it, for every
  annotation type. The grab point tracks the cursor; size, style,
  and z-order are untouched. One move gesture is exactly one undo
  entry (the snapshot is taken on first actual movement, so a
  click-without-drag records nothing); `Ctrl+Z` restores the
  pre-move position, `Ctrl+Y` re-applies it, and undo / crop
  mid-drag cancels cleanly. Selecting and moving are separate
  tools: the Select tool (`S`) now only drags region selections
  (a press on an annotation starts a fresh selection over it), and
  all annotation repositioning happens through Move. In the
  overlay, a Move press always reaches the dispatcher so
  annotation grabs work anywhere, while region handles and
  interior drags remain Select-owned (spec:
  `docs/specs/annotation-move/`).

### Changed

- **Move is the default edit-phase tool**: `Tool::default()` is now
  Move, so a plain editor window (`fullscreen`, `open`, …) opens on
  it, and the edit phase after a region capture's crop lands on Move
  (shell-side: the region-selection phase still opens on Select per
  the region-first contract, then flips Select → Move when the phase
  ends — a tool the user picked mid-phase survives). The rail in
  both the editor toolbar and the overlay's floating toolbar now
  leads with Move, rendered as a **labeled chip** (glyph + "Move"
  text, the Copy primary's glyph+label pattern) so the tool teaches
  itself without a tooltip.

### Fixed

- **`Enter` did nothing after selecting a region in the
  selection-first editor window** (the Wayland demotion path of
  `flintshot region`): `EditorApp::logic`'s key translation chain
  forwarded only `Esc` / `Z` / `Y` / `S` / `C`, so the dispatcher's
  Enter-crops-selection branch was reachable solely through the
  toolbar's "Crop · Enter" button — the shipped tests dispatched
  `Key::Enter` directly into the dispatcher and never caught the
  missing shell translation. Enter now forwards to the dispatcher
  (gated there on Select tool + live selection), and a synthetic-
  `egui::Context` regression test drives the translation layer
  itself. This also restores the Phase 1 Select-tool contract
  (`S`-select + `Enter` crop in the plain editor), which had the
  same gap. The X11 overlay is unaffected (it intercepts Enter
  before dispatch with accept-as-copy semantics).

- **The tray menu duplicated after a capture** (every capture session
  left one more flintshot icon + menu in the panel, clearing only on
  daemon exit). The ADR-0038 unregister/re-register hide could never
  work: `tray-icon`'s Linux drop only flips the appindicator to
  `Passive` (the GObject leaks, nothing unregisters), SNI has no
  unregistration method — COSMIC's watcher removes an item only when
  the owning process's bus name disappears — and each rebuild mints a
  fresh `{pid}-{counter}` object path, so every hide/restore cycle
  registered one *more* permanent panel item. The daemon now holds
  exactly **one** `TrayIcon` for its whole life under a stable id, and
  the gate drives it with `set_visible` (`Active`/`Passive`) flips;
  the ADR-0036/0037 handshake timings are unchanged (ADR-0039). The
  never-closing COSMIC menu popup is accepted as an upstream
  `cosmic-applets` limitation — no daemon-side lever exists for it.

- The tray **menu popup** still landed in *every* tray-launched
  capture on COSMIC despite the ADR-0037 menu grace (L5, 2026-09-03):
  `cosmic-applet-status-area` never closes an SNI menu popup on item
  activation (an upstream `TODO`), and it ignores SNI `NewStatus`, so
  no wait duration or status flip could remove it. ADR-0038's
  unregister/re-register attempt to close it causally proved
  impossible (see the bullet above / ADR-0039): the popup's presence
  in tray-launched captures on COSMIC is an upstream limitation; on
  panels that close popups on activation (native macOS/Windows
  backends, conforming SNI panels) the ADR-0037 grace already covered
  the close.

- The daemon's own tray icon — and the tray **menu** the click
  dismissed — landed in every daemon-dispatched capture's frozen
  frame (tray clicks, hotkeys, CLI delegation, applet / desktop
  actions — L5, 2026-08-31). A worker→daemon handshake now spans
  exactly the grab (ADR-0036/0037): before capturing, the worker
  asks the daemon to hide its icon (SNI `Passive` + the GLib pump)
  and waits for an ack that only leaves after the panel has
  actually re-rendered — 250 ms for the icon repaint, 600 ms
  click-anchored on tray-menu launches because the panel's popup
  close (DBusMenu round trip + spring animation) is unobservable
  and outlasts the repaint. The instant the pixels are taken —
  success or failure — a best-effort report restores the icon.
  The icon vanishes at the tray click itself (pre-hide), concurrent
  captures share a depth counter, a killed worker self-heals via a
  30 s safety bound, every timeout degrades to "capture anyway"
  rather than hanging, and standalone no-daemon captures cost one
  socket probe. Daemon-triggered captures gain ~250–400 ms
  click-to-grab (~600–700 ms from the tray menu) — the panel
  repaint the guarantee depends on.

- Tray menu clicks appeared to do nothing (any item, any click). One
  click on *Quit* was enough to kill them all: the event loop broke
  for the shutdown but the daemon never exited — `run` still held the
  heartbeat channel's receiver, so the tokio side's `send`s kept
  succeeding and the `join()` after the loop waited forever. The
  half-dead daemon kept its tray icon, stopped pumping the GLib
  context, and silently dropped every later menu click (L5,
  2026-08-31; reproduced by driving the tray's DBusMenu `Event`
  exactly as the panel does). `run_event_loop` now owns the receiver,
  so returning from it drops it, the tokio side's next heartbeat
  fails, and the daemon exits cleanly ~300 ms after the click.
  A regression test pins the receiver-drop handshake.

- Tray *Open config* silently failed whenever `$EDITOR`/`$VISUAL`
  carried arguments (`code --wait`): the whole value was passed to
  `Command::new` as one binary name, so the spawn died with "No such
  file or directory" before any window opened. The value is now
  whitespace-split into program + leading args; a blank value prints
  the config path like an unset one.

- The release binary aborted with a core dump on every capture
  (`flintshot region` → *"there is no reactor running"* at
  `zbus-5.18.0/…/executor.rs`): accesskit's a11y event-pump thread
  builds a zbus connection with no ambient tokio reactor, and the
  release profile's `panic = "abort"` turned that thread panic into
  process death — no editor window ever appeared (debug builds
  survived with a scary panic printed, masking the release
  breakage). accesskit's `tokio` executor is unreachable through
  eframe's feature set (egui-winit hard-defaults its `async-io`
  executor, mutually exclusive with `tokio`), so the editor crate
  now builds eframe without the `accesskit` feature; editor /
  overlay / pin windows lose screen-reader support until egui
  exposes the executor choice (ADR-0034, TEST_MATRIX finding 2
  amended).

- Daemon could never restart while an editor from a previous daemon
  was still open: `single-instance` 0.3's Linux lock binds its
  abstract unix socket **without `FD_CLOEXEC`**, so the daemon's
  spawned workers (`region --standalone` editors) inherited the fd
  and ghost-held the lock name after the daemon exited — every
  subsequent `flintshot daemon` (including `--replace`) failed with
  "another flintshot daemon is already running" while no daemon
  existed. Live-reproduced on COSMIC (L5, 2026-08-30): after a
  daemon quit with an editor open, the only recovery was closing
  every editor. Linux now uses a std-only abstract-socket lock
  (`UnixListener::bind_addr`, close-on-exec by construction) in
  `single.rs`; macOS/Windows keep the crate (ADR-0033). With the fix,
  `daemon --replace` acquires the lock ~50 ms after the old daemon
  quits, and a fresh `flintshot daemon` starts cleanly beside a
  live orphaned editor.

### Changed

- `flintshot region` now lands in a **selection-first** editor
  session whenever the editor window is the shell (the default
  `interaction-mode = "editor"` on every platform, `--editor`, and
  the Wayland demotion of config-set `overlay`; see ADR-0035). The
  window opens on the frozen capture with the Select tool pre-armed,
  a spotlight dim mask over everything outside the live selection
  (the overlay's own four-rect mask + `MASK_ALPHA` via shared
  helpers — no second implementation), and a one-line hint ("drag to
  select · Enter crop · Esc cancel"). The first drag draws the
  region; `Enter` (or the Crop button) crops and the session drops
  into the normal editor on the cropped image — the phase never
  returns, not even via undo after the crop. `Esc` during the phase
  cancels the capture (exit 0, no file, no clipboard write, no
  worker). Copy / Save / Pin / Upload honour an active *uncropped*
  selection while the phase is live (Save joins the selection-aware
  outputs via `compose_for_save`, byte-identical to crop-then-Save
  for fully-contained annotations); after the phase ends — and in
  `fullscreen` / `open`, which stay plain — Save keeps its
  full-image Phase 1 contract. `fullscreen`, `open`, and the true
  overlay on X11 / macOS / Windows are otherwise unchanged, pinned
  by dispatch tests.

- `just install` / `just install-user` now build the release binary
  with the `tray` feature on by default, so the launcher entry's
  promise holds: activate it, the daemon starts *and the tray icon
  appears*. This needs the GTK dev libs at build time
  (`libgtk-3-dev` + `libayatana-appindicator3-dev` on Debian/Ubuntu;
  the tray backend links `libgtk-3`, appindicator itself is dlopened
  at runtime); on failure the recipes print that prerequisite plus
  the escape hatch. `FEATURES=` overrides the set — `FEATURES=''`
  installs the minimal no-tray binary (no GTK build deps),
  `FEATURES='a,b'` passes through verbatim. Cargo-level default
  features are unchanged, so plain `cargo build` / CI stay minimal.

### Added

- CI: new `tray-build` job compiles and lints `flintshot-app` with
  `--features tray` (`cargo clippy -p flintshot-app --features tray
  --all-targets -- -D warnings`, GTK + appindicator dev libs installed
  in the job), so the opt-in tray code path can't rot silently between
  manual `just install` builds.

- Desktop launcher entry: after `just install-user` (or
  `sudo env "PATH=$PATH" just install` for a system prefix), typing
  "Flintshot" in the app menu — GNOME Activities, KDE kicker, COSMIC
  launcher — starts the daemon; no terminal needed. Right-click /
  long-press the entry for *Capture region* / *Capture fullscreen*
  desktop actions, and `AUTOSTART=1 just install-user` additionally
  installs the login-autostart entry (`~/.config/autostart`) so the
  daemon is running from your next login on. `just uninstall` removes
  everything either recipe installed, and re-running an install over
  an existing one is safe. Assets live under `assets/` (SVG icon,
  `flintshot.desktop`, autostart variant).
- Desktop-file guardrail test
  (`crates/flintshot-app/tests/desktop_files.rs`): parses both
  `.desktop` files per the freedesktop entry grammar (required keys,
  `Actions=` ↔ `[Desktop Action …]` consistency, autostart variant
  action-free) and asserts every `Exec=` names a real CLI subcommand
  by diffing against `flintshot --help` — renaming or removing a
  subcommand now fails the test suite instead of shipping a dead
  launcher entry.
- Wayland global hotkeys via the xdg-desktop-portal
  `GlobalShortcuts` interface (`ashpd`): on a Wayland session the
  daemon is now portal-first — it skips `global-hotkey` X11 grabs
  entirely (under `XWayland` those can register-but-never-fire) and
  instead attaches silently to whatever shortcuts the portal already
  has bound under the stable ids `region` / `fullscreen`, dispatching
  activations through the same worker path as IPC and tray. Portal
  startup failures are classified (no session bus / portal frontend
  missing / interface too old) and logged as non-fatal notices naming
  the compositor-keybind workaround. Non-Wayland sessions keep the
  existing `global-hotkey` registration unchanged.
- `flintshot hotkeys bind` end-to-end (Phase 6, Tasks 001 + 003):
  the CLI sends `Request::BindShortcuts` over IPC; the daemon opens
  the xdg-desktop-portal bind dialog with one row per capture id
  (`region` / `fullscreen`), each carrying a human description and a
  *suggested* binding converted from the `[hotkeys]` config chords
  (`global-hotkey` accelerator syntax → XDG desktop-entry syntax; a
  chord with no portal form degrades to "no suggestion" at warn — the
  suggestion is never authority). The reply (`Event::ShortcutsBound`
  with the user's actual bindings, or a `bind-cancelled` /
  `bind-failed` `Event::Error`) is written back on the requesting
  connection and printed by the CLI; bindings take effect immediately
  (no daemon restart) and a repeat bind cleanly replaces the session's
  bindings. Non-Wayland daemons answer with a clear
  "Wayland-portal-only" error; no daemon → "start `flintshot daemon`
  first".
- Tray *Bind shortcuts…* item + portal signal refresh (Phase 6,
  Task 004): Wayland trays (built with `--features tray`) gain a
  *Bind shortcuts…* item that opens the same portal bind dialog as
  the CLI (the item is absent on non-Wayland sessions — it would be
  dead there). Re-binds made from the compositor's own settings UI
  arrive as the portal's `ShortcutsChanged` signal; the daemon
  consumes it and logs the new binding set (ids are stable, so no
  re-subscribe is needed). Config hot-reload is now
  platform-asymmetric by design: on Wayland a `[hotkeys]` edit logs
  an info notice — "hotkey suggestions changed; re-run
  `flintshot hotkeys bind`" — instead of re-registering (re-binding
  requires the user dialog), while existing portal bindings keep
  firing; X11 `HotkeyRegistry::refresh` semantics are unchanged.
- New dependency: `ashpd` 0.13 (workspace-pinned with
  `default-features = false, features = ["tokio",
  "global_shortcuts"]` so its async runtime matches the daemon's
  single tokio side thread; `zbus` arrives transitively). Runtime
  requirement for the portal path: xdg-desktop-portal ≥ 1.16 **and**
  a frontend implementing `GlobalShortcuts` (KDE Plasma; COSMIC
  where the packaged `xdg-desktop-portal-cosmic` lists it — some
  distro builds do not). Elsewhere on Wayland the daemon logs the
  classified fallback notice naming the compositor-keybind
  workaround and continues (see ADR-0029).
- `justfile` run-shortcut recipes now cover the full CLI surface:
  `config`, `open PATH`, and `auth BACKEND` join the existing
  `region` / `fullscreen` / `pin` / `upload` shortcuts, and `daemon`
  accepts passthrough args (`just daemon --replace`). All new
  recipes forward extra args after the fixed positional ones.
- Phase 5 Wayland detection + editor-window fallback: the
  interaction-mode resolution chain is now complete — CLI flag >
  `[ui] interaction-mode` > platform. On a Wayland session
  (detected from `WAYLAND_DISPLAY` / `XDG_SESSION_TYPE`, the same
  signals winit keys off), a config-set `overlay` is demoted to the
  Phase 1 editor window with an info log naming the fallback —
  honest UX, never silent — because a fullscreen borderless window
  cannot reliably be raised above other windows or grab input on
  all Wayland compositors. The decision log gains a
  `platform-fallback` source so every capture's resolution is
  legible. An explicit `--overlay` flag is honored best-effort on
  Wayland (with a warning naming the risk, documented in
  `--help`): the fallback protects the default path, not informed
  overrides. macOS / Windows have no Wayland, so detection is
  constant false there and the overlay stays available; standalone
  and daemon-worker captures share the same resolution code
  (workers inherit the session environment).
- Phase 5 overlay output flows: the live overlay's outputs now run
  the real pipelines instead of placeholders. `Enter` (also the ✓ /
  Copy chips and `Ctrl+C`) copies the selection-aware region to the
  clipboard and closes, publishing `CopiedToClipboard`; Save opens
  the native `rfd` dialog and writes a region-cropped PNG
  (`Saved(path)`); Pin and Upload write their temp PNG and spawn
  the same detached workers as the editor window (`Pinned` /
  `Uploaded`). The X11 owner-clipboard rule is adapted to the
  overlay's close-on-accept semantics: the process lingers briefly
  after the window closes, holding the `arboard` handle so the
  selection survives to paste (60 s on Linux; none on macOS /
  Windows where the OS owns the clipboard). The three copies of the
  selection-aware compose (copy / pin / upload) collapsed into one
  shared `compose_selection_aware`; the editor window's Save keeps
  its full-image Phase 1 contract and is otherwise untouched.
- Phase 5 overlay annotation confinement: all ten tools now draw
  inside the selection in the live overlay, Flameshot-style.
  Annotation-tool presses dispatch only when they land inside the
  usable region (a press outside is swallowed whole — its drags and
  release never reach the dispatcher; the Select tool always flows
  for fresh region drags and handle/move gestures), and annotation
  rendering is clipped to the region, so strokes dragged past the
  edge stop painting exactly where the region-cropped output cuts.
  Tool semantics are byte-identical to the editor window — the
  overlay sends the dispatcher the same event sequences, pinned by a
  ten-tools parity test (text/number/blur/marker included); the
  editor window itself never sets a clip and is unchanged.
- Phase 5 floating overlay toolbar: once a selection exists, a
  compact two-row toolbar floats beside it — the ten tool chips and
  undo/redo (row 1, reusing the editor toolbar's chips and glyph
  language verbatim) and the accept row (✓ accept / ✕ cancel, the
  Save / Pin / Upload outputs, and the ember Copy primary; row 2).
  It anchors below the selection's bottom-left, flips above it when
  the space below runs out, and clamps into the viewport (pure,
  unit-tested placement math); it hides during fresh region drags
  and follows the region through resize/move gestures. Toolbar
  presses are swallowed before selection chrome or dispatcher
  dispatch (it floats inside the overlay panel), the hover cursor
  and magnifier stand down over it, and a tool-chip click mid-text
  draft commits the draft first. Copy is Enter-equivalent (Enter =
  Copy per the spec); the Save / Pin / Upload pipelines land later
  in Phase 5 — their buttons are present and their clicks log a
  notice until then.
- Phase 5 overlay selection chrome: the live overlay now layers
  Flameshot-style selection affordances over the dispatcher's
  selection — a dim mask over everything outside the active region
  (the whole screen until one is drawn), 8 grab handles (corners +
  edge midpoints) with per-direction resize cursors, move-by-drag
  inside the region (clamped to the screen), a live `W × H` size
  badge that flips below the region near the top screen edge, and
  the pixel-zoom magnifier shown on hover (no modifier hold in the
  overlay). Handle/move gestures run while the Select tool is active
  and mutate the selection through pure, unit-tested geometry
  helpers; fresh region drags keep flowing through the unchanged
  dispatcher. Annotation tools are untouched — presses with a
  drawing tool active draw, they never grab the region.
- Phase 5 overlay shell MVP: `flintshot region --overlay` /
  `fullscreen --overlay` (and `interaction-mode = "overlay"`) now open
  the real thing — a fullscreen, undecorated eframe viewport showing
  the frozen capture, rendered through the shared `render()` path.
  The overlay opens with the Select tool (first drag draws a region
  selection), drives the unchanged `EditorState` dispatcher for all
  input (single-key tool switching and Ctrl+Z/Y work; the
  egui→dispatcher translation helpers are factored out of the editor
  window for reuse — no second dispatcher), and closes on `Esc`
  (cancel, clean exit) or `Enter` (accept; the Enter→Copy output
  pipeline lands later in Phase 5 — until then an accepted close
  publishes `Cancelled` with a notice). New
  `EguiShell::run_overlay(state)` concrete entry (not a trait
  addition), plus shared `state_from_config` / outcome reporting in
  the CLI so both shells start from the same persisted `[ui]`
  defaults.
- `[ui] interaction-mode` config key (`"editor"` default | `"overlay"`,
  Phase 5 groundwork): selects how a fresh capture is presented. Stored
  as a free-form string like `magnifier-modifier`, so unknown values
  warn and fall back to `"editor"` without breaking config loading.
- `--overlay` / `--editor` flags on `flintshot region` and `flintshot
  fullscreen` (mutually exclusive): per-invocation override of
  `[ui] interaction-mode`. Mode resolution is CLI flag > config value,
  resolved once in `capture_and_dispatch` and logged with its source.
  An explicit flag runs the capture in this process (the daemon IPC
  request cannot carry it). Daemon-spawned workers keep picking the
  mode from their own config.

- Editor toolbar redesign: two-row "rail + bay" layout in
  `flintshot-editor-egui`. The rail holds the ten tool chips,
  undo/redo, and the output actions (Save, Pin, Upload, and the
  ember-filled Copy primary with sheet→check "Copied" feedback); the
  bay shows only the controls the active tool uses (swatches +
  custom color popup, stroke presets + width slider, text typography
  popover, blur-kind segmented control, select dims + crop) plus a
  per-tool hint. All icons are painter-drawn vector glyphs (see
  ADR-0027) — no emoji, no icon-font dependency. New private modules
  `theme` (graphite + ember token set and `Visuals`) and `toolbar`.
- Single-key tool shortcuts in the editor: unmodified
  `S R E A L M P T B N` select Select/Rectangle/Ellipse/Arrow/Line/
  Marker/Pencil/Text/Blur/Number. Modifier chords (Ctrl+S etc.) keep
  their dispatcher meanings; text-editing mode is exempt.
- Undo/Redo toolbar buttons (previously keyboard-only) — they replay
  the same Ctrl+Z / Ctrl+Y dispatch the keypresses produce.
- Workspace scaffolding with four crates: `flintshot-app` (bin, default),
  `flintshot-core` (lib), `flintshot-editor-egui` (lib),
  `flintshot-cosmic` (bin, opt-in).
- `flintshot` CLI surface (`clap`) with subcommands `daemon`, `region`,
  `fullscreen`, `pin`, `upload` — all stubs, tagged with their target
  phase in `--help`.
- `flintshot-core` public API: geometry types (`Point`, `Rect`, `Color`,
  `Stroke`, `Key`), `Annotation` enum, `EditorState`, `Tool`, `BlurKind`,
  the `Canvas` and `EditorShell` traits, and the `UploadBackend` trait
  with supporting types (`ImageMime`, `UploadContext`, `UploadOutcome`,
  `UploadError`).
- `flintshot-core::config`: `Config`, `UiConfig`, `CaptureConfig` types
  with `Config::load()` / `Config::save()` reading and writing
  `<config_dir>/flintshot/config.toml`. Comment-preserving writes via
  `toml_edit`; partial files fall back to documented defaults.
- `flintshot-core::capture`: `capture_fullscreen(monitor)` and
  `capture_region(rect, monitor)` wrapping `xcap`. Captures return
  physical-pixel `RgbaImage`s; HiDPI conversion happens at the shell
  boundary. macOS permission denials surface as
  `FlintshotError::Capture { source }` whose `source` contains the
  substring `"permission"` so callers can detect & guide.
- `flintshot-core::serde_util::color_hex` — `Color` now serializes as
  `#rrggbb` (opaque) or `#rrggbbaa` (with alpha) everywhere it appears
  (config, IPC, annotations). Reading accepts both forms with optional
  leading `#`, any case.
- New `FlintshotError::Config { path, detail }` variant for config
  parse / serialize failures.
- New `FlintshotError::Capture { source }` variant for `xcap` failures
  (permission denied, monitor out of range, no monitors available).
- New `FlintshotError::InvalidRect { rect, reason }` variant for
  degenerate rects passed to `capture_region` (`reason` is `"empty"`
  or `"negative"`).
- Hidden throwaway `flintshot --dump-config` flag that prints the
  resolved TOML (removed in Phase 1 Task 3 when the real `config`
  subcommand lands).
- `flintshot config` subcommand: prints the resolved configuration
  as TOML to stdout (Phase 1 Task 3). Replaces the throwaway
  `--dump-config` flag.
- `flintshot region --out PATH` and `flintshot fullscreen --out PATH`
  flags (Phase 1 Task 3): capture and write a PNG to `PATH`. Default
  when `--out` is omitted is `/tmp/flintshot-<ISO-8601>Z.png` (e.g.
  `/tmp/flintshot-20260721T143022Z.png`).
- Real `run_region` / `run_fullscreen` bodies backed by
  `flintshot_core::capture_fullscreen` (Phase 1 Task 3). The CLI is
  now a working one-shot capture tool; editor wiring follows in
  Task 6.
- `flintshot-core::render::render()` now uploads the background
  screenshot on first frame, caches the texture handle, and blits
  to the full image rect on every subsequent frame (Phase 1 Task 4).
  Annotation dispatch (Tasks 8/11/12/13) will be appended after
  this background-blit step.
- `flintshot-editor-egui::EguiCanvas::upload_image` /
  `draw_image` real implementations (Phase 1 Task 4). Upload goes
  through `egui::Context::load_texture` with
  `TextureOptions::NEAREST` for pixel-sharp screenshots; blit
  converts physical → logical px at `pixels_per_point`. The canvas
  now carries a stable `egui::Context` plus a per-frame `Painter`
  bound by the embedding shell via `EguiCanvas::bind_painter`.
- `flintshot-editor-egui::EguiShell::run_editor` real
  implementation (Phase 1 Task 5): opens a single-window eframe
  app sized to the screenshot, runs the shared `render()` path
  each frame, and returns `EditorOutcome::Cancelled` on `Esc` or
  window-close-via-X. Window title `"flintshot — annotate"`.
  Internal `EditorApp` (`eframe::App`) lives in the new `app.rs`
  module.
- New `FlintshotError::Shell { detail }` variant for editor-shell
  lifecycle failures (eframe startup, GPU surface init, etc.).
  Sits alongside `Capture` / `Config` and follows the same
  `detail: String` field convention.
- `flintshot-editor-egui::EguiShell::run_editor` real
  implementation (Phase 1 Task 5). Opens an `eframe` window titled
  `"flintshot — annotate"`, sized to the screenshot dimensions,
  displays the screenshot via the shared `render()` path, and
  returns `EditorOutcome::Cancelled` when the user presses `Esc`
  or closes the window. Outcome is propagated through a shared
  `Rc<RefCell<Option<Result<EditorOutcome, FlintshotError>>>>`
  slot because `eframe::run_native` consumes the `Box<dyn App>`
  and fields cannot be recovered by value after the event loop
  ends. `Rc<RefCell<…>>` (not `Arc<Mutex<…>>`) because eframe's
  native event loop is strictly single-threaded.
  `flintshot-editor-egui` now has a 3-module layout:
  `canvas.rs` (`EguiCanvas`, unchanged from Task 4), `app.rs`
  (private `EditorApp` `eframe::App` impl), and `shell.rs`
  (`EguiShell` `EditorShell` impl + `run_editor_blocking`
  helper). `lib.rs` is now a thin module-export file.
  Annotation tools, toolbar, and keyboard shortcuts land in
  Tasks 7+ — Task 5 is the "de-risk egui integration" slice.
- New `FlintshotError::Shell { detail }` variant for editor-shell
  / window-system failures (Phase 1 Task 5). Currently wraps
  `eframe::Error` via `to_string()`; will be reused for Task 9's
  `rfd` / `arboard` failures and Phase 3's pin window.
- `flintshot region` / `flintshot fullscreen` now route through the
  `EguiShell` editor when `--out` is omitted (Phase 1 Task 6). Capture
  → `EditorShell::run_editor(state)` → log the outcome. The editor
  path is gated on the `egui` Cargo feature; `--no-default-features`
  builds reject `region` / `fullscreen` without `--out` with a clear
  error ("editor not built; pass --out PATH to capture headlessly").
  Outcome logging: `Saved(p)` logs the path, `Cancelled` logs and
  exits cleanly with no output file, `CopiedToClipboard` / `Uploaded`
  are wired for Tasks 9 / 4, and the `#[non_exhaustive]` wildcard arm
  warns on future variants.
- `flintshot-core::EditorState` gains a `pending: Option<Annotation>`
  field plus the state-machine methods `begin_annotation`,
  `update_annotation`, `commit_annotation`, `cancel_annotation`,
  `undo`, `redo`, and `hit_test` (Phase 1 Task 7). The pending
  annotation holds the in-flight drag-based tool output; the shared
  `render()` path now draws it on top of `anns` so the user sees live
  feedback. `undo()` clears `pending` (Ctrl+Z mid-drag cancels the
  in-flight stroke, matching typical editor semantics).
- `flintshot-core::Annotation` gains `bounding_box() -> Option<Rect>`
  and `contains(point: Point) -> bool` methods (Phase 1 Task 7).
  These underpin `EditorState::hit_test`, which returns the topmost
  annotation under the cursor — used in Phase 1.F's select tool.
  `contains` uses stroke-width-aware tolerances so thin lines remain
  selectable.
- `flintshot-core::render` now exposes the public `MockCanvas` struct
  and `DrawCall` enum (Phase 1 Task 7) for snapshot tests across the
  annotation tools. `DrawCall` has one variant per `Canvas` method;
  `MockCanvas` records every call into a public `Vec<DrawCall>`. The
  inline `#[cfg(test)]`-only `MockCanvas` from Task 4 is replaced by
  this public version.
- `flintshot-core::render::render()` now iterates `state.anns` and
  dispatches each annotation via the new public `render_ann(canvas,
  ann)` helper (Phase 1 Task 7). `state.pending` is drawn last (on
  top) so the user sees live feedback during drags. All 9 annotation
  variants have a dispatch path; `Blur` renders as a grey placeholder
  + `tracing::warn!` until Task 13 adds real `image_ops::blur`.
- Rectangle annotation tool end-to-end (Phase 1 Task 8):
  `Tool::Rect` is now interactive. Mouse-down inside the canvas
  begins a rect annotation with the active color/stroke; drag updates
  the opposite corner; release commits. An editor toolbar
  (`egui::Panel::top`) appears at the top of the window with a Rect
  tool selector (`selectable_value`) and an annotation-count label.
  `Ctrl+Z` undoes the last commit and clears any in-flight pending
  rect. Click-without-drag is filtered at the shell boundary (cancel
  on degenerate rect rather than commit). `EguiCanvas::draw_rect_outline`
  and `EguiCanvas::draw_rect_filled` now have real egui `Painter`
  implementations using `StrokeKind::Inside` (so the stroke's outer
  edge coincides with the dragged rect — "what you dragged is what
  you get"). Other `Canvas` primitives (`draw_ellipse`, `draw_line`,
  `draw_arrow`, `draw_text`) remain stubs until Tasks 11/12/13.
- Save / Copy output paths (Phase 1 Task 9). Two new toolbar buttons
  in the editor (`💾 Save`, `📋 Copy`) compose the final image
  (background + all committed annotations) and either write a PNG
  via `rfd::FileDialog` (blocking save dialog + `image::save_with_format`
  with PNG forced) or place it on the system clipboard via
  `arboard::Clipboard::set_image`. On success the editor publishes
  the matching `EditorOutcome` variant (`Saved(path)` / `CopiedToClipboard`)
  and closes; on failure it publishes `Err(FlintshotError)` and
  closes (matching the existing render-error pattern). Dialog cancel
  keeps the editor open with no outcome published. Two new workspace
  deps: `rfd = "0.17"`, `arboard = "3"` (default features; X11 on
  Linux, native elsewhere).
- New `flintshot-core::render::compose_final(state: &EditorState)
  -> RgbaImage` (Phase 1 Task 9). Clones `state.image` and renders
  every committed annotation into the clone via the shared
  `render_ann` dispatch — the same path the live editor uses, so
  what the user sees is what gets saved / copied. No toolkit-specific
  "screenshot the canvas" path is involved. Excludes `state.pending`
  (in-flight annotations are not part of the saved output). The
  function backs the Save / Copy buttons and is the source of truth
  for Phase 4's Upload path.
- New `flintshot-core::input` module (Phase 1 Task 10) with three
  public types: `Modifiers`, `InputEvent`, `DispatchOutcome`. The
  shell translates its native input events into `InputEvent`s at the
  toolkit boundary; `EditorState::dispatch_input` walks them and
  returns a `DispatchOutcome` naming the side-effect the shell
  should perform (Save / Copy / Cancel). The dispatcher is pure and
  unit-testable — no `egui::Context` or `eframe` types reach it —
  so a future `iced` / `slint` shell reuses it verbatim. Includes
  the cross-platform modifier mapping: egui's `Modifiers::command`
  (Ctrl on Linux / Windows, ⌘ on macOS) maps onto `Modifiers::ctrl`
  so the dispatcher's chord predicates work uniformly without
  `cfg!(target_os = "macos")` in core.
- Keyboard shortcuts now route through `EditorState::dispatch_input`
  (Phase 1 Task 10): `Esc` (cancel), `Ctrl+Z` (undo), `Ctrl+Y` and
  `Ctrl+Shift+Z` (redo), `Ctrl+S` (save), `Ctrl+C` (copy). The
  pointer flow (down → drag → up) also routes through the
  dispatcher; the zero-area rect filter from Task 8's "deviation 3"
  moved into core so all shells inherit it without duplicating the
  predicate. Existing `EditorApp::logic` is now a thin translator:
  egui input → `InputEvent`, dispatch, `if matches!` on outcome.
- Arrow annotation tool end-to-end (Phase 1 Task 11). Toolbar gains an
  "→ Arrow" button; drag from tail to head commits an
  `Annotation::Arrow` (tail = down point, head = up point, endpoints
  not normalized — direction matters). `head_size` defaults to 10
  physical px; the shaft inherits `state.stroke_width`. New shared
  `flintshot_core::arrow_barbs(a, b, head_size) -> [Point; 2]`
  computes the ±30° barb endpoints so `EguiCanvas` (live view) and
  `ImageCanvas` (`compose_final` → Save / Copy) render identical
  arrows (ADR-0015 WYSIWYG). `ImageCanvas::draw_line` filled in
  alongside; degenerate arrows (sub-px drag) cancel on pointer-up,
  matching the Rect tool's click-without-drag contract.
- Text annotation tool end-to-end (Phase 1 Task 12). Toolbar gains a
  "T Text" button; click places a caret, typing appends to an
  in-flight `Annotation::Text` draft, `Enter` commits, `Esc` cancels
  the draft (stays in editor), `Backspace` deletes the last char. Empty
  drafts never commit. While editing, keyboard shortcuts (`Ctrl+S`,
  `Ctrl+Z`, etc.) are suppressed so the user can type freely; pointer
  events are ignored until the draft is committed or cancelled (Phase 1
  text editing is modal). New `InputEvent::TextInput(char)` carries
  printable-char input from the shell (`egui::Event::Text`) into the
  dispatcher. Multi-line text is deferred to Phase 2.
- `ab_glyph` + `fontdb` added to `flintshot-core` (Phase 1 Task 12).
  `ImageCanvas::draw_text` rasterises text annotations into the saved /
  copied PNG via `ab_glyph` glyph outlines + source-over alpha
  compositing; `fontdb` discovers a system sans-serif face via
  fontconfig (cached in a process-wide `OnceLock`). See ADR-0017 for
  the swap from the originally-specced `cosmic-text` (shipped `ab_glyph`
  instead to keep the highest-risk Phase 1 task achievable — the plan
  pre-authorised this fallback). `EguiCanvas::draw_text` uses egui's
  native `Painter::text` (no new egui-side dep).
- Blur / pixelate annotation tool end-to-end (Phase 1 Task 13).
  Toolbar gains a "░ Blur" button with a Gaussian / Pixelate
  `BlurKind` toggle (visible only while the Blur tool is active). Drag
  selects a region; on commit, `flintshot-core::image_ops::blur` crops
  the region from `state.image`, applies either
  `imageproc::filter::gaussian_blur_f32` (Gaussian, sigma = strength)
  or a block-average (Pixelate, block = strength), and blits the
  result back over the region. New `Canvas::draw_image_region(img,
  dest)` primitive carries the freshly-computed sub-image from core to
  the shell (egui uploads a uniquely-named texture; `ImageCanvas` copies
  pixels in-place). `render_ann` now takes a `source: &RgbaImage`
  parameter so the blur dispatch can crop from the background. New
  `flintshot-core::image_ops` module re-exports `blur` and `crop_region`.
- Select / crop region tool end-to-end (Phase 1 Task 14). Toolbar gains
  a "⬚ Select" button; drag selects a region (rendered with a distinct
  blue outline so it's distinguishable from a committed `Rect`), and
  `Enter` or a "✂ Crop to selection" toolbar button crops `state.image`
  to the selection. Existing annotations translate by subtracting the
  crop origin; annotations fully outside the crop are dropped;
  rect-shaped annotations (`Rect` / `Ellipse` / `Blur`) that partially
  overlap are geometrically clipped to the overlap. Crop is undoable:
  the pre-crop image + anns are captured in a `Snapshot::full` on
  `undo`, and `undo()` / `redo()` restore the image when the snapshot
  carries one. New `Rect::intersect` / `Rect::contains` geometry
  helpers, `Annotation::crop_and_translate` per-variant dispatch, and
  `EditorState::selection: Option<Rect>` field for the in-flight drag.
- Color / stroke-width pickers + config persistence (Phase 1 Task 15).
  Toolbar gains a color swatch (`egui::color_edit_button_srgb`) and a
  stroke-width slider (1–20 px); a font-size slider (8–72) appears
  while the Text tool is active. Changes update `state.color` /
  `state.stroke_width` / `state.font_size` live. On editor close,
  `persist_ui_config` writes the current values back to
  `~/.config/flintshot/config.toml` via `Config::load` + `Config::save`
  (preserves unrelated sections). Config write failures surface as
  `tracing::warn!` — non-blocking, annotation continues. CLI now loads
  `Config::load()` before opening the editor and seeds `EditorState`
  with the persisted defaults.
- Shared error types: `FlintshotError`, `Result` (libs); `color_eyre`
  reporting init in the binary.
- `tracing` + `tracing-subscriber` initialized from
  `RUST_LOG`/`EnvFilter`.
- Integration test pinning the boundary: no GUI toolkit types in
  `flintshot-core`'s public API
  (`crates/flintshot-core/tests/no_gui_types_in_core.rs`).
- CI pipeline (`.github/workflows/ci.yml`): `fmt --check`, `clippy
  -D warnings`, `cargo test`, MSRV (1.95) build, opt-in
  `flintshot-cosmic` build job.
- Project documentation: `README.md`, `docs/VISION.md`,
  `docs/ARCHITECTURE.md`, `docs/ROADMAP.md`, `docs/adrs/`
  (ADRs 0001–0012), `CONTRIBUTING.md`, `AGENTS.md`, `docs/TEST_MATRIX.md`.
- Dual licensing: `MIT OR Apache-2.0`, with both license files in the
  repo root and the `license` field on every crate.

#### Phase 2 — Daemon, tray, hotkeys, IPC

- `flintshot-core::ipc`: transport-agnostic IPC schema
  (`Request`/`Event` enums, `#[non_exhaustive]`, postcard-serde) and the
  `socket_path()` resolver. `Event` uses postcard-native external tagging
  (`#[serde(tag = "kind")]` is `WontImplement` under postcard); see
  ADR-0011.
- `flintshot-core::error::FlintshotError::Ipc { detail }` variant.
- `flintshot-core::config::HotkeyConfig` (`[hotkeys]` section:
  `region`/`fullscreen` chord strings, serde-defaulted) with
  comment-preserving merge, and a public `config_path()`.
- `flintshot-core::capture::monitors()` + `MonitorInfo` (xcap monitor
  enumeration for the tray's per-monitor submenu).
- Workspace dependencies for the resident daemon: `interprocess`
  (local-socket transport, `tokio` feature), `postcard` (binary
  encoding), `global-hotkey` (X11/macOS/Windows hotkeys), `notify`
  (config hot-reload), `single-instance` (daemon-role lock). Resolves
  the `single` vs `single-instance` open question: `single` is an
  unrelated Itertools helper; `single-instance` is the Tauri-affiliated
  single-instance lock.
- `flintshot daemon`: long-running process — tokio side thread + main-
  thread platform loop bridged by mpsc (ADR-0018), single-instance lock
  (daemon role only), `interprocess` IPC server with length-prefixed
  postcard framing, `Request::Quit` graceful shutdown, and `Ctrl-C`
  exit. Proven at runtime.
- `flintshot daemon --replace`: sends `Request::Quit` to the running
  daemon, waits for it to exit, then takes over (ADR-0020). Runtime-
  confirmed it swaps daemons cleanly.
- Worker-spawn model (ADR-0019): the daemon never captures in-process —
  tray/hotkey/IPC capture actions spawn a `flintshot` child (resolved
  via `current_exe`, reusing the Phase 1 CLI) and broadcast
  `CaptureStarted`/`EditorOpened` to IPC subscribers.
- System tray (`tray` feature): *Capture region*, *Capture fullscreen*,
  *Fullscreen on monitor →* (per-monitor submenu), *Open config*,
  *Quit*. Tray failure is non-fatal (compositor without an SNI host).
- Global hotkeys from `[hotkeys]` (X11/macOS/Windows); on Wayland the
  daemon logs the compositor-keybind workaround and continues. Hot-
  reload: editing `[hotkeys]` re-registers without restarting the
  daemon.
- CLI detect-and-delegate: `flintshot region`/`fullscreen` hand the
  capture to a running daemon when one is present (centralising the
  event stream), and fall back to the Phase 1 in-process path
  otherwise. `--standalone` forces the in-process path.
- `flintshot-app` `tray` feature (default OFF): gates the `tray-icon`
  crate, whose Linux backend (`libappindicator`) pulls the full GTK3
  stack that the default CI build doesn't install. The daemon runs
  without it (IPC + hotkeys + single-instance); build with
  `--features tray` to enable the system tray.
- Boundary test extended: `flintshot-core` also rejects `use
  interprocess` / `use global_hotkey` / `use tray_icon` / `use notify`.

##### Notes (Phase 2)

- `Event` uses postcard-native external tagging; the spec/ARCHITECTURE
  `#[serde(tag = "kind")]` internally-tagged form is `WontImplement`
  under postcard (Task 1 found this; kept the schema, dropped the tag).
- Default hotkey chords are `Ctrl+Shift+PrintScreen` /
  `Ctrl+Alt+PrintScreen` — `global-hotkey` names the key `PrintScreen`,
  not `Print` (ARCHITECTURE.md's example updated).
- Wayland global hotkeys are deferred to Phase 6 (`ashpd`
  GlobalShortcuts portal); bind a compositor keybinding to `flintshot
  region` / `flintshot fullscreen` in the meantime.
- The tray's per-monitor submenu is built at tray creation; muda/tray-
  icon don't portably expose a submenu-open event, so hot-plugging a
  monitor mid-session needs a daemon restart (documented limitation).
- Text tool overhaul (see ADR-0021):
  - **Box indicator**: a solid block caret at the insertion point plus
    a hollow bounding box around the in-flight text (replaces the thin
    caret bar). Live-view-only; never reaches Save/Copy output.
  - **Multi-line text** (`Shift+Enter` inserts a newline; plain `Enter`
    still commits). Pulled forward from Phase 2 — both the egui view
    and the `ImageCanvas` save path now lay out multiple baselines.
  - **Decoupled text colour**: the Text tool gets its own colour
    picker distinct from the drawing-tools colour (persisted as
    `[ui] default-text-color`).
  - **Text customisation**: font family (Sans / Mono), weight
    (Regular / Bold), style (Upright / Italic), alignment
    (Left / Center / Right), optional background highlight, and
    optional outline (colour + width). New core types `FontFamily`,
    `FontWeight`, `FontStyle`, `TextAlign`, `TextOutline`,
    `TextStyle`, `TextMetrics`; `Annotation::Text` gained matching
    fields with `#[serde(default)]`. `Canvas::draw_text` /
    `Canvas::measure_text` now take a `&TextStyle` bundle and return
    `TextMetrics` (width / height / last-line width). Bold is real on
    the save path (fontdb weight query) and faux on the egui view
    (egui 0.35 has no weight API); serif family falls back to sans
    until a serif face is bundled.

#### Phase 2b — annotation tools + magnifier

- Ellipse tool end-to-end (Phase 2b Task 001): `EguiCanvas::draw_ellipse`
  renders an `EllipseShape` (outline + optional fill via the egui
  `Painter`); `ImageCanvas::draw_ellipse` rasterizes fill + outline into
  the `RgbaImage` so committed ellipses appear in saved/copied PNGs
  (fixes a latent Phase 1 bug where the no-op silently dropped them).
  Toolbar exposes "◯ Ellipse" alongside "▭ Rectangle".
- Line tool end-to-end (Phase 2b Task 002): toolbar exposes "╱ Line"
  alongside the other shape tools. Drag from one point to another
  commits an `Annotation::Line` with a live rubber-band preview;
  click-without-drag (sub-1px length) is now treated as degenerate and
  cancels rather than committing an invisible zero-length stroke — the
  `finish_pointer_up` guard mirrors the existing Arrow check. Hit-test
  reuses the point-to-segment distance tolerance so thin strokes stay
  clickable.
- Marker + Pencil freehand tools (Phase 2b Task 003). New pure
  `flintshot-core::freehand` module: `smooth_polyline(points, tension)`
  applies a Catmull-Rom spline to raw pointer samples inside
  `render_ann` (pending + committed, so WYSIWYG holds), and
  `MARKER_ALPHA` (`0x66`) is applied to `Annotation::Marker.color` at
  construction so every backend renders a translucent highlight with no
  render-path special-casing. New `Canvas::draw_polyline` method (with
  a `draw_line`-looping default) lets `ImageCanvas` composite the whole
  stroke once per pixel (avoids stamp-compounding a translucent marker
  toward opaque in saved PNGs) and `EguiCanvas` issue one
  `PathShape::line`; `MockCanvas` records a `DrawPolyline`. Toolbar
  exposes "🖍 Marker" and "✏ Pencil". See ADR-0022.
- `Color::with_alpha(a)` helper on `flintshot-core::Color`.
- `Stroke::ZERO` constant on `flintshot-core::Stroke` — a zero-width
  stroke that renders nothing, used by callers that need a `&Stroke`
  for a fill-only shape (e.g. `Canvas::draw_ellipse` with `Some(fill)`
  and no outline, as the Number tool's bubble does).
- Number tool end-to-end (Phase 2b Task 004). Click-to-place dispatch
  path: a `PointerDown` with `Tool::Number` commits an
  `Annotation::Number` immediately (no drag, no `pending`) via the new
  `EditorState::place_number(pos)` entry point, increments
  `next_number`, and pushes a `Snapshot`. The bubble renders as a true
  filled circle via `Canvas::draw_ellipse(bbox, &Stroke::ZERO,
  Some(color))` (equal axes — a circle is an inscribed ellipse; reuses
  Task 001's impl), replacing the square `draw_rect_filled`
  placeholder. Digit text colour is picked by Rec. 601 luminance
  (white on dark fills, black on light) so the number stays legible
  regardless of bubble colour. Toolbar exposes "# Number" alongside
  the other tools.
- Magnifier widget (Phase 2b Task 005). New toolkit-owned
  `flintshot-editor-egui::magnifier` module: a pixel-zoom inset
  (`INSET_PX = 96`, `ZOOM = 4.0`) that samples `EditorState::image`
  directly and paints a cached one-frame texture. Triggered by holding
  a modifier (default `Space`, configurable via `[ui] magnifier-modifier`)
  or always-on-while-drawing (`[ui] magnifier-always-on = true`, default
  `false`). The pure `sample_zoomed_region(image, cursor, zoom, inset)`
  helper is unit-tested headless (center-sample, edge-clamp,
  smaller-than-inset fallback, `zoom < 1.0` clamp). No core abstraction —
  an iced/slint shell implements its own per ARCHITECTURE.md §What stays
  toolkit-specific. Two new `[ui]` config fields (`#[serde(default)]`,
  kebab-case per convention).

#### Phase 3 — Pin-to-desktop + COSMIC applet

- `flintshot pin` scaffolding (Phase 3 Task 001). New `[pin]` config
  section (`flintshot-core::config::PinConfig`: `default_opacity = 0.5`,
  `copy_on_close = false`, `default_position = "center"`; re-exported
  from `flintshot-core`). `flintshot pin <path>` now loads the image,
  resolves window geometry (natural size by default; `--size WxH`
  overrides; active-monitor centre by default, `--at X,Y` overrides),
  merges CLI overrides (`--opacity`, `--no-clipboard`) over `[pin]`
  config, and reaches `EguiShell::run_pin` — still a `NotImplemented`
  stub until Task 2 (logged gracefully, not panicked).
- `flintshot-core::capture::MonitorInfo` gained `x`, `y`, `width`,
  `height` (the monitor's virtual-desktop geometry in physical px) so
  the pin window can centre itself on the active monitor via
  `flintshot_core::monitors()`. Existing readers (tray submenu) are
  unaffected.
- `flintshot pin <path>` now floats a real borderless, always-on-top,
  transparent window (Phase 3 Task 002). `EguiShell::run_pin` opens an
  eframe viewport (`.with_decorations(false)`, `.with_always_on_top()`,
  `.with_transparent(true)`, `.with_resizable(true)`, positioned/sized
  from the `Rect` arg) driven by the new
  `flintshot-editor-egui` `PinApp` (`eframe::App`). The window blits the
  image once (cached texture) and implements all five UX features:
  drag-to-move anywhere (via `ViewportCommand::StartDrag`), close on
  `Esc` or double-click, instant hover-opacity (opaque while hovered,
  `[pin] default-opacity` otherwise), aspect-locked resize via a
  bottom-right handle (`ViewportCommand::InnerSize`), and optional
  copy-on-close to the clipboard via `arboard`. Resolved
  `--opacity` / `--no-clipboard` overrides reach the window through a
  new `EguiShell::with_pin_config` builder (the `EditorShell::run_pin`
  trait signature stays `(image, rect)`).
- Worker save reporting + daemon last-saved state (Phase 3 Task 003).
  New `flintshot_core::ipc::Request::ReportSaved { path }` variant: a
  worker that closes its editor with `EditorOutcome::Saved(path)`
  reports the path back to the daemon (best-effort — silently skipped
  when no daemon answers, so a standalone worker is unaffected). The
  daemon tracks the most recent save in a new
  `flintshot_app::ipc_server::LastSaved`
  (`Arc<std::sync::Mutex<Option<PathBuf>>>`), created in `daemon::run`
  and shared with the IPC server; `PinLast`/`OpenLast` (Task 4) and
  Phase 4 upload-from-last read it back. On receipt the daemon also
  broadcasts the previously-unused `Event::Saved { path }` to
  subscribers. See ADR-0023.
- Daemon `PinLast` / `OpenLast` dispatch + monotonic pin ids (Phase 3
  Task 004). `Request::PinLast` reads the last-saved path from the
  daemon's state, spawns `flintshot pin <path>`, and emits
  `Event::Pinned { id }` with a daemon-assigned monotonic id (via a new
  `flintshot_app::worker::PinIdSource` counter). `Request::OpenLast`
  spawns `flintshot open <path>` to re-edit the last-saved capture. With
  no prior save, both emit `Event::Error { code: "no-last-capture" }`.
  New `flintshot_app::worker::DispatchState` bundles the last-saved
  handle + pin-id counter and is threaded through every dispatch entry
  point (IPC `handle_connection`, tray `drain_tray_menu`, hotkey
  `drain_hotkeys`). New `flintshot open <path>` CLI subcommand loads an
  existing image into the editor (no capture).
- Editor toolbar Pin button (Phase 3 Task 005). A "📌 Pin" button in the
  egui editor toolbar composes the final image (selection-aware, like
  Copy),   writes it to a temp PNG (`flintshot-pin-<pid>-<n>.png` under
  the OS temp dir, persisted for the pin worker to read), and spawns
  `flintshot pin <tmpfile>` as a detached child resolved via
  `current_exe` (mirroring `worker::CommandSpawner`). The editor closes
  on success. New `EditorOutcome::Pinned` variant names the outcome so
  the daemon / future callers can distinguish it. Compose / temp-write
  failures close the editor with an error; spawn failures keep the
  editor usable (Save / Copy still work) so the user can fall back.
- COSMIC applet IPC client (Phase 3 Task 006). New
  `flintshot-cosmic::ipc` module: length-prefixed postcard framing
  (duplicated ~30 LOC from `flintshot-app::ipc_server` per ADR-0010),
  `send_request` (one-shot connect-send-disconnect), `subscribe`
  (long-lived `EventStream`), and `probe_daemon` (`DaemonState` enum
  for running/not-running). Headless-testable against a hand-rolled
  in-process listener (no `flintshot-app` dep). The applet is a pure
  IPC client: depends on `flintshot-core` + `libcosmic` +
  `interprocess`/`postcard`/`tokio` only — no `egui`, no `flintshot-app`.
- Real COSMIC panel applet replacing the stub (Phase 3 Task 007).
  `flintshot-cosmic` is now a `cosmic::applet::run` entry driven by a
  `cosmic::Application` impl (`flintshot_cosmic::app::App`). The panel
  button opens a popup mirroring the tray menu item-for-item: *Capture
  region*, *Capture fullscreen*, per-monitor entries (from
  `flintshot_core::monitors()`), *Pin last*, *Open last*, *Open config*
  (shells out to `$EDITOR` via `flintshot_core::config_path()`, ~10 LOC
  mirror of the daemon's `open_config_file`), and *Quit*. Each action
  resolves through a pure `flintshot_cosmic::menu` dispatch
  (`action_for(&MenuMessage) -> Action`, unit-tested headless) and sends
  the matching `Request` via the Task 6 IPC client. The live event
  stream + daemon-absent status line is Task 8.
- COSMIC applet live event stream + daemon-absent state (Phase 3 Task
  008). The popup now shows a live status line (*Idle* / *Capturing…* /
  *Editor opened* / *Saved \<basename\>* / *Pinned #N* /
  *Uploaded \<url\>* / *Error: \<message\>*, tinted red) driven by a
  persistent `Subscribe` connection. A background `Subscription`
  runs the reconnect loop: probe → subscribe → read events, with a 2 s
  re-probe on stream loss or daemon absence. When the daemon is down,
  the popup shows *"daemon not running — start with `flintshot
  daemon`"* and disables daemon-dependent buttons (*Open config* stays
  enabled). New pure `ipc::status_for_event(&Event) -> String` maps
  each event variant to its status text (unit-tested headless).
#### Phase 4 — upload

- `[upload]` config section (Phase 4 Task 002). New
  `flintshot-core::config::UploadConfig` (`default-backend`, built-in
  default `"imgur"`), re-exported from `flintshot-core` and wired into
  `Config::load` / `Config::save` with the same comment-preserving
  merge as `[pin]`; a missing section falls back to defaults. No
  per-backend sub-tables — credentials live in the OS keychain, never
  in config.
- IPC schema upload variants (Phase 4 Task 003):
  `Event::UploadStarted { backend }` (optimistic dispatch signal,
  mirrors `CaptureStarted`) and `Request::ReportUploaded { url,
  delete_handle }` (worker → daemon result report, parallels
  `ReportSaved`). Postcard round-trip tests extended for both.
- `flintshot-app::upload` module (Phase 4 Task 004) with the first
  concrete `UploadBackend` impl: `ImgurBackend`. POSTs multipart form
  data to `https://api.imgur.com/3/image`; supports anonymous (zero-
  config) and Client-ID authenticated (`Authorization: Client-ID <id>`
  from `keyring`) modes. HTTP-status mapping: 2xx → `UploadOutcome`,
  401/403 → `UploadError::Auth`, 429 + other non-2xx →
  `UploadError::Backend`, transport failure → `UploadError::Transport`.
  `resolve_backend("imgur")` returns an `Arc<dyn UploadBackend>` from
  the registry; unknown ids → `UploadError::UnknownBackend`. Response
  mapping tested via `mockito` (no real network). New workspace deps:
  `reqwest` (rustls-tls, multipart, json), `mockito` (dev-dep);
  `keyring` added to `flintshot-app`.
- `UploadOutcome::new(url, delete_handle)` constructor (Phase 4 Task
  004). Required because `UploadOutcome` is `#[non_exhaustive]`, so
  external crates cannot use struct-literal construction.
- `flintshot upload <path>` CLI body (Phase 4 Task 005) — replaces the
  Phase 0 stub. Loads the file (clear `no such file` error), sniffs
  the format via `image::guess_format` (PNG / JPEG / WebP sent as-is;
  no re-encode, no `--mime` flag), resolves the backend from
  `--backend` over `[upload] default-backend` over the built-in
  `"imgur"`, reads the Client-ID from the keyring entry `flintshot` /
  `imgur:client-id` when present (a failed keychain read falls back to
  anonymous with a log line), runs the backend on a fresh tokio
  runtime, prints the URL to stdout as the last line
  (machine-parseable; diagnostics to stderr), copies the URL to the
  clipboard (`arboard`), fires a desktop notification (`notify-rust`)
  on success and on failure, reports the result to the daemon
  best-effort, and exits non-zero on any failure.
- `flintshot auth imgur [--clear]` subcommand (Phase 4 Task 006) —
  the only credential-entry path. Prompts for the Imgur Client-ID with
  hidden input (`rpassword`) when stdin is a TTY, or reads
  `IMGUR_CLIENT_ID` otherwise; stores it in the OS keychain
  (`flintshot` service, `imgur:client-id` entry) — never in
  `config.toml`, logs, or stdout. `--clear` removes the entry;
  clearing an absent entry is a no-op with exit 0.
- Daemon `Request::UploadLast` dispatch (Phase 4 Task 007) — replaces
  the `unsupported` stub. Resolves the `ReportSaved`-fed last-saved
  path, spawns `flintshot upload <path> [--backend <id>]` as a worker
  process (`--backend` omitted when it equals the built-in default),
  and emits `Event::UploadStarted { backend }` optimistically before
  the spawn (mirrors `CaptureStarted`). With no prior save it emits
  `Event::Error { code: "no-last-capture" }` and does not spawn
  (mirrors `PinLast` / `OpenLast`).
- Worker → daemon upload-result reporting (Phase 4 Task 008). On
  success `run_upload` sends `Request::ReportUploaded { url,
  delete_handle }` through the same `delegate_to` path as
  `ReportSaved` — best-effort and silent when no daemon answers. The
  IPC server intercepts it in `handle_connection` and broadcasts
  `Event::Uploaded` to every subscriber. See ADR-0025.
- Editor *Upload* button (Phase 4 Task 009). A "☁ Upload" button in
  the egui toolbar composes the final image (selection-aware, like
  Pin), writes it to a temp PNG (`flintshot-upload-<pid>-<n>.png`,
  left on disk for the OS temp reaper; a reaper thread waits on the
  child so it doesn't zombie), spawns `flintshot upload <tmpfile>` as
  a detached worker, and closes the editor with
  `EditorOutcome::Uploaded(String::new())` — the outcome signals
  initiation; the real URL reaches subscribers via the worker's
  `ReportUploaded`, not the editor. Compose / write failures close the
  editor with the error; a spawn failure keeps the editor open so
  Save / Copy still work.

### Fixed

- The tray icon now registers with StatusNotifierItem hosts
  (COSMIC's status area included) and tray menu clicks dispatch.
  Since Phase 2, a tray-build daemon exported its
  `/StatusNotifierItem` + `/org/ayatana/NotificationItem` objects
  on the bus but never completed the `RegisterStatusNotifierItem`
  handshake with `org.kde.StatusNotifierWatcher` — no icon
  appeared on any SNI host — and menu clicks never arrived. The
  daemon's platform loop never iterated the GLib main context that
  libappindicator (behind `tray-icon` on Linux) and muda drive
  from: it built the tray once, then blocked on
  `recv_timeout(100 ms)` forever, so registration and menu events
  queued on GLib sources that nobody dispatched. The module doc's
  "raw polling suffices on Linux" claim was true for
  `global-hotkey`'s crossbeam receiver and false for the GTK-backed
  tray half — both code and claim are now fixed: the loop pumps the
  default GLib context every iteration before draining tray events
  (bounded — ≤64 non-blocking `iteration(false)` calls with
  early-out when idle, so a busy source can't wedge the daemon),
  and the doc states the per-source truth. Headless unit tests pin
  the seam (`crates/flintshot-app/src/daemon.rs`): an idle source
  never fires unpumped, fires after the pump, a self-re-arming
  source is bounded by the cap, and the real `run_event_loop`
  dispatches only via its own pumping. This was the last of the
  three pre-existing bugs the launcher effort's L5 pass found
  (2026-08-28, TEST_MATRIX finding 3); the other two shipped in
  `fix-delegated-capture`. Manual L5 verification pending — the
  dev container has no desktop session; the maintainer runs the
  spec's exit criteria before merge.
- Daemon-mediated captures work again: daemon-spawned
  region/fullscreen workers now carry `--standalone`, so each runs
  the capture in its own process instead of delegating straight
  back to the daemon. Since Phase 2's detect-and-delegate, every
  capture path through the daemon (tray menu, desktop actions,
  global hotkeys, CLI-with-daemon) fork-looped — each spawned
  worker re-delegated and the daemon spawned the next (~470
  processes in 8 s observed on COSMIC) with no editor ever
  opening. The dispatch argv tests pin the guard, including the
  intent-named
  `capture_workers_spawn_standalone_to_break_delegation_loop`
  (`crates/flintshot-app/src/worker.rs`).
- Window-opening workers no longer crash on at-spi desktops
  (GNOME/KDE/COSMIC with `org.a11y.Bus` running): `eframe` →
  accesskit → zbus 5 needs an ambient tokio reactor on the
  window-creating thread, and `region`/`fullscreen`/`open`/`pin`
  workers owned none — they panicked ("there is no reactor
  running, must be called from the context of a Tokio 1.x
  runtime") before any window appeared. The new
  `flintshot-app::ambient` module enters a multi-thread runtime
  around every window path (`run_editor_with_config`,
  `run_overlay_with_config`, `run_pin_window`) for exactly the
  duration of the window loop; regression tests in
  `crates/flintshot-app/src/ambient.rs` proxy the exact zbus-5
  condition via `Handle::try_current()`. Standalone captures on
  at-spi sessions open the editor instead of panicking.
- Tray *Bind shortcuts…* clicks are no longer silently dropped when
  the portal session task is outside its select loop (during
  startup awaits or while a bind dialog is open): the trigger now
  uses `Notify::notify_one`, whose stored permit survives the busy
  window and fires when the loop resumes; repeat clicks collapse
  into one queued bind (found in the Phase 6 ship review).
- A mid-session portal crash or restart no longer stops global
  hotkeys *silently*: every stream-end path of the portal session
  now logs a `warn` naming the ended signal source and the re-attach
  fix, and records an actionable failure on the handle so a later
  `hotkeys bind` reports "restart the daemon to re-attach" instead
  of a vague session-gone error.
- The daemon no longer waits passively when the session's tray host
  (StatusNotifierWatcher) is dead or late: an SNI watchdog (Linux,
  `--features tray`) probes `NameHasOwner` at startup and every
  30 s, and when the watcher is unowned pokes the standard then the
  COSMIC well-known name (`com.system76.CosmicStatusNotifierWatcher`)
  — a method call against an activatable name is the only
  client-side D-Bus activation trigger, so on COSMIC the watcher is
  brought back and the tray icon (re)appears within one interval
  with no user action beyond having the daemon running (root cause
  observed live on L5 2026-08-30: the watcher had exited and even a
  daemon restart didn't help, because nothing ever activated it).
  Pokes are best-effort and bounded (2 s), failures log at `debug`
  (a tray-less desktop is a normal steady state), and a watchdog
  failure never affects daemon startup or shutdown beyond one
  bounded join. At startup the barrier is synchronous: the daemon
  probes, pokes if the watcher is unowned, and waits (≤ 2 s) for
  the activated watcher to take the name **before** building the
  tray — a live race showed the fire-and-forget variant could
  activate the watcher mid-libappindicator-init and miss the
  registration event, leaving the icon absent until restart. New
  `zbus` 5 direct dep resolves onto the copy ashpd already builds —
  a dep edge, not a new crate build (ADR-0032).

### Changed

- Editor toolbar reorganized from a single overflowing row into the
  two-row rail + bay layout (see Added above). The always-visible
  color/stroke pickers now appear only for tools that use them; the
  annotation counter and the static "Drag to draw · Ctrl+Z undo ·
  Esc cancel" hint row are removed (replaced by per-tool hints); the
  emoji-labeled buttons are replaced by painter-drawn glyph chips.
- `UploadBackend` trait methods now return `Pin<Box<dyn Future + Send>>`
  instead of `impl Future` (Phase 4 Task 004). The `-> impl Future`
  return type made the trait not dyn-compatible, but the registry and
  daemon need `Arc<dyn UploadBackend>`. This is the standard boxed-
  async pattern; `Send + Sync + 'static` shape preserved.

- Planning docs reorganized to the canonical OpenCode harness v0.5.0+
  layout. Root `SPEC.md`, `tasks/plan.md`, `tasks/todo.md`, and
  `tasks/archive/` are eliminated; in their place, each effort lives at
  `docs/specs/<effort-slug>/` with `spec.md` (frontmatter
  `status: draft|active|shipped`), thin `plan.md`, flat `todo.md`, and
  per-task `tasks/00N-*.md` files. Shipped Phase 0 →
  `docs/specs/phase-0-scaffolding/` (status: shipped); in-flight Phase 1
  → `docs/specs/phase-1-capture-editor/` (status: active). `docs/adr/`
  renamed to `docs/adrs/`. `docs/ROADMAP.md` "How we plan" now describes
  the status-flip lifecycle (no archive step). Links in `README.md`,
  `CONTRIBUTING.md`, and `AGENTS.md` updated to the new paths.
- `EditorState::undo` / `redo` fields changed from
  `Vec<Vec<Annotation>>` to `Vec<Snapshot>` (Phase 1 Task 14). The new
  `Snapshot` type captures both `anns` and an optional `image`, so
  crop (which mutates the source image) is undoable. Annotation-only
  commits carry `image: None` (no extra clone); crop carries
  `image: Some(pre_crop)`. Callers that pushed raw `Vec<Annotation>`
  onto `undo` / `redo` must switch to `Snapshot::anns_only(vec)` or
  `Snapshot::full(vec, img)`.
- `Snapshot` extended with `next_number: u32` (Phase 2b Task 004) so
  undoing a Number-tool placement decrements the auto-incrementing
  counter (otherwise it would ratchet upward across undos and the
  next placement would skip a number). Constructor signatures widen:
  `Snapshot::anns_only(anns)` → `Snapshot::anns_only(anns,
  next_number)`, and `Snapshot::full(anns, image)` →
  `Snapshot::full(anns, image, next_number)`. The undo / redo paths
  always restore `next_number` from the snapshot.
- `EditorState::selection` field renamed to `selected_index` (Phase 1
  Task 14). The old `selection: Option<usize>` held the hit-tested
  annotation index; it collided with the new `selection: Option<Rect>`
  drag-rectangle field. `selected_index` is still `None` (not yet wired
  into any tool — reserved for Phase 2's select-and-move feature).
- `Color` (de)serialization switched from struct-of-fields (the default
  `serde::Serialize` derive) to hex string (`#rrggbb` / `#rrggbbaa`).
  Affects every type that embeds `Color` (currently `Annotation` and
  `Config::UiConfig`); downstream consumers of `serde_json` /
  `postcard` encoded `Color` values need to update.
- `flintshot-core::render::render()` signature gained a third
  parameter: `bg: &mut Option<C::Image>` for the toolkit-specific
  background texture handle cache (Phase 1 Task 4). The shell owns
  the cache field; `render()` borrows it for the duration of the
  frame. Previous signature was `render(state, canvas)`; existing
  code must thread the cache through. No external callers existed
  at the time of the change.
- `flintshot-editor-egui` now uses a 3-module layout
  (`canvas.rs` / `app.rs` / `shell.rs`) rather than a single
  `lib.rs` (Phase 1 Task 5). Public API unchanged — `pub use`
  re-exports in `lib.rs` preserve the import paths. `canvas.rs`
  is the Task 4 content (unchanged); `app.rs` is the new private
  `EditorApp` `eframe::App` impl; `shell.rs` is the `EguiShell`
  `EditorShell` impl + `run_editor_blocking` helper.
- `justfile` gained `bootstrap`, `nextest`, and `watch` recipes, and
  `test` now prefers `cargo-nextest` (with fallback to `cargo test`)
  for unit + integration runs. Doc tests still use `cargo test`
  since nextest doesn't collect them. `just ci` is unchanged — it
  still mirrors the GitHub Actions `test` job byte-for-byte
  (`cargo test`, no nextest) so local pre-push results match CI.
  `just bootstrap` installs the optional `cargo-watch` and
  `cargo-nextest` subcommands idempotently. AGENTS.md and
  CONTRIBUTING.md document the new recipes.

### Removed

- Throwaway `flintshot --dump-config` debug flag (Phase 1 Task 3).
  Replaced by the real `flintshot config` subcommand.
- Default-timestamped `--out` path for `flintshot region` /
  `flintshot fullscreen` (Phase 1 Task 6). When `--out` is omitted
  the CLI now opens the editor instead of writing
  `/tmp/flintshot-<ISO-8601>.png`. Explicit `--out PATH` still writes
  a PNG (Task 3's path preserved as the headless / scripting escape
  hatch).
- `crates/flintshot-app/src/time_util.rs` (Phase 1 Task 6). Its only
  caller was `OutputArgs::resolve_path`, removed alongside the
  default-timestamp path above. The algorithm (Howard Hinnant's
  `civil_from_days`) is recoverable from git history; per the Task 3
  close-out note, Phase 4's upload-cache timestamps will promote it
  to `flintshot-core` rather than re-derive it.
- `crates/flintshot-editor-egui/examples/open_editor.rs` (Phase 1
  Task 6). The example was the Task 5 smoke probe for
  `EguiShell::run_editor`; the `flintshot region` CLI path now covers
  that role end-to-end.

### Fixed

- The editor's blur rendering re-uploaded a fresh GPU texture every
  frame per blur annotation (admitted Phase 1 debt the overlay's
  every-pointer-motion repaint cadence made expensive).
  `EguiCanvas::draw_image_region` now keeps a content-hash-keyed
  region-texture cache with per-frame liveness eviction at
  `bind_painter`: committed blurs re-blit an existing texture,
  changed content misses and uploads as before, and undrawn entries
  free their GPU memory — so long overlay sessions neither re-upload
  nor leak. The overlay's on-demand repaint contract is now pinned by
  tests: construction and idle frames (pointer parked, hover held)
  request no repaints, and the floating toolbar requests a single
  convergence repaint only when its measured size changes while
  visible.
- `ImageCanvas::draw_ellipse` was a no-op throughout Phase 1 — committed
  ellipses silently dropped from saved/copied PNGs even though they
  rendered correctly in the live editor (a WYSIWYG hole masked because
  no Phase 1 tool ever committed an `Annotation::Ellipse`). Phase 2b
  Task 001 filled in the real rasterizer (fill + outline into the
  `RgbaImage`), so ellipses now round-trip through Save / Copy
  identically to the live view.
- Crop no longer leaves the cropped image partly hidden behind the
  editor toolbar. The egui canvas now offsets every draw call by the
  canvas panel's top-left, so the image's (0, 0) lands at the panel
  origin instead of the window's — the toolbar used to cover the
  topmost rows of every screenshot, which became visible as soon as
  a small crop made the hidden strip a large fraction of the image.
  The shell also keeps requesting `ViewportCommand::InnerSize` until
  the panel reports the cropped dimensions, so the window resizes to
  fit on toolkits / platforms that honour the command.
- Text tool no longer traps the editor in modal mode. Clicking elsewhere
  while editing commits the current draft and places a new caret (or
  starts the newly-selected tool) — Flameshot-like behaviour. Switching
  tools via the toolbar while a draft is active now flushes the draft
  instead of blocking all subsequent input.
- Copy-to-clipboard no longer loses the image on X11. The
  `arboard::Clipboard` handle is kept alive in the `EditorApp` for the
  window's lifetime (X11's clipboard is owner-based: dropping the handle
  or exiting the process clears the selection). The editor window stays
  open after copy with "Copied!" feedback; the user pastes and then
  closes manually.
- Copy now respects the active `Tool::Select` selection. When a
  non-degenerate selection rect is present, Copy emits just the
  selected region (annotations baked in, clipped at the region
  boundary); without a selection it falls back to the full image.
  Editor state is left untouched — the displayed image and selection
  rect remain as-is, so the user can adjust and copy again. This
  decouples "copy the selection" (read-only output) from the Crop
  tool (which permanently narrows the editor). New
  `flintshot_core::compose_final_region(state, rect)` is the
  selection-aware source of truth alongside `compose_final`.
- Clicking any toolbar button (Copy / Crop / Save / tool selectors /
  pickers) with an active `Tool::Select` selection no longer corrupts
  the selection rect. The shell's pointer dispatch previously emitted
  `PointerDrag` for every frame the primary button was held,
  regardless of where the press began — so a press that began on the
  toolbar fed `update_selection` with the toolbar's (negative
  canvas-y) coordinates, extending the rect up past the image origin
  before Copy / Crop read it. The shell now tracks whether the
  current press began inside the canvas, and only emits drag/up
  events for canvas-originated presses.
- Text caret now follows the insertion point. Previously the block
  caret was pinned to the first line's end — after `Shift+Enter` it
  stayed on line 0 while typing continued on the new line below. A
  `text_cursor` (char index into the draft) now tracks the real
  insertion point; `←`/`→`/`↑`/`↓` move it (wrapping across line
  breaks, preserving the column), and `Home`/`End` jump to the line
  edges. Typing, backspace, and newline all operate at the cursor, so
  text can now be edited mid-string rather than only appended.

### Notes

- **Phase 5 (Live overlay) shipped 2026-08-17.** All nine tasks
  complete; exit criteria in
  `docs/specs/phase-5-live-overlay/spec.md` met (per-task entries
  above landed with their code). ADR-0028 records the
  capture-then-overlay decision and the Wayland editor-window
  fallback that follows from it; `docs/TEST_MATRIX.md` gains the
  Phase 5 manual section (per-OS overlay flows + Wayland fallback).
  Spec status flipped to `shipped`. No version tag cut — `0.x.y`
  remains in effect; the `1.0.0` dogfooding clock (earliest
  2026-09-14, see `docs/ROADMAP.md`) is unaffected.
- **Phase 4 deviations from the spec draft** (documented in
  ADR-0025 and `docs/TEST_MATRIX.md` Phase 4 caveats): upload
  *failures* do not reach daemon subscribers (the sketched
  `Event::Error { code: "upload-failed" }` broadcast was dropped; the
  worker owns failure UX — notification, stderr, non-zero exit), and
  no tray / COSMIC-applet *Upload last* menu item shipped —
  `Request::UploadLast` is reachable over IPC from scripts and custom
  clients.
- **Phase 1 (Capture + Editor Window) shipped 2026-07-28.** All
  checkpoints A–F complete; exit criteria in
  `docs/specs/phase-1-capture-editor/spec.md` met. Spec status flipped
  to `shipped`; `docs/ROADMAP.md` moves Phase 2 to **In flight**. No
  version tag cut — `0.x.y` remains in effect until Phase 4 (upload)
  per the versioning policy.

- Phase 0 closed against the local standing gate (`fmt`, `clippy`,
  `test`, MSRV build, cosmic build) — no git remote is configured yet,
  so the GitHub Actions pipeline has not executed against a push.
  First push will run the real CI; any failure becomes a session note.
- Standing gate corrected during Task 1 close-out: the gate commands
  in `AGENTS.md`, `docs/TODO.md`, `docs/PLAN.md`, `CONTRIBUTING.md`,
  `justfile`, and `.github/workflows/ci.yml` previously used
  `cargo test --all-targets` and `cargo test --doc` without a package
  selector — which silently ran only `flintshot-app`'s zero tests and
  errored on `flintshot-app`'s missing library target. Corrected to
  `cargo test --workspace --all-targets` and
  `cargo test --doc -p flintshot-core -p flintshot-editor-egui`.
  Same fix applied to the clippy command (`--workspace`).
- CI's `lint`, `test`, `msrv`, and `cosmic-build` jobs now install
  `xcap`'s Linux build deps (`libclang-dev`, `libpipewire-0.3-dev`,
  etc.) — needed because `flintshot-core` pulls in `xcap` for capture.
- No released versions yet. This unreleased section tracks Phase 0
  scaffolding plus Phase 1 Tasks 1 (`config` module), 2 (`capture`
  module), 3 (CLI wiring: `region`/`fullscreen` write PNGs,
  `config` subcommand, `--out` flag), 4 (`render()` background
  blit + `EguiCanvas::upload_image` / `draw_image`), 5
  (`EguiShell::run_editor` opens an eframe window, displays the
  screenshot, returns `EditorOutcome::Cancelled` on `Esc` / close),
  and 6 (`region` / `fullscreen` routed through `EguiShell` when
  `--out` is omitted; `--out PATH` preserved as the headless path;
  `cfg(not(feature = "egui"))` fallback errors clearly), and 7
  (editor state machine: `pending` field + `begin/update/commit/
  cancel_annotation`, snapshot `undo`/`redo`, `hit_test`; public
  `MockCanvas` + `DrawCall`; `render()` dispatches all 9 variants +
  draws `pending` on top), and 8 (`Tool::Rect` interactive
  end-to-end: egui pointer events → state machine → `render()`;
  toolbar with Rect selector; `Ctrl+Z` undo; zero-area rect filter),
  and 9 (Save / Copy buttons in the editor toolbar; PNG write via
  `rfd`; clipboard via `arboard`; new `compose_final` in core for
  headless render), and 10 (keyboard shortcuts via a pure
  `EditorState::dispatch_input`; new `flintshot-core::input` module
  with `Modifiers` / `InputEvent` / `DispatchOutcome`; cross-
  platform modifier mapping; pointer flow also routes through the
  dispatcher; `EditorApp::logic` reduced to a thin translator), and
  14 (`Tool::Select` drag-select + crop: new `EditorState::crop` /
  `confirm_selection` / `selection` field; `Annotation::
  crop_and_translate` per-variant; `Snapshot` type for image-aware
  undo/redo; `Rect::intersect` / `Rect::contains`; shell toolbar
  gains Select + Crop buttons; bg cache invalidated on image-dims
  change), and 15 (color / stroke-width / font-size pickers in the
  toolbar; config persistence on editor close via `Config::load` +
  `Config::save`; CLI seeds `EditorState` from `Config::load()`).

### Removed

- **Pin-to-desktop, removed entirely** (ADR-0049, spec
  `docs/specs/remove-pin-to-desktop/`): the `flintshot pin`
  subcommand, the editor/overlay Pin button, the tray/applet *Pin
  last* action, the pin window machinery
  (`flintshot-editor-egui`'s `pin` module, `EditorShell::run_pin`,
  `EditorOutcome::Pinned`), daemon pin sessions and the pre-grab
  pin kill/respawn path, and the `PinConfig` core type are gone —
  deleted, not stubbed. The feature never worked as intended on
  Wayland (no always-on-top protocol for ordinary top-levels) and a
  layer-shell-based pin is not worth the platform-specific
  investment. **Wire-format break:** `Request::PinLast` and
  `Event::Pinned` are deleted from the postcard-encoded IPC enums,
  which re-indexes every variant after them — old and new binaries
  cannot interoperate (accepted: every client except the COSMIC
  applet is daemon-spawned and version-locked; rebuild the applet
  with the daemon). A `[pin]` section in an existing `config.toml`
  is now silently ignored (config parsing has no
  `deny_unknown_fields`); no migration is provided. Save / Copy /
  Upload / OpenLast flows are unchanged.

## Release cheat sheet (for maintainers)

When cutting the first release:

1. Confirm Phase exit criteria in `docs/ROADMAP.md` are met.
2. Move the `## [Unreleased]` contents to a new `## [0.1.0] - YYYY-MM-DD`
   section.
3. Bump `version = "..."` in every `Cargo.toml` (workspace +
   per-crate).
4. Tag with `git tag -s v0.1.0`; tags are signed.
5. Publish crates in dependency order: `flintshot-core` first, then
   dependents.
6. Add a fresh `## [Unreleased]` heading above the new release.
