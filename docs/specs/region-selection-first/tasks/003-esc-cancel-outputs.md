# Task 003 — Esc-cancel contract + outputs-with-live-selection hardening

> Status: complete
> Effort: [`region-selection-first`](../spec.md) · Plan decision 5

## Description

Two hardening halves over task 001's phase.

**Esc-cancel contract.** During the live phase, Esc maps onto the
editor's standing cancel path: the dispatcher answers
`DispatchOutcome::Cancel`, `logic` publishes-if-unset
`Ok(EditorOutcome::Cancelled)` and closes the window, and the app
crate logs "no file written" and exits 0. The spec's split ("no
selection yet → cancel the capture; after a selection exists or the
phase ended → today's behaviour") collapses onto this one path *by
design* — today's editor Esc behaviour already is
close-as-`Cancelled` with no side effects, and a prior copy is
honoured via the if-unset publish (the editor stays open after
Copy, so the later Esc close carries the copy's outcome). Pinned by
unit tests: the phase survives a stray Esc (only a crop ends it),
Esc answers `Cancel` both with and without a live selection, and a
pre-published copy outcome wins over the Esc cancel. Two deliberate
non-changes, recorded here so they read as decisions:

- `[ui]` default persistence (`persist_ui_config`, Phase 1 Task 15)
  still runs on cancel. It fires on every editor exit including
  X-close; the spec's "no side effects" enumerates capture outputs
  (no file, no clipboard write, no worker), not the settings file.
- Esc while a text draft is active still cancels the *draft* only
  (the text-editing modal branch precedes the shortcut dispatch) —
  today's behaviour, kept.

**Outputs with a live selection.** Copy / Pin / Upload were already
selection-aware via `compose_selection_aware`; Save was not (the
Phase 1 full-image contract). Save now composes through a new pure
decision point, `compose_for_save(selection_aware, state)`, and
`EditorApp::handle_save` passes `self.phase_active()`: while the
phase is live, an active selection scopes the saved file (spec exit
criterion 3 — "cropping first is a convenience, not a requirement");
once the phase has ended, or was never requested (`fullscreen` /
`open`), the full-image contract stands, pinned by a test. The
overlay's `save_region_to_disk` collapses to
`save_to_disk(state, true)` (one save path, the flag is the
contract). Parity pinned: a live-selection Save emits
pixel-identical output to crop-then-Save for annotations fully
inside the region (straddling annotations legitimately differ —
geometric clip after crop vs pixel clip before — and sit outside
the parity claim).

## Acceptance criteria

1. Esc during the live phase (no selection) dispatches `Cancel`
   with no state side effects and does not end the phase (unit
   test).
2. Esc with a live selection keeps the standing
   close-as-`Cancelled` behaviour (unit test), and a pre-published
   copy outcome wins over the Esc cancel (if-unset publish, unit
   test).
3. Save during the live phase honours an active selection; after
   the phase ends (crop) or without a selection it saves the full
   image (unit tests).
4. Live-selection Save == crop-then-Save pixels for a
   fully-contained annotation (unit test).
5. Standing gates green: `fmt`, `clippy -D warnings`, workspace
   tests, doc tests.

## Verification

```sh
cargo test -p flintshot-editor-egui
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
```

Manual checkpoint (needs a display; Wayland + X11 editor-mode):
`cargo run -- region` → Esc before any drag exits 0 with no file,
no clipboard write, no worker; drag + Copy (uncropped) pastes the
region only; drag + Ctrl+S (uncropped) saves the region only;
Enter then Ctrl+S saves the cropped image; `cargo run -- fullscreen`
Save stays full-image.
