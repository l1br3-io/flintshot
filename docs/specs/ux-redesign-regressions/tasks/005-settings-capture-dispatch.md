# Task 005: Settings-route Capture dispatches (dead control)

**Description:** The enable matrix keeps Capture live on the Settings
route (`enable_for(Surface::Settings) = CAPTURE`), but the router's
dispatch arm requires `Route::Home`
(`window.rs` ui_impl: `Some(TopbarAction::Capture) if matches!(self.route,
Route::Home)`) — a Capture click on Settings falls through to `_ => {}`
and does nothing. `HomeApp` is always constructed and owns the
capture path (`start_capture`: bridge request + close, ADR-0047
choreography), so the fix is extending the guard to
`Route::Home | Route::Settings`. The close classifies as a capture
intent (pending capture beats hide), the outer loop grabs, and the
restore reopens Settings per the same-page restore rule.

**Acceptance criteria:**
- [ ] Clicking the topbar Capture on the Settings route fires
      `request_capture` on the home bridge exactly once and issues the
      window Close (headless click test through `WindowApp::ui_impl`
      on the Settings route, mirroring
      `home_topbar_capture_fires_the_bridge_request`)
- [ ] Home-route behavior unchanged (existing test still green)
- [ ] Loading stays fully disabled (existing matrix test untouched)

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui window::` (quiet)
- [ ] `cargo fmt --all` && `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: app → ⋯ → Settings → Capture starts a capture; Settings
      restores after the grab resolves

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-editor-egui/src/window.rs` (dispatch guard + test)

**Docs:**
- None beyond the guard comment (the arm list's route conditions are
  the contract; the test pins it).

**Estimated scope:** Small: 1 file

**Git Commit:** `git commit -m "fix(window): Capture fires on the Settings route — the enabled hero was a dead control"`
