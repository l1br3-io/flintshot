# Task 005: `flintshot upload` CLI body

**Description:** Replace the `run_upload` stub (`cli.rs:593`) with a
real implementation: load the image file, sniff the format via
`image::guess_format` → `ImageMime` (no `--mime` flag; Decisions §4),
resolve the backend (`--backend` override → `[upload]
default-backend` → built-in `"imgur"`), read the keyring entry for the
backend (if present), construct the `UploadContext`, run the backend's
`upload()` future on a tokio runtime, then:
- Print the URL to **stdout** as the last line (machine-parseable)
- Copy the URL to clipboard via `arboard` (text content)
- Fire a desktop notification via `notify-rust` ("Uploaded to <url>" /
  "Upload failed: <message>")
- Exit 0 on success, non-zero on error (error message to stderr)

Adds `notify-rust` and `arboard` to `flintshot-app` deps (arboard is
already a workspace dep; notify-rust is new).

**Acceptance criteria:**
- [ ] `run_upload` loads the image, sniffs format, resolves backend,
      constructs `UploadContext`, runs upload on a tokio runtime
- [ ] URL printed to stdout (last line); diagnostics to stderr
- [ ] URL copied to clipboard on success
- [ ] Desktop notification on success ("Uploaded to <url>") and error
      ("Upload failed: <message>")
- [ ] Exit 0 on success, non-zero on error (no-such-file, network
      failure, backend rejection)
- [ ] `--backend NAME` overrides `[upload] default-backend`
- [ ] `notify-rust` added to workspace deps + `flintshot-app`
- [ ] `arboard` added to `flintshot-app` (already a workspace dep)

**Verification:**
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo fmt --all --check`
- [ ] Manual check: `cargo run -- upload --help` shows the subcommand;
      `cargo run -- upload /nonexistent` exits non-zero with clear error

**Dependencies:** Task 001 (trait types), Task 002 (config default),
Task 004 (ImgurBackend + registry)

**Files likely touched:**
- `Cargo.toml` (workspace deps: notify-rust, arboard)
- `crates/flintshot-app/Cargo.toml`
- `crates/flintshot-app/src/cli.rs`

**Estimated scope:** Medium: 3 files
