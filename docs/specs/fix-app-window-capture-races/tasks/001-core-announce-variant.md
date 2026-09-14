# Task 001: Core — `Request::AnnounceAppWindow`

**Description:** Add the presence-announce variant to the IPC schema
(appended — postcard variant-index rule) with its roundtrip case.

**Done:** `flintshot-core/src/ipc.rs` carries the variant; the
roundtrip test covers it.

## Verification

- [x] `cargo test -p flintshot-core request_postcard_roundtrip`
