# Task 011: Settings consumers (gates + output save path)

**Description:** Wire the config fields into their consumers:
`ui.show-statusbar` gates Task 3's statusbar; `ui.statusbar-size-mode`
switches heuristic/exact byte estimate; `ui.animations` feeds
`widgets::animate` (all spring microinteractions no-op when off);
`capture.default-mode` seeds the `+`/topbar Capture path's mode;
`capture.after-grab` clipboard semantics confirmed and honored;
`output.*` drives a direct-save path — when `filename-template` is
set, Ctrl+S/overflow Save writes `save-dir/template.ext` with the
configured format + JPEG quality, else today's dialog. Worker windows
read the config snapshot at construction (no watcher there).

**Acceptance criteria:**
- [ ] Each field's consumer verified: statusbar hides/shows live;
      size mode flips the statusbar value (heuristic instant, exact
      encodes PNG); animations off = no spring motion anywhere
      (sections, chips, rail pop, hero pop-in).
- [ ] Direct-save: template + dir + format + jpeg-quality compose the
      output path/encoding; collision handling defined (append `-2`
      before ext); dialog fallback when template empty; tested at the
      pipeline seam.
- [ ] Default capture mode honored by the topbar Capture action;
      after-grab clipboard path confirmed against the daemon flow
      (no regression to ADR-0047 choreography).
- [ ] Config round-trip + old-config tests still green; no new
      failures in daemon/worker tests.

**Verification:**
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: settings → toggle each row → observe live effect;
      restart → persisted; Ctrl+S with template set writes the
      expected file (PNG and JPEG paths)

**Dependencies:** Tasks 3, 9, 10

**Files likely touched:**
- `crates/flintshot-editor-egui/src/app.rs`
- `crates/flintshot-editor-egui/src/statusbar.rs`
- `crates/flintshot-editor-egui/src/widgets.rs` (gate consumption)
- `crates/flintshot-app/src/capture_flow.rs` (default mode seam, if
  not already plumbed)

**Estimated scope:** Medium (3-4 files)
