# Task 001: IPC contract — `Request::BindShortcuts` + `flintshot hotkeys bind` CLI

**Description:** Land the client ↔ daemon contract before any portal
code, mirroring how Phase 2 staged `flintshot-core::ipc`. Add the
`Request::BindShortcuts` variant to the postcard schema (client →
daemon: open the GlobalShortcuts bind dialog), a defensive no-op arm
in `worker::dispatch` (alongside `Subscribe`/`ReportSaved` — the
IPC server intercepts it before dispatch in Task 003), and the CLI
surface: `flintshot hotkeys bind`, which sends the request over IPC
and errors clearly when no daemon is running. This task ships no
behavior change beyond a clean "unsupported" response for the new
request.

**Acceptance criteria:**
- [ ] `Request::BindShortcuts` variant exists in
      `flintshot_core::ipc::Request`, doc-commented (non_exhaustive
      enum — additive), with a postcard roundtrip test case.
- [ ] `worker::dispatch` handles `Request::BindShortcuts` with a
      defensive `DispatchOutcome::Continue` arm (never dispatched
      directly; the IPC server intercepts it — same pattern as
      `Subscribe`).
- [ ] `flintshot hotkeys bind` parses (clap), sends the request to
      the daemon over IPC, and exits non-zero with the message
      "no running daemon; start `flintshot daemon` first" when
      connection fails.
- [ ] Against a daemon built *without* Task 003's interception, the
      CLI gets a defined outcome (connection-level or unsupported
      error), never a hang or protocol error.
- [ ] `cargo run -- hotkeys --help` renders sensibly.

**Verification:**
- [ ] `cargo test --quiet -p flintshot-core` (roundtrip test)
- [ ] `cargo test --quiet --workspace --all-targets`
- [ ] `cargo run --quiet -- hotkeys --help` (manual smoke)
- [ ] `cargo run --quiet -- hotkeys bind` with no daemon → error +
      non-zero exit (manual smoke)

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-core/src/ipc.rs` (variant + roundtrip case)
- `crates/flintshot-app/src/worker.rs` (defensive arm)
- `crates/flintshot-app/src/cli.rs` (`HotkeysArgs` + run fn)
- `crates/flintshot-app/src/main.rs` (wire command, if cli.rs
  pattern requires)

**Docs:**
- Gotchas → inline comment at the dispatch arm explaining why it is
  intercepted (mirrors the `Subscribe` comment block).
- CHANGELOG `[Unreleased]` line for the new subcommand comes with
  Task 005 (user-visible surface lands complete there).

**Estimated scope:** Small (3–4 files, all pre-existing patterns)
