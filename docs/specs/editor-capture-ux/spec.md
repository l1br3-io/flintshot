# Spec: Editor capture UX — home-parity `+`, stable window, routing, capture progress

---
status: active
started: 2026-09-16
---

## Objective

Four capture-UX gaps in the single app window, reported together
because they share one surface (the editor page) and one flow (the
pump's in-process grab):

1. **The editor's `+` menu must mirror the home window** — Capture /
   Open… / Paste — and its Capture action must use the **native capture
   backend** (the portal's OS selection UI on Wayland). `xcap` is a
   fallback only. Today the popup offers Region/Fullscreen (whose mode
   label is dropped by the router anyway), and the user observed the
   xcap fallback flow (monitor grab → editor with Select pre-armed)
   where the OS selection UI was expected.

2. **The editor page opens at a stable default size** with the image
   fitted inside, instead of resizing the window to every image's
   pixel dimensions. This also answers the standing question: yes, the
   app is a single window — the differing sizes are the per-page
   window geometry (home fixed 360×240, editor image-sized), by
   design. This effort removes the image-sized churn.

3. **In-window page routing.** Page transitions stop recreating the
   window: one eframe lifetime per *window generation*, with Home ↔
   Editor ↔ Loading as internal routes and a one-time
   `ViewportCommand::InnerSize/Resizable/Title` swap on transition.
   Window **destruction** remains only where it is mandatory — every
   capture (Wayland `set_visible` is a no-op; surface destruction is
   the only not-in-the-shot guarantee, ADR-0047) — plus user quit.
   Wins: no WM window churn (taskbar / pinning / open/close
   animations refire on every recreate today), snappier transitions,
   and the progress→image swap becomes a natural in-place state
   change. Viable *now* because item 2 removes the per-image geometry
   that forced per-epoch windows.

4. **A progress indicator bridges the post-selection gap.** After the
   user confirms a selection in the OS UI, the portal's clipboard
   write lands >1 s later (observed in the wild; the read-back waits
   up to ~5 s), then decode + window spawn add more. The editor page
   now opens **immediately** after the OS UI confirms — stable size,
   spinner, "retrieving capture…" — and the image swaps in when the
   grab delivers, in-window (item 3). Applies to every trigger that
   rides the pump's grab (home Capture, editor `+`, tray-forwarded,
   hotkey).

**User:** the flintshot user on Linux/Wayland (COSMIC today) — the
same person the single-window effort serves.

**Success looks like:** clicking Capture from home, the editor `+`,
or the tray feels like one continuous gesture: OS overlay → editor
window already on screen with progress → image ready. Page
transitions keep the same window (surface identity) and a stable
editor size. No silent dead zone after confirming a selection.

## Background (verified in code)

- The `+` chip popup (`toolbar.rs:486`) offers Region/Fullscreen;
  both exit as `EditorPageExit::Capture(mode)` and
  `arm_editor_exit` (`app_window.rs:836`) drops the mode — every
  capture runs the same `grab_for_app_window` →
  `resolve_capture_backend` → auto (portal-first on Wayland,
  `capture/mod.rs:230`). The xcap fallback the user saw is the
  `!supports_selection()` arm of `grab_with_backend`
  (`capture_flow.rs:319`) — either a portal probe failure at grab
  time or a pre-in-process-grab build. Diagnosis is in scope.
- The editor page viewport is image-sized (`shell.rs:317`); home is
  360×240 (`shell.rs:277`). A page transition is window close +
  recreate (ADR-0047/0053) — per-epoch geometry is why sizes differ.
  The recreation-for-captures stance stays (destruction is the only
  portable hide on Wayland); recreation-for-transitions does not
  survive this effort.
- The post-confirm delay chain: `capture_selection` answers
  `clipboard:///` → `read_portal_clipboard_grab` polls up to
  50×100 ms (`capture_flow.rs:104`) → decode → `set_pending_editor`
  + `Show` → new window epoch. The whole chain runs before any
  window exists.

## Scope

### In scope

- `+` popup: replace Region/Fullscreen with Capture / Open… / Paste
  (home parity: labels, ordering, Replace-semantics hint on
  Open/Paste). Capture routes the existing `NewCapture` exit; Open
  and Paste route the page bridge's existing `open_image` /
  `request_paste` (same contract as the chrome strip).
- Portal-first determinism: reproduce the observed xcap-fallback on
  the dev session, fix the root cause (probe robustness or stale
  build), and log the resolved backend on every grab (already does —
  keep). Acceptance is behavioral on a Wayland+portal session.
- Stable editor window: fixed default logical size (resizable stays
  `true`), clamped to the monitor's work area. Image renders
  **fit-to-view** (scale = min(fit, 1.0) — never upscaled),
  letterboxed and centered. Annotations stay in image-pixel space;
  the canvas transform gains a scale factor (physical → logical
  mapping, input hit-testing, selection-first landing, crop refit).
- In-window routing: a router app (editor-egui) holds Home / Editor /
  Loading as internal routes over **one eframe lifetime per window
  generation**; `run_home` / `run_editor_page` merge into a single
  window entry. Transitions (home→editor via Open/Paste, editor→home
  via Esc, image swaps, loading→image) swap routes in-place with a
  one-time viewport command batch (inner size, resizability, title).
  Destruction stays exactly where it already is mandatory: capture
  hides (self- and externally-triggered) and user quit — the app
  crate's outer loop keeps classifying closes, but only those two
  kinds remain. Grab delivery to a *closed* window (a capture just
  finished) still reopens a generation starting on the right page.
- Progress page: a loading route — spinner + status — that appears
  the moment `capture_selection` returns when the image is not yet
  deliverable (the portal clipboard path), and swaps to the fitted
  image in-place (same route lifecycle, no window recreation) when
  the grab delivers. Direct grabs (image in hand) open the editor
  route with the image, no spinner.
- Failure during the wait surfaces in the open page (status line +
  Esc-to-home), never a silent hang or a stranded spinner.

### Out of scope

- The pre-overlay gap (tray click → OS overlay appears) — cannot be
  covered by a window without polluting the grab.
- The worker-path editor window (`run_editor_blocking`, legacy
  no-app-window flow) — unchanged this effort.
- Zoom / pan controls beyond fit-to-view; the overlay; the COSMIC
  applet; home window size.

## Technologies

Existing stack only: Rust workspace, eframe/egui (editor-egui),
flintshot-core capture registry (portal + xcap). No new dependencies.

## Commands

```sh
cargo build                                  # default members
cargo run -- app                             # the single window
cargo run -- backends                        # backend probes + auto pick
cargo fmt --all --check                      # format gate
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
```

## Project Structure

| Change                                     | Where                                              |
| ------------------------------------------ | -------------------------------------------------- |
| `+` popup actions                          | `crates/flintshot-editor-egui/src/toolbar.rs`      |
| Router app: one lifetime, Home/Editor/Loading routes, viewport swaps | `crates/flintshot-editor-egui/src/{shell,app}.rs` |
| Fit-to-view canvas transform               | `crates/flintshot-editor-egui/src/{canvas,app}.rs` |
| Grab split: confirmed vs pending-delivery  | `crates/flintshot-app/src/capture_flow.rs`         |
| Outer loop: close classification shrinks to capture-hide / quit; generation spawn | `crates/flintshot-app/src/app_window.rs` |
| Portal probe robustness (if buggy)         | `crates/flintshot-core/src/capture/portal.rs`      |
| Spec / plan / tasks                        | `docs/specs/editor-capture-ux/`                    |

## Code Style

Per `.opencode/tech/rust/coding-style.md` and repo conventions:
`#![forbid(unsafe_code)]`, pedantic clippy, `thiserror` in core /
`color_eyre` in bins, no `unwrap()` outside tests, doc comments on
public items, `#[non_exhaustive]` on plausibly-extended public types.
Pure seams extracted for headless tests, the established pattern:

```rust
// Pure, headless-testable: the fit scale for an image in a viewport.
// Never upscales (cap 1.0); degenerate inputs yield no scale.
#[must_use]
pub(crate) fn fit_scale(img: (f32, f32), viewport: (f32, f32)) -> Option<f32> {
    let (w, h) = img;
    let (vw, vh) = viewport;
    if w <= 0.0 || h <= 0.0 || vw <= 0.0 || vh <= 0.0 {
        return None;
    }
    Some((vw / w).min(vh / h).min(1.0))
}
```

## Testing Strategy

- **Unit (headless, preferred):** popup action mapping (each `+`
  entry fires the right action); fit-scale math (large, tiny,
  degenerate, aspect-extreme); canvas round-trip (logical → image px
  → logical under scale); grab-outcome classification
  (deliver-now vs pending-clipboard vs quiet vs failed) with the
  existing `FakeCapturer` doubles; route state machine (transitions,
  viewport command batches, loading → image delivery, failure
  surfaces) with the existing `Shared`/`HomeCore`/recorder-bridge
  test seams; the app crate's outer-loop classification (capture
  hides and quits still close; nothing else does).
- **Doc tests:** `cargo test --doc` for the two lib crates.
- **Manual (Wayland session, recorded in tasks):** tray capture →
  spinner page → image; `flintshot backends` shows portal available;
  `+` Capture opens the OS overlay, not the fallback editor.

## Boundaries

- **Always:** run the full lint/test gate before declaring done;
  keep `flintshot-core` free of GUI types (the
  `no_gui_types_in_core` test must stay green); update
  `CHANGELOG.md` under `## [Unreleased]`; extract pure seams with
  tests.
- **Ask first:** changing public types in `flintshot-core`
  (`EditorOutcome::NewCapture`, `HomeDirective`); altering the
  prepare-gate / hide-ack handshake timings; removing the editor
  chrome strip (see Open Questions); new dependencies.
- **Never:** open a flintshot window between trigger and grab
  (not-in-the-shot, ADR-0047); `unsafe`; `#[allow]` without a
  justifying comment; silence a failing test.

## Success Criteria

1. `+` popup shows exactly Capture / Open… / Paste; each routes the
   same action its home twin does (verified by unit test + manual).
2. On a Wayland session with the portal up, `+` → Capture opens the
   compositor's selection UI; the xcap monitor-grab fallback runs
   only when the portal is unavailable (manual + log line
   `backend=portal`).
3. Editor page opens at the same default size for every capture,
   home → editor included; the image is fitted (never upscaled) and
   centered; annotations draw and hit-test correctly under scale
   (unit tests on the transform, manual smoke).
4. Page transitions keep the same window: home → editor (Open/Paste
   capture-routing excluded), editor → home (Esc), image swaps, and
   loading → image all happen without surface destruction — same
   surface identity to the WM, no taskbar/animation churn (manual;
   unit tests on the route state machine). Window destruction occurs
   only for captures and user quit.
5. After confirming an OS-selection capture from tray, the editor
   window is on screen with a progress indicator within ~200 ms of
   the confirm; the image swaps in without the window recreating;
   a wedged read-back surfaces an error, not a hang (manual, plus
   unit tests on the delivery state machine).
6. Full gate green: `fmt --check`, `clippy -D warnings`,
   `test --workspace --all-targets`, doc tests.

## Open Questions

1. **Chrome strip fate:** the strip under the toolbar
   (`show_page_actions`, shipped in cfaef50) duplicates the `+`
   popup once it mirrors home. Remove it, or keep both surfaces?
   (Lean: remove — one surface, less rail clutter; maintainer call.)
2. **Default editor size:** propose ~1024×640 logical clamped to the
   work area; open to preference.
3. **Ctrl+N binding:** stays as the `+` popup's Capture accelerator
   (C/O remain the page's Capture/Open accelerators, Paste stays
   button-only — P is Pencil's tool key, documented collision).
4. **Home geometry under routing:** home stays a fixed 360×240
   non-resizable route (one resize pop per home↔editor transition),
   or both routes share the editor's stable size? (Lean: keep home
   compact — its three buttons want a small window; the resize pop
   is one compositor-negotiated frame.)
