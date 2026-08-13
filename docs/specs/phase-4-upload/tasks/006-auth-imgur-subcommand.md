# Task 006: `flintshot auth imgur` subcommand

**Description:** Add a new `auth` CLI subcommand for setting / clearing
the Imgur Client-ID in the OS keychain. `flintshot auth imgur` prompts
for the Client-ID (hidden input via `rpassword` or stdin), writes it to
`keyring::Entry::new("flintshot", "imgur:client-id")`, exits 0 with a
confirmation. `flintshot auth imgur --clear` removes the entry (reverts
to anonymous). This is the **only** credential-entry path — no config
file field (secrets-stay-out-of-config boundary).

`IMGUR_CLIENT_ID` env var is read as a non-interactive fallback (one-shot
read by `flintshot auth imgur` when stdin is not a TTY).

**Acceptance criteria:**
- [x] `Command::Auth(AuthArgs)` variant added to the CLI enum
- [x] `AuthArgs` has a `backend: String` positional (only `"imgur"`
      valid for Phase 4) and `--clear` flag
- [x] `flintshot auth imgur` writes the Client-ID to keyring entry
      `"flintshot" / "imgur:client-id"`
- [x] `flintshot auth imgur --clear` removes the entry; clearing an
      absent entry is a no-op (exit 0, "no credential stored")
- [x] Non-interactive fallback: `IMGUR_CLIENT_ID` env var is read when
      stdin is not a TTY
- [x] Exit 0 on success, non-zero on keyring failure
- [x] No credential ever written to config, logs, or stdout

**Verification:**
- [x] `cargo test --workspace --all-targets`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo fmt --all --check`
- [x] Manual check: `cargo run -- auth --help` shows the subcommand

**Dependencies:** Task 001 (keyring in workspace deps)

**Files likely touched:**
- `crates/flintshot-app/Cargo.toml` (rpassword dev/dep)
- `crates/flintshot-app/src/cli.rs`

**Estimated scope:** Small-Medium: 2 files
