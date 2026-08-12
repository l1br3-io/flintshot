# Task 004: ImgurBackend impl + workspace deps

**Description:** Implement the first concrete `UploadBackend`:
`ImgurBackend` in `crates/flintshot-app/src/upload/imgur.rs`. Supports
anonymous (no auth header) and Client-ID authenticated (`Authorization:
Client-ID <id>` from keyring) modes. POSTs multipart form to
`https://api.imgur.com/3/image`, parses the JSON response into
`UploadOutcome { url, delete_handle }`, and maps HTTP / network / auth
failures to the four `UploadError` variants.

Also creates `upload/mod.rs` with a simple backend registry (id →
`Arc<dyn UploadBackend>`) and adds the workspace deps: `reqwest`
(default features + multipart + json), `keyring` to `flintshot-app`,
`mockito` as a dev-dep for HTTP mocking in tests.

Response mapping is tested via `mockito` (dev-dep) returning canned
responses: success JSON, 429 rate-limit, 401 auth-error, transport
failure. No real network in tests.

**Acceptance criteria:**
- [ ] `ImgurBackend` struct with a `reqwest::Client` field
- [ ] `impl UploadBackend for ImgurBackend`: `id()` → `"imgur"`,
      `name()` → `"Imgur"`
- [ ] `upload()` POSTs multipart to `https://api.imgur.com/3/image`;
      adds `Authorization: Client-ID <id>` when `ctx.credentials` is
      `Some` and the keyring read succeeds; falls back to anonymous
      (logs at `info`) when credentials are absent
- [ ] Success response (200 with `data.link` + `data.deletehash`)
      parsed into `UploadOutcome`
- [ ] 429 → `UploadError::Backend`; 401/403 → `UploadError::Auth`;
      network failure → `UploadError::Transport`
- [ ] `upload/mod.rs` registry: `resolve_backend("imgur")` returns
      `Arc<ImgurBackend>`; unknown id → `UploadError::UnknownBackend`
- [ ] `reqwest` added to `[workspace.dependencies]` + `flintshot-app`
- [ ] `mockito` added as dev-dep to `flintshot-app`
- [ ] All tests pass (including mock-backed response-mapping tests)

**Verification:**
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo fmt --all --check`
- [ ] Manual check: `cargo build` (compiles with new deps)

**Dependencies:** Task 001 (reworked trait types)

**Files likely touched:**
- `Cargo.toml` (workspace deps: reqwest, keyring)
- `crates/flintshot-app/Cargo.toml`
- `crates/flintshot-app/src/upload/mod.rs` (NEW)
- `crates/flintshot-app/src/upload/imgur.rs` (NEW)
- `crates/flintshot-app/src/main.rs` or `lib.rs` (module declaration, if
  `main.rs` declares modules)

**Estimated scope:** Medium: 4-5 files
