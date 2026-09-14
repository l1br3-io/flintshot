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
- [x] `flintshot capture --help` parses; flag set as specced;
      `--backend` validation reuses the existing helper
- [x] Native flow: portal selected → interactive grab → editor
      opens with the selection (verified pre-wedge via the spike
      probe; the live CLI check is the Phase 2 checkpoint after
      Task 4 — the session's selection UI is wedged, spike C
      addendum, until a reboot)
- [x] Quiet cancel: Escape in the OS UI → exit 0, one info log, no
      error output/notification (pinned by a unit test on the
      cancel-classification helper — covers both `"cancelled"` and
      the interactive `"timed out"` hang)
- [x] Fallback flow: `--backend xcap` reproduces today's `region`
      flow (primary monitor, no `--monitor`) — verified live via
      `--out` (2560×1440 PNG written)
- [x] `Request::Capture { delay_ms }` postcard round-trips; daemon
      dispatch spawns `capture_argv` with backend elision at default
      (tests mirror the existing argv tests; the daemon arm itself
      needed no edit — routing flows through
      `spawns_capture_worker`/`dispatch`, both updated)
- [x] `flintshot region` / `fullscreen` behave exactly as before
      (nothing removed yet)

**Verification:**
- [x] `cargo test -p flintshot-app --bins --quiet` green
- [x] `cargo test --workspace --all-targets --quiet` green
- [x] `cargo clippy --workspace --all-targets -- -D warnings` green
- [ ] Manual (COSMIC): `cargo run -- capture` (native + editor),
      Escape quiet-exit — deferred to the Phase 2 checkpoint
      (post-reboot; `capture --backend xcap` fallback verified via
      `--out` headlessly)

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
