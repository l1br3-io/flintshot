# Task 001 — Core IPC variants

> Status: complete
> Effort: [`hide-tray-for-capture`](../spec.md) · Plan decision 1

## Description

Append the handshake's three schema members to `flintshot-core::ipc`
(postcard encodes the variant index, so tail-appending keeps every
earlier variant's wire bytes stable — same note as Phase 6):

- `Request::HideTrayForCapture` — worker → daemon, pre-grab,
  request/reply (the daemon acks only after hide + propagation
  grace).
- `Request::ReportCaptured` — worker → daemon, post-grab,
  fire-and-forget best-effort (pairs with `ReportSaved`'s pattern).
- `Event::TrayHidden` — the one-frame reply, written straight back
  on the worker's connection, never broadcast (mirrors
  `ShortcutsBound`'s reply-only shape).

## Acceptance criteria

1. Both requests + the event round-trip through postcard (tests
   extend `request_postcard_roundtrip` / `event_postcard_roundtrip`).
2. Variants are appended at the enum tails, with the wire-stability
   rationale referenced in the doc comments.
3. `cargo test -p flintshot-core` green; clippy `-D warnings` clean.

## Verification

```sh
cargo test -p flintshot-core
cargo clippy -p flintshot-core --all-targets -- -D warnings
```
