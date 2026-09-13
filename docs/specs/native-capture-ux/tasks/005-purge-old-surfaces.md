# Task 005: Purge `region`/`fullscreen` + migrate all senders

**Description:** The clean break (per decision: no aliases). Delete
`flintshot region` / `flintshot fullscreen` (commands, args structs,
runners, `--monitor` plumbing), the `Request::CaptureRegion` /
`CaptureFullscreen` IPC variants, and `region_argv`/`fullscreen_argv`
(kept as `capture_argv`). Migrate every remaining sender to
`Request::Capture` — mechanically, no UX changes in this task:
`app_window.rs` bridge (both request methods send `Capture`),
`tray.rs` (menu items still two; both send `Capture`; per-monitor
IDs send `Capture` with the monitor dropped), `hotkeys_portal.rs`
(REGION_ID/FULLSCREEN_ID both map to `Capture`), `cli.rs` delegation
paths, `ipc_client.rs` senders. Daemon dispatch arms collapse to
one. Tests updated everywhere; a pinned test asserts `region`/
`fullscreen` **fail to parse** and the old IPC variants are gone.

**Acceptance criteria:**
- [ ] `flintshot region …` / `flintshot fullscreen …` fail to parse
      (pinned by CLI test); `flintshot capture` unaffected
- [ ] `Request` enum has exactly one capture variant; old variants
      don't compile (postcard round-trip test updated)
- [ ] All senders (`app_window`, `tray`, `hotkeys_portal`,
      `ipc_client`, CLI delegation) compile against `Capture`
- [ ] Tray/hotkeys still *work* (redundant duplicate actions are
      fine until Tasks 7/9 collapse the UX)
- [ ] Workspace gate green incl. doc tests

**Verification:**
- [ ] `cargo test --workspace --all-targets --quiet` green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` green
- [ ] Manual: tray *Capture region* still captures (via the new
      request); hotkey fires; `flintshot region` errors

**Dependencies:** Tasks 3-4 (capture flow complete)

**Files likely touched:**
- `crates/flintshot-app/src/cli.rs`, `worker.rs`, `daemon.rs`,
  `app_window.rs`, `tray.rs`, `hotkeys_portal.rs`, `ipc_client.rs`
- `crates/flintshot-core/src/ipc.rs`

**Docs:**
- GOTCHA: none expected — mechanical migration.

**Estimated scope:** Large (7-8 files, all mechanical)
