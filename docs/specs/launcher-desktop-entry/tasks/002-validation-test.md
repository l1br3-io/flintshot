# Task 002: Validation test — parse grammar + Exec ↔ CLI sync

**Description:** The guardrail that keeps the assets honest over
time: an integration test in `flintshot-app` that (a) parses each
`.desktop` file per the Desktop Entry spec's line grammar (groups,
key=value, Actions ↔ `[Desktop Action *]` consistency, required keys
present), and (b) asserts every `Exec=`'s subcommand token exists in
the actual binary — via `env!("CARGO_BIN_EXE_flintshot") --help`
output, so drift in `cli.rs` fails this test. Creates the crate's
first `tests/` directory.

**Acceptance criteria:**
- [ ] `crates/flintshot-app/tests/desktop_files.rs`: locates assets
      via `CARGO_MANIFEST_DIR/../../assets`, parses both files.
- [ ] Grammar checks: `[Desktop Entry]` group first; required keys
      (`Type`, `Name`, `Exec`, `Icon`) non-empty in every group that
      requires them; every id in `Actions=` has a matching
      `[Desktop Action <id>]` section and vice-versa (main file
      only; autostart variant must have *no* Actions).
- [ ] Sync check: for each `Exec=` line, the first argument after
      the binary name (`daemon`, `region`, `fullscreen`) appears as
      a subcommand in the binary's `--help` text (run via
      `env!("CARGO_BIN_EXE_flintshot")`, `Command::output`).
- [ ] Demonstrated failure mode: temporarily editing an `Exec=` to a
      bogus subcommand makes the test fail (verify locally, then
      revert).
- [ ] No `unwrap()` on user-facing paths; errors via `panic!`/assert
      messages that name the file + key (test-context asserts are
      fine per conventions).

**Verification:**
- [ ] `cargo test -p flintshot-app` green
- [ ] Mutation check above observed passing→failing→reverted

**Dependencies:** Task 001

**Files likely touched:**
- `crates/flintshot-app/tests/desktop_files.rs` (new)

**Docs:** GOTCHA comment where the test hard-codes nothing but
relies on `--help` output format: clap's help layout is stable for
subcommand listing; if help rendering changes, the parse regex/
contains-check is the thing to update (named in place).

**Estimated scope:** Small (1 file)
