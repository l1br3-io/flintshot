# Task 002: Remove `flintshot pin` CLI subcommand

**Description:** Delete the `flintshot pin <path>` subcommand —
`Command::Pin` variant, `PinArgs` (`--at`, `--size`, `--opacity`,
`--no-clipboard`), `run_pin` and its `PinConfig`-taking helper, the
`main.rs` dispatch arm, and related tests. This kills the last
consumer of `EguiShell::with_pin_config` outside editor-egui.

**Acceptance criteria:**
- [ ] `Command::Pin(PinArgs)` variant and `PinArgs` struct gone
- [ ] `run_pin` + the helper that passes resolved `PinConfig` into
      the shell deleted
- [ ] `main.rs` no longer dispatches `Command::Pin`
- [ ] CLI help / subcommand list text mentions no `pin`
- [ ] Pin-specific CLI tests deleted; surrounding tests untouched

**Verification:**
- [ ] `cargo test -p flintshot-app` green
- [ ] `cargo run -- --help 2>&1 | grep -i pin` prints nothing

**Dependencies:** None (but run after 001 so the editor crate is
already pin-free at the UI layer)

**Files likely touched:**
- `crates/flintshot-app/src/cli.rs`
- `crates/flintshot-app/src/main.rs`

**Docs:**
- None expected.

**Estimated scope:** Small-Medium (2 files, cli.rs is large but edits are localized)
