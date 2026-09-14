# Task 007: Editor — Copy closes on Wayland

**Description:** `handle_copy` closes the window when the session is
Wayland (`WAYLAND_DISPLAY` set): the compositor owns the selection
under `zwlr_data_control`, so the X11 stay-open rule does not apply.

**Done:** pure platform-seam predicate + test; call site in
`app.rs`.

## Verification

- [x] `cargo test -p flintshot-editor-egui copy_closes`
- [ ] Live: copy → editor closes → paste elsewhere still yields the
      image on COSMIC (TEST_MATRIX row)
