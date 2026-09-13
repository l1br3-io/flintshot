# Task 004: Config/CLI backend preference + worker grab path

**Description:** Wire backend selection into the user surface:
`CaptureConfig` gains a `backend` string (default `"auto"`, tolerant
like `interaction-mode`), `region`/`fullscreen` gain `--backend`
(hard-error on unknown ids, mirroring upload's
`validate_auth_backend`), and the worker's grab path in `cli.rs`
resolves CLI > config > auto, then captures via
`select_backend(...).capture_monitor(...)` with `include_cursor` from
config. Daemon-spawned worker argv elides `--backend` when it equals
the config default (mirror `upload_argv`). A `tracing::info!` line
records the chosen backend id on every grab.

**Acceptance criteria:**
- [ ] `[capture] backend = "gnome-shell"` loads, roundtrips, and an
      unknown value loads + re-saves verbatim with no parse failure
- [ ] `--backend` validation: known ids (`auto`, `xcap`, `gnome-shell`,
      `portal`) accepted; unknown → hard CLI error naming valid ids
- [ ] `capture_prepared` grabs through the registry; chosen backend id
      appears in logs; behavior with default config is identical to
      pre-Task path (xcap) on this session kind
- [ ] Worker argv builder unit tests: default elision + explicit pass
- [ ] Config merge preserves user comments on the new key (existing
      `set_value` pattern)

**Verification:**
- [ ] `cargo test --workspace --all-targets --quiet` green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` green
- [ ] Manual: `cargo run -- region --backend xcap` and
      `cargo run -- region` behave identically on X11

**Dependencies:** Task 3 (registry API)

**Files likely touched:**
- `crates/flintshot-core/src/config.rs`
- `crates/flintshot-app/src/cli.rs`
- `crates/flintshot-app/src/worker.rs` (argv builder + tests)

**Docs:**
- None beyond doc comments (config field documented in-code).

**Estimated scope:** Medium (3 files)
