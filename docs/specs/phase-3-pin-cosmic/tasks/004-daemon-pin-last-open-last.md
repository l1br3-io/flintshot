# Task 4: Daemon `PinLast` / `OpenLast` dispatch

**Description:** Make the daemon act on the two Phase 3 IPC requests
that are currently "unsupported" (`worker.rs:117` wildcard):
`Request::PinLast` (spawn `flintshot pin <last-saved>`, emit
`Event::Pinned { id }`) and `Request::OpenLast` (spawn `flintshot
fullscreen`-style editor re-open on the last-saved path). Both read the
last-saved state Task 3 introduced. `PinLast` with no prior save emits
`Event::Error { code: "no-last-capture" }` (matching the Phase 2
unsupported-error pattern). The pin id in `Event::Pinned` is a
daemon-assigned monotonic counter.

**Acceptance criteria:**
- [ ] `worker::dispatch` (`worker.rs:89`) handles `Request::PinLast`:
      read the last-saved path from the shared state (Task 3); if
      `None`, emit `Event::Error { code: "no-last-capture", message:
      … }` and return `Continue`; if `Some(path)`, spawn `flintshot pin
      <path>` via the spawner, emit `Event::Pinned { id }` with the
      next monotonic id, return `Continue`.
- [ ] `worker::dispatch` handles `Request::OpenLast`: read last-saved;
      if `None`, `Event::Error { code: "no-last-capture" }`; if
      `Some(path)`, spawn the editor on that path. (Decide: reuse the
      existing `region`/`fullscreen` argv with an image-load path, or a
      dedicated `flintshot open <path>` subcommand. Recommend the
      latter — a thin `open` subcommand that loads + opens the editor —
      since `region`/`fullscreen` capture fresh. Surface in `/planning`
      if it grows.)
- [ ] The monotonic pin id source: a `AtomicU64` (or `Arc<Mutex<u64>>`)
      in the daemon, incremented per pin. Shared into `dispatch` via
      the spawner/state bundle (dispatch may need to widen its signature
      to accept the last-saved + id state, OR the connection layer
      resolves `PinLast`/`OpenLast` before calling `dispatch` — pick the
      cleaner seam).
- [ ] `RecordingSpawner` tests (`worker.rs:179`) extended:
      `PinLast` with a last-saved path → spawns `pin <path>`, emits
      `Pinned { id: N }`; second `PinLast` → `id: N+1`; `PinLast` with
      no last-saved → `Error { code: "no-last-capture" }`, no spawn.
      `OpenLast` mirrors.
- [ ] `variant_name` (`worker.rs:166`) already covers `pin-last` /
      `open-last` (verify); the unsupported-test
      (`unsupported_requests_emit_error_and_continue`, `worker.rs:320`)
      removes `PinLast`/`OpenLast` from its unsupported cases list (they
      are now supported) and asserts the new behaviour instead.

**Verification:**
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-targets` — incl. the new
      `RecordingSpawner` dispatch tests above; the existing
      `unsupported_requests_emit_error_and_continue` updated (no longer
      lists `PinLast`/`OpenLast`).
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual: `flintshot daemon` + `flintshot region` → save (Task 3
      reports it) → from a test IPC client (or tray/applet later), send
      `PinLast` → a pin window floats; send again → second pin with id
      2. `OpenLast` → editor re-opens on the saved path. `PinLast`
      before any save → `Error { code: "no-last-capture" }`.

**Dependencies:** Task 3 (needs the last-saved state). Task 2 (needs
`flintshot pin <path>` to actually open a window — but dispatch can be
unit-tested with `RecordingSpawner` before Task 2 lands; the manual
verify needs Task 2).

**Files likely touched:**
- `crates/flintshot-app/src/worker.rs` — `PinLast`/`OpenLast` dispatch arms; monotonic id source; updated tests.
- `crates/flintshot-app/src/ipc_server.rs` — pass last-saved state + id source into `handle_connection`/`dispatch`.
- `crates/flintshot-app/src/daemon.rs` — wire the id source + state into the server.
- `crates/flintshot-app/src/cli.rs` — possibly a new `open <path>` subcommand (if Task 4 chooses that over argv reuse).

**Docs:**
- `CHANGELOG.md` `[Unreleased]` → Added: daemon `PinLast`/`OpenLast`
  support, `Event::Pinned` emission, monotonic pin ids.
- If a new `flintshot open` subcommand is added, document it in
  `docs/ARCHITECTURE.md` §Process model subcommand list.

**Estimated scope:** Medium (3–4 files; the dispatch + state-threading is the substantive part).
