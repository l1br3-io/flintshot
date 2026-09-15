# ADR-0054: In-window routing — routes, not epochs; destruction reserved for captures

**Context.** ADR-0053's single window still recreated itself on
every page transition: one `eframe::run_native` lifetime per *page*,
because the editor page's viewport was image-sized (a runtime resize
on Wayland is compositor-negotiated and fractional-scale-drifting —
per-image geometry forced per-epoch windows). The cost was real WM
churn: taskbar entries refired, pinned windows lost their anchor,
open/close animations replayed, and the portal's post-selection
clipboard delay (>1 s observed) left a silent dead zone with no
window on screen at all. Three gaps shared one root: the window's
geometry and lifecycle were coupled to the page.

**Decision.**

1. **One eframe lifetime per *window generation*; routes, not
   epochs.** `EguiShell::run_window` opens a generation and a
   `WindowApp` router hosts the pages — Home / Editor / Loading —
   delegating each frame to the live route. A transition swaps the
   route in place and issues a one-time viewport command batch
   (`InnerSize` + `Resizable` + `Title`) from a pure per-route
   geometry table; same-kind swaps (an image replacing the editor's)
   issue nothing. `run_home` / `run_editor_page` and the
   `EditorPageExit` home/open arms are gone; the app crate's outer
   loop classifies only capture-intent / hidden-by-event / user-quit
   and reads the route-at-close for the same-page restore.
2. **Destruction stays mandatory-only.** Every capture still closes
   the window before its grab (ADR-0047: surface destruction is the
   only portable hide on Wayland) and user quit still exits; nothing
   else closes anything. Open/Paste arm a routed editor the router
   consumes (a new `RouteHost` seam), `Esc` publishes `Cancelled`
   which the router swaps into Home, and render errors report
   through the status line and route Home — the window survives all
   three.
3. **Stable editor geometry is the enabler.** The editor route opens
   at a fixed 1024×640 logical default (open-time clamped to small
   monitors) and the image **fits** inside it: the fit scale lives
   in a single shared view transform (`CanvasView`) threaded through
   every image-px ↔ screen-logical mapping — canvas draws, pointer
   hit-testing, chrome placement, text metrics. Annotations stay in
   image-pixel space; nothing in the data model moved. Loading
   deliberately shares the editor geometry byte-for-byte, so the
   progress → image hand-off issues zero viewport churn.
4. **Two-phase grab delivery bridges the portal's write gap.** The
   grab splits at the `capture_selection` return — `Ready` (image in
   hand) vs `AwaitClipboard` — and the grab thread emits
   `Confirmed` (pixels frozen) *then* `Finished` through the pump's
   channel. `Confirmed` reopens the generation on the Loading route
   (spinner, "retrieving capture…", 50 ms heartbeat); `Finished`
   delivers into the open window through the shared pending-editor
   slot + the router's host poll — Loading swaps to Editor in place.
   A fast read-back racing the open lands the image first (no
   spinner flash); failures and quiet exits route the open page home
   (never a stranded spinner); stale generations fold silently at
   either phase. No cross-thread context wake is needed: the
   heartbeat polls the flags (the `CloseHandle` precedent documents
   the alternative).

**Consequences.**

- The window's surface identity is stable across every non-capture
  transition; taskbar/pinning/compositor state survives editing
  sessions. The one accepted cost is a single resize pop per
  home ↔ editor hop (home stays deliberately compact, OQ-4).
- Wayland's async resize never fights the app: only two fixed
  geometries exist, plus the user's own resizes of the (resizable)
  editor route, with the fit recomputed per frame from the live
  canvas rect.
- The worker path (`run_editor_blocking`, no-app-window fallback) is
  untouched: image-sized windows, monitor-grab recapture, the
  legacy lifecycle — out of scope by spec.
- Testing posture: the route state machine, geometry batches,
  delivery ordering, and the fit transform are all headless-unit
  -tested (egui synthetic contexts, `FakeCapturer` doubles, an
  injectable clipboard wait); the ≤200 ms confirm-to-spinner
  perception and the compositor-selection UX remain manual checks.
