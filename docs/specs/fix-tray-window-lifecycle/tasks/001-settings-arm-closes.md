# Task 001: Settings arm closes behind the handoff (ADR-0065 mirror)

**Description:** The worker editor's `TopbarAction::Settings` arm
currently fires the `open_settings` hook and *stays open* — its exact
twin `NavigateHome` (the ADR-0065 fix) fires the hook, publishes
`Cancelled`, and closes the viewport. Mirror the twin verbatim so a
worker-editor Settings click never leaves two windows on screen
(spec AC 1–3).

**Acceptance criteria:**
- [ ] RED first: `settings_with_hook_fires_it_then_cancels_and_closes` (mirror of `navigate_home_with_hook_fires_it_then_cancels_and_closes`) fails on current code — hook fired exactly once, outcome `Ok(Cancelled)` published, `ViewportCommand::Close` issued (assert via `close_command_issued` on a `ctx.run_ui` full output)
- [ ] Hook-absent worker `EditorApp`: `handle_topbar_action(Settings)` still fires nothing and now publishes `Ok(Cancelled)` + closes (mirror `navigate_home_publishes_cancelled_on_worker_windows`; publish-if-unset — never overwrite an existing outcome)
- [ ] Page-mode defensive arm: with the hook installed the handler still publishes + closes (mirror `page_navigate_home_with_hook_still_publishes_and_closes`); the router normally intercepts Settings before editor actions flow
- [ ] Existing `worker_overflow_settings_click_fires_the_open_settings_hook` stays green (popup → row click → hook fired; the added close does not break the frame harness)
- [ ] No shell.rs changes — hooks already exist (`with_open_settings` / `set_open_settings`)

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui -q` green
- [ ] Full gate: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --all-targets -q`, `cargo test --doc -p flintshot-core -p flintshot-editor-egui -q`
- [ ] Manual check deferred to the manual matrix (spec §Commands A/C 1–3)

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-editor-egui/src/app.rs` (Settings arm, tests)

**Docs:**
- The arm's comment explains delegate-then-close (mirror of the
  NavigateHome comment); ADR-0065 already records the stance — add
  the Settings mirror note in Task 008's CHANGELOG entry instead of a
  new ADR.

**Estimated scope:** Small: 1 file
