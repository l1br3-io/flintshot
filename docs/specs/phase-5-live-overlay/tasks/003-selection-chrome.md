# Task 003: Selection chrome — dim mask, 8 resize handles, move-drag, W×H badge, magnifier on hover

**Description:** Flameshot selection semantics layered on top of the
existing dispatcher selection (plan decision 2): the dispatcher keeps
owning selection *geometry* for fresh drags (`PointerDown` →
`begin_selection`, drag → `update_selection`, release →
`finish_selection`); the overlay owns the *chrome* — the dim mask
outside the active region, 8 grab handles (4 corners + 4 edge
midpoints), move-by-drag inside the region, the live `W × H` badge,
and the magnifier on hover. Handle hit-testing, resize/move math, and
badge placement are pure functions in `overlay.rs` (unit-tested),
candidate for `core::geom` only if the COSMIC crate ever needs them.
Pointer presses with `Tool::Select` active are intercepted by the
shell *before* dispatcher dispatch: press on a handle → resize
gesture, press inside the region → move gesture, press outside →
normal dispatch (fresh selection). Gesture drags mutate
`EditorState::selection` (a `pub` field) directly — a documented,
tested exception to "everything goes through `dispatch_input`": the
dispatcher's selection transitions model fresh anchor-to-corner drags
only and have no concept of grab points. Gestures clamp to the image
bounds and normalize on release, mirroring `finish_selection`.

**Acceptance criteria:**
- [x] Dim mask: translucent black over everything outside the active
      region; the full panel when no (or degenerate) selection
      exists. Painted after the shared `render()` call so
      annotations outside the region dim too.
- [x] 8 handles at the corners + edge midpoints, drawn while the
      Select tool is active and no fresh selection drag is in
      progress; edge in logical px (DPI-independent), white fill +
      dark border.
- [x] `handle_at` hit-test: nearest handle center within a
      logical-px-scaled radius; returns `None` outside; tolerates
      un-normalized rects.
- [x] `resize_from`: the grabbed corner/edge follows the cursor, the
      opposite corner(s) stay anchored; rect stays un-normalized
      during the drag (mirror of `update_selection`) and normalizes
      on release.
- [x] `moved_rect`: translate preserving size, clamped to the image
      bounds on both axes.
- [x] Press interception (`gesture_for_press`, pure): handle press →
      `Resize`, interior press → `Move { offset }`, exterior press /
      non-Select tool / no selection → fall through to the
      dispatcher.
- [x] Cursor shapes: per-handle resize cursors (Nw/Se → NW-SE,
      Ne/Sw → NE-SW, N/S → vertical, E/W → horizontal), `Move`
      inside the region, `Crosshair` elsewhere with Select active;
      driven by hover *and* the active gesture.
- [x] Live `W × H` badge (physical px, rounded) near the selection's
      top-left, flipping below the region when the space above is
      off-panel, clamped into the panel; visible whenever a
      non-degenerate selection exists (including during fresh drags).
- [x] Magnifier on hover: no modifier hold (overlay UX per spec —
      "magnifier works on hover"); shows whenever the pointer is
      inside the overlay panel, suppressed during text editing.
      Reuses `crate::magnifier::Magnifier` verbatim.
- [x] Overlay repaint behaviour / idle CPU untouched (Task 008).

**Verification:**
- [x] `cargo fmt --all --check`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo test --workspace --all-targets` — incl. new pure-fn
      unit tests (hit-testing, resize/move math, gesture decision,
      badge text/placement, cursor mapping, usable-selection guard).
- [x] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual (X11, real desktop session — the dev shell's Xwayland
      hangs in wgpu device creation, see Task 002): `cargo run --
      region --overlay` — mask dims the screen, drag draws an
      undimmed region with live badge, release shows handles, handle
      drags resize with matching cursors, interior drag moves the
      region clamped to screen, magnifier follows the cursor, Esc
      exits clean.

**Dependencies:** Task 002 (`OverlayApp` shell).

**Files likely touched:**
- `crates/flintshot-editor-egui/src/overlay.rs` — chrome pure fns +
  tests, gesture state, `logic()` interception, `ui()` painting.
- `CHANGELOG.md` — Unreleased entry.

**Estimated scope:** Medium (~350 lines added to one module, no core
changes).
