# Task 008: Idle-repaint discipline + performance check + polish

**Description:** Make the overlay's on-demand repaint contract explicit,
fix the one per-frame cost the discipline exposes, and verify the idle
CPU budget (spec exit criterion 6).

**Idle-repaint audit (the discipline).** eframe 0.35's native loop is
reactive by construction: `ControlFlow::Wait` at rest, repaints only on
input events, explicit `request_repaint` calls, or egui's own
machinery (verified in eframe/egui 0.35 source:
`run.rs` sets `ControlFlow::Wait` / `WaitUntil`;
`ContextImpl::begin_pass_repaint_logic`). The overlay itself must
therefore hold to two rules:

1. **No unconditional repaint requests.** Every state change the
   overlay renders originates from an input event that already
   triggered the frame (pointer/key → dispatcher or chrome gesture).
   Construction requests nothing (pinned by test). The floating
   toolbar's appearance transitions are covered by egui's `Area`
   machinery itself: a new/hidden-again area runs an invisible sizing
   pass and egui requests the follow-up repaint
   (`containers/area.rs` `visible_last_frame` → `request_repaint`).
2. **A repaint is requested exactly when shell-side ui state needs one
   more frame to settle** — the toolbar anchor runs one frame behind
   the measured size (`toolbar_anchor`). The sizing pass covers first
   appearance; a *size change while visible* (DPI move, future layout
   change) would otherwise leave the toolbar misplaced at idle until
   the next input event. `show_toolbar` now requests the single
   convergence repaint when measured ≠ placed (fires on transitions,
   never per frame).

**Perf fix (the check's code change).** `EguiCanvas::draw_image_region`
— the blur render path — re-uploaded a fresh GPU texture every frame
per blur annotation (admitted debt in two code comments: `canvas.rs`
"Phase 1 doesn't cache across frames" and `magnifier.rs`'s GOTCHA
note). The editor window tolerated it; the overlay multiplies frame
frequency (fullscreen hover magnifier repaints on every pointer
motion), so the debt comes due. `EguiCanvas` now caches region
textures keyed by (content hash, buffer length, dest rect) with
per-frame liveness eviction at `bind_painter`:

- **Key by content hash, not dest/pointer:** a dest-only key shows a
  stale blur after undo-and-redraw at the same rect; a buffer-pointer
  key breaks when the core `BlurCache` re-allocates (crop clears it;
  allocator address reuse would alias different pixels at the same
  address). `DefaultHasher` over the buffer is a read-only pass and
  still beats what it replaces (`ColorImage::from_rgba_unmultiplied`
  premultiplies + copies, then `load_texture` allocates and uploads).
- **Liveness eviction at frame start:** entries not drawn in the
  previous frame are dropped, so their `TextureHandle`s free the GPU
  textures (undo, crop, session end). A long overlay session cannot
  leak texture memory.
- Pending (in-flight) blur drags produce fresh content each frame →
  cache miss → upload, exactly as before; the previous frame's entry
  evicts a frame later. Committed blurs (the static common case) hit.

**Polish:** the overlay module docs gain a §Repaint discipline section
recording the two rules and the egui-internals evidence; a duplicated
comment block in `overlay.rs` (leftover from a Task 003 edit) is
removed.

**Acceptance criteria:**
- [x] Drawing the same blur region two frames in a row uploads no
      texture on the second frame (observable: `end_pass()`
      `textures_delta.set` empty in a headless two-pass harness).
- [x] Different content at the same dest re-uploads (hash key, not a
      dest key).
- [x] An entry that stops being drawn is freed (its texture appears in
      `textures_delta.free` on the next frame's eviction) — no GPU
      memory growth across a long session.
- [x] `OverlayApp` construction requests no repaint
      (`!ctx.has_requested_repaint()` after `new`).
- [x] `show_toolbar` requests the convergence repaint iff the measured
      toolbar size differs from the size its anchor was computed with
      (transition-only, documented in code).
- [x] No unconditional `request_repaint` anywhere in `overlay.rs`.
- [x] Overlay module docs explain the on-demand contract (§Repaint
      discipline); the duplicated comment block is gone.

**Verification:**
- [x] `cargo fmt --all --check`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo test --workspace --all-targets`
- [x] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual (X11/XWayland): `flintshot region --overlay` idle for
      ~10 s with the pointer parked → process CPU near 0% (sampled
      via `/proc/<pid>/stat` utime+stime); hovering / dragging
      repaints visibly (magnifier + badge track the pointer).

**Dependencies:** Tasks 002–006 (the overlay being disciplined).

**Files likely touched:**
- `crates/flintshot-editor-egui/src/canvas.rs` — region texture
  cache + headless two-pass tests.
- `crates/flintshot-editor-egui/src/overlay.rs` — convergence
  repaint, construction test, module docs, comment cleanup.
- `CHANGELOG.md` — Unreleased entry.

**Estimated scope:** Medium (~200 lines incl. tests).
