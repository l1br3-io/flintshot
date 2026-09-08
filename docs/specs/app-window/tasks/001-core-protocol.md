# Task 001 — Core protocol: `OpenApp`, `SessionEnded`, `AppFocus`

## What

- `flintshot-core/src/ipc.rs`:
  - `Request::OpenApp` — client → daemon: open (or focus) the app
    window. Daemon dispatch spawns the `app` worker; the connection
    layer also broadcasts `Event::AppFocus` (see Task 004).
  - `Event::SessionEnded` — broadcast when a capture worker
    process exits (any exit status; the daemon reaper emits it).
  - `Event::AppFocus` — broadcast on `OpenApp`; a subscribed app
    window shows + focuses itself.
- All three **appended at the end** of their enums: postcard
  encodes the variant index, so appending keeps every existing
  wire byte stable across version skew (established rule, ipc.rs
  Phase 6 comment).
- Doc comments state who sends/receives each and why (crash-safe
  reaper semantics for `SessionEnded`).

## Where

- `crates/flintshot-core/src/ipc.rs` only.

## Verify

- `cargo test -p flintshot-core`
- Existing serde round-trip style tests (if any cover the enums)
  still pass; no reordering anywhere.
