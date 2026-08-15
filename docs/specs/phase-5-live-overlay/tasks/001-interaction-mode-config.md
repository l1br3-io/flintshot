# Task 001: `interaction_mode` config + `--overlay`/`--editor` flags + mode-resolution helper

**Description:** Lay the Phase 5 plumbing before any overlay UI exists.
Add `[ui] interaction-mode` to `flintshot-core::config` (string,
`"editor"` default — same string-storage policy as
`magnifier-modifier`, so an unknown value never bricks config loading
and is preserved verbatim across re-saves). Add mutually exclusive
`--overlay` / `--editor` flags to `region` / `fullscreen` (flattened
`InteractionArgs`). Add the mode-resolution helper in `cli.rs` —
CLI flag > config value (platform fallback joins in Task 007) — and
wire it into `capture_and_dispatch` so the decision (mode + source)
is logged once per capture. An explicit flag skips daemon delegation:
the IPC request cannot carry it and the daemon's worker re-resolves
from config, so delegating would silently drop the user's flag.
Until Task 002 lands the overlay shell, a resolved `overlay` logs a
notice and opens the editor (safe fallback, no behavior change).

**Acceptance criteria:**
- [x] `UiConfig::interaction_mode: String` in
      `crates/flintshot-core/src/config.rs`, `#[serde(default =
      "default_interaction_mode")]`, default `"editor"`, kebab-case
      TOML key `interaction-mode` (via the existing struct-level
      rename), included in `Default` and `merge_into_existing`.
- [x] Config round-trip: absent key → `"editor"`; `"overlay"`
      persists and reloads; unknown value loads fine and is preserved
      (string storage, forward-compat).
- [x] `InteractionArgs` (`--overlay`, `--editor`) flattened into
      `RegionArgs` + `FullscreenArgs`; passing both is a clap error.
- [x] `resolve_interaction_mode(cli_flag, config_value)` returns the
      mode plus its source (`Flag` / `Config`); unknown config values
      fall back to `Editor` with a `tracing::warn!`.
- [x] `capture_and_dispatch` resolves once (flag > config) and logs
      mode + source; resolved `overlay` currently falls back to the
      editor with a notice (Task 002 replaces this branch).
- [x] Explicit `--overlay`/`--editor` skips `delegate_to_daemon`
      (runs standalone, like `--standalone`).
- [x] Daemon-spawned workers (hotkey/tray path) unchanged: they run
      bare `region`/`fullscreen` argv and pick the mode from their own
      config (per spec: IPC unchanged).

**Verification:**
- [x] `cargo fmt --all --check`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo test --workspace --all-targets` — incl. new config
      round-trip tests + resolution/flag/delegation unit tests.
- [x] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [x] Manual: `cargo run -- region --help` shows both flags;
      `cargo run -- region --overlay --editor` errors clearly;
      `cargo run -- config` prints `interaction-mode = "editor"`.

**Dependencies:** None (foundation task).

**Files likely touched:**
- `crates/flintshot-core/src/config.rs` — field + default fn + merge + tests.
- `crates/flintshot-app/src/cli.rs` — `InteractionArgs`, `InteractionMode`
  + source enum + `resolve_interaction_mode`, delegation skip,
  `capture_and_dispatch` wiring, tests.
- `CHANGELOG.md` — Unreleased entry.

**Estimated scope:** Small-medium (2 source files, no behavior change
for existing flows).
