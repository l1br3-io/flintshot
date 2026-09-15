# Task 006: Editor joins the router — in-window swaps

**Description:** The Editor becomes a route inside `run_window`,
and non-capture transitions stop destroying the window. Open /
Paste (from home or from the editor) swap to the Editor route
in-place — `HomeApp` no longer sends `ViewportCommand::Close` after
a routed Open/Paste; it publishes a route request the router
consumes (bridge contracts unchanged). Esc in the editor swaps to
Home instead of closing. Image swaps (a second Open/Paste, or a new
grab while the window is open) replace the route's editor state
in-place. Capture intents (`+` Capture, home Capture, page C)
still register intent + Close — destruction stays mandatory for
grabs. The app crate's outer loop shrinks accordingly:
`CloseReason::OpenImage` and `GoHome` disappear; the router's
current route feeds the same-page restore (an external capture's
close reopens on the route that was open); `run_editor_page`,
`PendingEditor`'s close-classification path, and the
`EditorPageExit` home/capture arms collapse into route intents.
The worker path (`run_editor_blocking`) is untouched.

**Acceptance criteria:**
- [ ] Home → editor via Open/Paste, editor → home via Esc, and
      image swaps all happen without window destruction (same
      surface identity; unit-test the route state machine + the
      HomeApp no-close contract)
- [ ] Capture intents still close the window; the outer loop
      classifies only capture-intent / hidden-by-event / user-quit
- [ ] External-capture close restores the route that was open
      (same-page restore preserved)
- [ ] Editor-page behaviors survive as routes: side-effect outcomes
      report through the bridge and keep the page; Hide directive
      still closes; Esc layering (selection clear → cancel) routes
      home, not close
- [ ] `run_editor_page` and `EditorPageExit`'s home/close arms are
      gone; ported tests cover the router equivalents

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui -p flintshot-app` green
      (ported + new route tests)
- [ ] Full gate green (see Task 001's gate commands)
- [ ] Manual: Open → editor appears (no WM close/open animation);
      Esc → home; Open again → new image in-place; `+` Capture →
      window closes for the grab, editor returns with the fresh
      image; taskbar/pinning identity stable throughout

**Dependencies:** Task 005 (the router), Task 004 (editor route
geometry)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/{window,shell,app,home}.rs`
- `crates/flintshot-app/src/app_window.rs`

**Docs:**
- GOTCHA (any winit/Wayland resize quirk found) → inline comment at
  the viewport-batch site.

**Estimated scope:** Medium-Large: 5 files
