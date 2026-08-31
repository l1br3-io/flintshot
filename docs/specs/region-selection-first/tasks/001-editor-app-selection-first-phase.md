# Task 001 — `EditorApp` selection-first phase

> Status: complete
> Effort: [`region-selection-first`](../spec.md) · Plan decision 1–3

## Description

Port the overlay's landing experience into the editor window as a
shell-owned phase on `EditorApp`:

- `EguiShell::with_selection_first()` builder flag (mirrors
  `with_pin_config`; no `EditorShell` trait change), plumbed through
  `run_editor_blocking` into `EditorApp::new`.
- Select pre-arm: `state.tool = Tool::Select` at construction when
  requested (same seeding as `OverlayApp::new`) — `EditorState` is
  only seeded, never extended; zero core changes.
- Dim mask outside the live selection, rendered from the same
  four-rect math + `MASK_ALPHA` as the overlay: the mask code is
  extracted into shared `pub(crate)` helpers beside the overlay's
  chrome helpers, and the overlay now calls them too — no second
  constant to drift.
- One-line hint ("drag to select · Enter crop · Esc cancel") painted
  during the phase, badge-style (dark pill, white text).
- Phase ends on first crop, permanently: a one-way latch keyed on the
  undo stack gaining a full snapshot (`EditorState::crop` is the only
  mutation that pushes one). Undo after crop must not resurrect the
  chrome.

## Acceptance criteria

1. `EguiShell::new().with_selection_first()` flags the shell; default
   construction does not (unit test).
2. `EditorApp::new(..., selection_first = true)` arms `Tool::Select`
   and reports the phase active; `false` leaves the `Tool::Rect`
   default and no phase (unit tests).
3. Both crop paths — the dispatcher's Enter and the toolbar's
   "Crop · Enter" button — end the phase on the frame they crop
   (unit tests).
4. Undo after crop does not revive the phase; annotation commits
   (anns-only snapshots) never end it (unit tests).
5. Mask math shared with the overlay: no selection → whole panel;
   selection → four bands framing it; edge selections produce
   degenerate bands, never inverted ones (unit tests).
6. The overlay's rendered mask is unchanged (it now calls the same
   shared helpers).
7. Standing gates green: `fmt`, `clippy -D warnings`, workspace
   tests, doc tests.

## Verification

```sh
cargo test -p flintshot-editor-egui
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
```
