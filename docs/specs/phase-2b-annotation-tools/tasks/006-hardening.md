# Task 006: Hardening (boundary test, CHANGELOG, ADR-0022, TEST_MATRIX, doc cleanups)

**Description:** Close-out sweep after all five tools + the magnifier
land. Verify the core boundary holds, record the effort in
CHANGELOG, confirm ADR-0022 is landed, add TEST_MATRIX rows, and sweep
stale doc comments surfaced during the build (notably the `canvas.rs`
module doc that Task 001 should already fix — re-verify here). This is
the task that proves the spec's exit criteria 1-7 all hold together.

**Acceptance criteria:**
- [ ] `no_gui_types_in_core.rs` green — confirms no egui types leaked
      into `flintshot-core` from the new tools or the magnifier config
      fields. (Test unchanged; must stay green. If any new `use egui`
      crept in, fix the leak, don't weaken the test.)
- [ ] `CHANGELOG.md` `[Unreleased]` has a Phase 2b subheading listing:
      Ellipse tool, Line tool, Marker tool (translucency), Pencil tool
      (smoothing), Number tool, Magnifier. Note the latent
      `ImageCanvas::draw_ellipse` save/copy bug fix (from Task 001).
- [ ] `docs/adrs/ADR-0022-*.md` landed (from Task 003) — verify it
      exists and follows the Context · Decision · Consequences
      format; cross-link from `docs/adrs/README.md` index.
- [ ] `docs/TEST_MATRIX.md` has new rows for each of the five tools
      (pending preview, commit, undo, select, hit-test) and the
      magnifier (modifier-hold show/hide, zoom factor, always-on
      toggle). Platforms: X11 / Wayland / macOS / Windows where
      applicable.
- [ ] All stale doc comments swept (notably `canvas.rs:23-25` —
      re-verify Task 001 fixed it; scan for any other "stub"/"Task NN"
      references that the new impls invalidate).
- [ ] Spec exit criteria 1-7 manually verified end-to-end in a single
      editor session over one capture (the full walk-through from
      `spec.md` §Exit criteria).
- [ ] Four-command gate green on Linux; `cargo build` green on macOS
      and Windows (cross-compile smoke where a native run isn't
      possible in CI).

**Verification:**
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual full walk-through of `spec.md` §Exit criteria steps 1-7
      (Ellipse, Line, Marker, Pencil, Number, Magnifier, save-and-
      reopen WYSIWYG).
- [ ] `python3 .opencode/harness/scripts/lint-plan.py` passes (plan
      stayed a thin index).
- [ ] `python3 .opencode/harness/scripts/check-refs.py` passes (spec /
      plan / task cross-links resolve).

**Dependencies:** Tasks 001-005 (all must be complete).

**Files likely touched:**
- `CHANGELOG.md`
- `docs/adrs/README.md` (index line for ADR-0022, if not added in 003)
- `docs/TEST_MATRIX.md`
- Possibly `crates/flintshot-editor-egui/src/canvas.rs` (doc sweep —
  should already be fixed in 001; verify only).

**Docs:**
- This task *is* the docs sweep. No further doc artefact beyond the
  files listed above.

**Estimated scope:** Small (3-4 files; mostly verification + doc
edits, little to no logic).
