# Task 6: COSMIC IPC client (framing + Request + Subscribe + daemon-absent)

**Description:** Build the pure-logic IPC client that the COSMIC applet
(Tasks 7–8) speaks to the daemon through. A new
`crates/flintshot-cosmic/src/ipc.rs` module: connect to the daemon
socket, length-prefixed postcard framing (duplicated ~30 LOC from
`flintshot-app::ipc_server` per ADR-0010 — schema in core, transport in
bins), send a `Request`, and maintain a long-lived `Subscribe`
connection that reads `Event` frames. Detect daemon-absent (missing
socket / connection refused) distinctly from a real fault, mirroring
`ipc_client::is_no_daemon` (`ipc_client.rs:71`). This task is
testable **headless against an in-process `IpcServer`** (no libcosmic)
— the UI wiring is Tasks 7–8.

**Acceptance criteria:**
- [ ] New `crates/flintshot-cosmic/src/ipc.rs` with:
      - `read_frame` / `write_frame` / `encode_msg` / `decode` helpers
        (mirror `flintshot-app/src/ipc_server.rs:43`/`:66`/`:91` — same
        u32-LE length-prefix, 1 MiB cap).
      - `send_request(request: &Request)` — connect, send one frame,
        disconnect (for one-shot actions: CaptureRegion, Quit, …).
      - `subscribe() -> EventStream` — connect, send `Subscribe`, return
        a handle that reads `Event` frames until disconnected.
      - `DaemonState` enum (`Running` / `NotRunning`) from a
        `probe_daemon()` that checks socket presence + connect.
- [ ] `flintshot-cosmic/Cargo.toml` gains `interprocess`, `postcard`,
      `tokio` (with `rt`/`io-util`/`net` as needed — minimal), and
      `flintshot-core` is already present (`Cargo.toml:17`). **No**
      `flintshot-app`, **no** `egui` dependency.
- [ ] `Lagged` / disconnect on the `Subscribe` stream is handled
      without panicking (mirror `ipc_server.rs:296`); a reconnect
      strategy or a clean "stream lost" signal to the caller.
- [ ] Unit tests against an in-process `flintshot_app::ipc_server::IpcServer`
      are **not** possible (the applet must not depend on
      `flintshot-app`). Instead, spin up a raw `interprocess` listener
      in the test that speaks the framing, and assert the client's
      send/receive. (The framing is identical, so a hand-rolled test
      listener is ~40 LOC.)
- [ ] `probe_daemon()` returns `NotRunning` for a missing socket /
      refused connection; `Running` when a socket accepts. Reuses the
      `socket_path()` resolver from `flintshot_core::ipc`.

**Verification:**
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` (note:
      `flintshot-cosmic` is not in default-members; clippy needs
      `-p flintshot-cosmic` or `--workspace` with the crate present —
      verify the gate covers it, as Phase 2 did).
- [ ] `cargo test -p flintshot-cosmic` — incl. the framing round-trip
      against a test listener; `subscribe()` receives expected `Event`s
      in order; `probe_daemon()` on an absent path returns
      `NotRunning`.
- [ ] `cargo build -p flintshot-cosmic` green on Linux (libcosmic
      system deps present); no libcosmic calls yet (Task 7 wires UI).

**Dependencies:** None from this effort (depends only on
`flintshot-core::ipc`, already present). Independent of Tasks 1–5;
could start in parallel from Task 3 onward.

**Files likely touched:**
- `crates/flintshot-cosmic/src/ipc.rs` — NEW: framing + `send_request` + `subscribe` + `probe_daemon`.
- `crates/flintshot-cosmic/Cargo.toml` — add `interprocess`, `postcard`, `tokio` (minimal features).
- `crates/flintshot-cosmic/src/main.rs` — minimal wiring so the module compiles (full applet is Task 7).

**Docs:**
- A `// GOTCHA:` comment if the duplicated framing ever risks drifting
  from `flintshot-app::ipc_server` (e.g. "if the wire format changes,
  update both"). Consider a shared test that encodes the same `Request`
  in both crates — flag in `/planning`.

**Estimated scope:** Medium (2–3 files; the framing + client logic is the substantive part; testable headless).
