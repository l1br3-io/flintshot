# Task 001 — IPC protocol types in `flintshot-core::ipc`

> Phase 2.A · size: S · depends on: none

## Description

Add the IPC schema to `flintshot-core`, transport-agnostic. Define
`Request` and `Event` enums (`#[non_exhaustive]`, serde, kebab-case)
matching ARCHITECTURE.md §IPC protocol, plus a `socket_path()` resolver
(platform default + `[ipc] socket_path` override). No `interprocess` /
transport code here — only the types external clients (the Phase 3
applet, scripts) will depend on. Add `postcard` + `serde` to
`flintshot-core`. Add a new `FlintshotError::Ipc { detail }` variant.

## Acceptance criteria

- [ ] `flintshot-core::ipc` module exists and is re-exported from
      `lib.rs`; module doc states transport lives in `flintshot-app`.
- [ ] `Request` has variants: `CaptureRegion { monitor, delay_ms }`,
      `CaptureFullscreen { monitor }`, `Quit`, `Subscribe`, `OpenLast`,
      `PinLast`, `UploadLast { backend }` — all `#[non_exhaustive]`.
- [ ] `Event` has variants: `CaptureStarted`, `EditorOpened`,
      `Saved { path }`, `Error { code, message }`, `Pinned { id }`,
      `Uploaded { url, delete_handle }` — all `#[non_exhaustive]`.
- [ ] Every public type has a doc comment (`#![warn(missing_docs)]`).
- [ ] `socket_path()` returns `$XDG_RUNTIME_DIR/flintshot.sock` on
      Linux, a platform-appropriate path on macOS, `\\.\pipe\flintshot`
      on Windows; honours an explicit override when passed.
- [ ] `FlintshotError` gains `Ipc { detail: String }` (follows the
      `detail`-field rule of ADR-0013).

## Verification

- [ ] Standing gate passes.
- [ ] `cargo test -p flintshot-core ipc::` — postcard roundtrip: encode
      every `Request` / `Event` variant, decode, assert equality
      (catches serde-tag / `#[non_exhaustive]` regressions).
- [ ] `cargo test -p flintshot-core ipc::socket_path` — resolver
      returns the expected platform default; override wins.
- [ ] `no_gui_types_in_core.rs` still green (Task 012 will extend it,
      but `ipc.rs` must already avoid `use interprocess` / `use tokio`).

## Dependencies

None. (Foundational; everything else in Phase 2 imports from here.)

## Files likely touched

- `crates/flintshot-core/src/ipc.rs` (new)
- `crates/flintshot-core/src/lib.rs` (module + re-exports)
- `crates/flintshot-core/src/error.rs` (`Ipc` variant)
- `crates/flintshot-core/Cargo.toml` (`postcard`, `serde` already present)

## Size

S
