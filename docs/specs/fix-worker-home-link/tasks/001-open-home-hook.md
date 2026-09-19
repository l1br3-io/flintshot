# Task 001: `open_home` hook seam + `NavigateHome` handler fires it

**Description:** Give the worker editor a Home delegate seam,
shape-mating `open_settings`: an `open_home` hook field on
`EditorApp` (`set_open_home`), a `with_open_home` builder on
`EguiShell` plumbed through `run_editor_blocking`, and the revised
`TopbarAction::NavigateHome` arm that fires the hook (when present)
before the Esc contract (publish-if-unset `Cancelled` +
`ViewportCommand::Close`). Unit-testable with a recording hook; the
real delegate lands in Task 002.

**Acceptance criteria:**
- [ ] `EguiShell::new().with_open_home(f)` stores the hook; default shell has none (test mirrors `with_open_settings_stores_the_hook`)
- [ ] `run_editor_blocking` threads the hook into `EditorApp` via `set_open_home` (same plumbing shape as the settings hook)
- [ ] Worker `EditorApp` with a hook installed: `handle_topbar_action(NavigateHome)` fires the hook exactly once, then publishes `Ok(Cancelled)` and issues `ViewportCommand::Close` (assert via `close_command_issued` on a `ctx.run_ui` full output)
- [ ] Worker `EditorApp` without a hook: existing behavior preserved — `navigate_home_publishes_cancelled_on_worker_windows` stays green, updated only if its comment drifts
- [ ] Page-mode `EditorApp` with a hook installed: handler still publishes + closes (defensive arm — the router normally intercepts `NavigateHome` before editor actions flow)
- [ ] Hook fires before the outcome publishes (ordering pinned by asserting both effects after one call, with the hook recording into shared state)

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui` green
- [ ] `cargo build` succeeds
- [ ] `cargo fmt --all --check` + `cargo clippy --workspace --all-targets -- -D warnings` clean

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-editor-egui/src/app.rs` (field, setter, `NavigateHome` arm, tests)
- `crates/flintshot-editor-egui/src/shell.rs` (builder, `run_editor_blocking` plumbing, builder test)

**Docs:**
- The revised arm's comment explains the delegate-then-close
  ordering (the "why" is non-obvious). No ADR yet — Task 003.

**Estimated scope:** Small: 2 files
