# Task 008: Real `EditorOpened` (worker-reported)

**Description:** Replace the process-spawn proxy with a truthful
window-open signal.

Worker side (`cli.rs`):
- `run_capture_shell`: just before opening the resolved shell
  (editor or overlay), send best-effort
  `Request::ReportEditorWindowOpened` (new `ipc_client` helper,
  `ReportSaved`-style). Also inside the recapture loops
  (`run_editor_with_config` / `run_overlay_with_config`) before
  each re-open — one event per actual window.
- `run_open` (the `open` subcommand): report before the editor runs.
- `--out` headless path never reports (nothing opens) — falls out
  naturally, add a regression test if cheap.

Daemon side:
- `ipc_server` intercepts `ReportEditorWindowOpened` → broadcast
  `Event::EditorOpened` (mirrors the `ReportSaved` interception).
- `worker.rs::dispatch`: remove the optimistic `sink(EditorOpened)`
  from the `CaptureRegion`/`CaptureFullscreen`/`OpenLast` arms
  (spawn no longer implies a window). PinLast never emitted it
  anyway (verify + leave).
- Audit subscribers before removing the proxy: grep
  `EditorOpened` consumers (app window `HomeCore` ignores it;
  check `flintshot-cosmic` menu/applet — it's not in
  default-members; `cargo build -p flintshot-cosmic` if system
  deps allow, else visual grep + note in TEST_MATRIX).

**Acceptance criteria:**
- [ ] Dispatch tests updated: capture arms emit `CaptureStarted`
      (+ `CaptureSuperseded` when superseding) and **no**
      `EditorOpened`; spawn-failure expectations adjusted
- [ ] Interception test: `ReportEditorWindowOpened` → one
      `EditorOpened` broadcast per report
- [ ] Recapture chain: a chain of two sessions reports twice
      (unit-test the loop decision if the report call is seam'd;
      otherwise manual)
- [ ] No `EditorOpened` on `--out` captures (test where the
      `--out` split happens in `capture_and_dispatch`)

**Verification:**
- [ ] `cargo test --workspace --all-targets` + doc tests
- [ ] Standing gates
- [ ] Manual: capture → editor → `+` chip recapture → subscriber
      (app window status / applet) sees one `EditorOpened` per
      window, none for `--out`

**Dependencies:** 001 (variant); independent of 002–007 otherwise
but ordered last so dispatch-test churn lands once

**Files likely touched:**
- `crates/flintshot-app/src/cli.rs`
- `crates/flintshot-app/src/ipc_client.rs`
- `crates/flintshot-app/src/ipc_server.rs`
- `crates/flintshot-app/src/worker.rs`

**Docs:** `Event::EditorOpened` doc comment rewritten: "an
annotation window actually opened" (the old spawn-proxy wording
removed).

**Estimated scope:** Medium: 4 files
