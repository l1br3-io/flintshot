# Task 001: Worker argv `--standalone` guard

**Description:** Daemon-spawned `region` / `fullscreen` workers
currently re-delegate back to the daemon (fork loop). Append
`--standalone` to the argv built by `worker.rs::region_argv` /
`fullscreen_argv` so the worker runs its capture in-process;
`cli.rs::delegate_to_daemon` already short-circuits on that flag.
Argv content is the dispatch contract — the `RecordingSpawner` tests
asserting it *are* the no-re-delegation regression test.

**Acceptance criteria:**

- [x] `region_argv` output ends with `--standalone` (with and without
      `--monitor` / `--delay-ms`)
- [x] `fullscreen_argv` output ends with `--standalone` (with and
      without `--monitor`)
- [x] `pin_argv` / `open_argv` / `upload_argv` unchanged (those
      subcommands never delegate)
- [x] A named test documents the fork-loop intent (both capture
      requests spawn standalone workers)
- [x] Existing exact-match argv tests updated to the new expectation

**Verification:**

- [x] `cargo test -p flintshot-app` green
- [x] Full standing gates green (`fmt`, `clippy --workspace
      --all-targets -D warnings`, workspace tests, doc tests)

**Dependencies:** None

**Files likely touched:**

- `crates/flintshot-app/src/worker.rs` (implementation + `#[cfg(test)]`)

**Docs:**

- Why-comment on `region_argv` / `fullscreen_argv` referencing the
  fork loop (TEST_MATRIX L5 finding 1).

**Estimated scope:** Small: 1 file
