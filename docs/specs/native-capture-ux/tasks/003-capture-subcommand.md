# Task 003: `flintshot capture` end-to-end (additive)

**Description:** The vertical slice: `flintshot capture` works
start-to-finish **alongside** the still-present `region`/`fullscreen`
(no removals yet — Task 5 purges). CLI: new `CaptureArgs`
(`--delay`, `--backend`, `--out`, `--standalone`, interaction flags;
**no** `--monitor`). Flow: `select_backend` → if
`supports_selection()`: `capture_selection` → editor (post-grab
routing is editor-only until Task 4) with **quiet cancel**
(`"cancelled"` error → info log, exit 0, no notification); else:
today's fallback flow (primary-monitor grab → editor/overlay, the
`region` semantics minus the monitor flag). IPC: new
`Request::Capture { delay_ms }` (old variants untouched); worker
gains `capture_argv`; daemon dispatch arm spawns it. Delegation
(detect-and-delegate, `--standalone` bypass) mirrors `region`'s.

**Acceptance criteria:**
- [ ] `flintshot capture --help` parses; flag set as specced;
      `--backend` validation reuses the existing helper
- [ ] Native flow: portal selected → interactive grab → editor
      opens with the selection (manual COSMIC check)
- [ ] Quiet cancel: Escape in the OS UI → exit 0, one info log, no
      error output/notification (pinned by a unit test on the
      cancel-classification helper)
- [ ] Fallback flow: `--backend xcap` reproduces today's `region`
      flow (primary monitor, no `--monitor`)
- [ ] `Request::Capture { delay_ms }` postcard round-trips; daemon
      dispatch spawns `capture_argv` with backend elision at default
      (tests mirror the existing argv tests)
- [ ] `flintshot region` / `fullscreen` behave exactly as before
      (nothing removed yet)

**Verification:**
- [ ] `cargo test -p flintshot-app --bins --quiet` green
- [ ] `cargo test --workspace --all-targets --quiet` green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` green
- [ ] Manual (COSMIC): `cargo run -- capture` (native + editor),
      `cargo run -- capture --backend xcap` (fallback),
      Escape quiet-exit

**Dependencies:** Task 2 (capability)

**Files likely touched:**
- `crates/flintshot-app/src/cli.rs` (subcommand + flows)
- `crates/flintshot-core/src/ipc.rs` (new variant + round-trip tests)
- `crates/flintshot-app/src/worker.rs` (`capture_argv` + dispatch)
- `crates/flintshot-app/src/daemon.rs` (dispatch arm)

**Docs:**
- GOTCHA: why quiet-cancel classifies on the `"cancelled"`
  substring → comment at the classification helper.

**Estimated scope:** Medium-Large (4 files; cli.rs carries the flow)
