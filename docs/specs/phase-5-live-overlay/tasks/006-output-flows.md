# Task 006: Output flows — Enter=Copy, Save dialog, Pin/Upload workers, outcome publication

**Description:** Wire the overlay's outputs to the Phase 1–4
pipelines (plan decision 6): **Enter = Copy** (Flameshot muscle
memory; the toolbar Copy chip and `Ctrl+C` are Enter-equivalent),
`Esc` = Cancel (already shipped in Task 002), Save opens the `rfd`
dialog, Pin / Upload write a temp PNG and spawn their detached
workers, and every close publishes the real `EditorOutcome` —
replacing the Task 002/004 accept-and-notice placeholders.

Reuse is verbatim where the editor window's helpers already match
the overlay's semantics:

- **Copy** — `copy_to_clipboard` is already selection-aware (a
  non-degenerate selection emits just the region; the toolbar only
  exists with one). Reused as-is.
- **Pin / Upload** — `compose_and_write_pin` /
  `compose_and_write_upload` + `spawn_pin_worker` /
  `spawn_upload_worker` are already selection-aware; reused as-is
  with the editor window's error policy (compose/write failure is
  terminal; a spawn failure logs and leaves the overlay open).
- **Save** — the editor's `save_to_disk` composes the *full* image
  (its Phase 1 contract), but the overlay's model is "the region
  defines what ships in the output" (Task 005's own words). The
  overlay Save therefore composes selection-aware — the one helper
  this task adds (`save_region_to_disk`). The editor window's Save
  is untouched (exit criterion 5: byte-identical editor mode).
- The three copies of the selection-aware compose `match` (copy /
  pin / upload) collapse into one shared
  `compose_selection_aware`, used by all of the above.

**Chords:** `Ctrl+S` / `Ctrl+C` route through the dispatcher (it
already recognizes them and returns `Save` / `Copy` with no state
mutation — no second dispatcher); the shell reacts to the returned
outcome exactly as `EditorApp::logic` does.

**X11 clipboard-owner adaptation** (plan decision 6: "Copy keeps
the process alive — same policy as the editor window's 'Copied'
state, adapted to overlay close semantics"): the editor window
keeps the window (and process) open after a copy so the owner-based
X11 selection survives until the user pastes. The overlay must
close on Enter — Flameshot semantics — so instead the *process*
lingers after the window is gone: `OverlayApp` hands its
`arboard::Clipboard` handle to a slot shared with
`run_overlay_blocking`, which — only when the outcome is
`CopiedToClipboard` — holds the handle for a bounded grace period
(60 s) before returning, keeping the X11 selection alive long
enough to paste. macOS / Windows clipboards are OS-owned, so the
linger duration is 0 there (the code path stays uniform and
testable).

**Acceptance criteria:**
- [x] `Enter` / ✓-chip / Copy-chip / `Ctrl+C` compose the region
      (selection-aware) onto the clipboard, publish
      `Ok(EditorOutcome::CopiedToClipboard)`, and close the
      overlay. A clipboard failure publishes `Err` and closes
      (same policy as the editor window's copy failure).
- [x] After a copy-close, the process lingers holding the
      clipboard handle for the grace period (60 s on X11-class
      platforms; 0 on macOS/Windows); no linger for any other
      outcome.
- [x] `Esc` still publishes `Cancelled` with no side effects.
- [x] Save chip / `Ctrl+S`: `rfd` dialog → selection-aware PNG →
      publish `Saved(path)` + close; dialog cancel keeps the
      overlay open (no outcome); write failure publishes `Err` +
      close.
- [x] Pin / Upload chips: temp PNG + detached worker spawn,
      publishing `Pinned` / `Uploaded(String::new())` + close;
      spawn failure logs and leaves the overlay open; compose
      failure publishes `Err` + close — the editor window's policy
      verbatim.
- [x] `compose_selection_aware` is the single selection-aware
      compose (no-selection / degenerate → full image; usable
      selection → `compose_final_region`), shared by copy / pin /
      upload / overlay-save.
- [x] Overlay chord dispatch pinned: `Char('s')`+ctrl →
      `DispatchOutcome::Save`, `Char('c')`+ctrl → `Copy`, plain
      (unmodified) letters → `Continue` — so wiring the chords
      cannot regress single-key tool selection.
- [x] Editor window behavior byte-identical (shared helpers only,
      no editor-path changes).

**Verification:**
- [x] `cargo fmt --all --check`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo test --workspace --all-targets` — incl. the new
      compose / chord / linger-decision tests.
- [x] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual (X11, real desktop session): drag a region, scribble,
      press Enter — overlay closes, paste elsewhere yields the
      annotated region (paste again ~1 min later still works);
      Save chip opens the dialog and writes a region-sized PNG;
      Pin spawns a pin window; Upload spawns the worker and
      notifies; Esc mid-flow writes nothing; Ctrl+C copies; the
      process exits ~60 s after a copy and immediately after
      save/pin/upload/cancel.

**Dependencies:** Task 005 (annotation confinement — the overlay
canvas the outputs compose from).

**Files likely touched:**
- `crates/flintshot-editor-egui/src/app.rs` —
  `compose_selection_aware`, `save_region_to_disk`, helper
  visibility (`pub(crate)`), compose tests.
- `crates/flintshot-editor-egui/src/overlay.rs` —
  accept→copy pipeline, Save/Pin/Upload handlers, `Ctrl+S` /
  `Ctrl+C` chord dispatch, clipboard handle handoff, chord tests,
  module docs.
- `crates/flintshot-editor-egui/src/shell.rs` — clipboard slot +
  `clipboard_linger_needed` + bounded linger, decision tests.
- `crates/flintshot-app/src/cli.rs` — stale doc comment only.
- `CHANGELOG.md` — Unreleased entry.

**Estimated scope:** Medium (~300 lines across three modules, no
core changes).
