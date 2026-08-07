# ADR-0025: Extract `flintshot-ipc` as the shared IPC transport

**Status.** Draft (full text lands in Task 006).

**Context.** Phase 2 placed the IPC transport — framing (`read_frame` /
`write_frame` / `encode_msg`), socket helpers (`socket_name`, `Stream`),
and the fire-and-forget client helpers (`send_request` /
`send_request_blocking` / `is_no_daemon`) — in `flintshot-app`, the
daemon's bin crate. Phase 3b adds the COSMIC panel applet
(`flintshot-cosmic`, also a bin crate) that must talk the *same* wire
format to the daemon: it sends `Request`s (Capture / Pin / Quit) and
subscribes to the `Event` stream.

A bin crate cannot depend on another bin crate. So the transport code
in `flintshot-app` is unreachable from `flintshot-cosmic` without
duplication, and duplicating the framing is the exact drift risk the
daemon/applet split is supposed to eliminate.

**Considered alternatives.**

- **Duplicate the framing in `flintshot-cosmic`.** Two sources of truth
  for the wire format (u32-LE length prefix, 1 MiB cap, postcard). A
  change to one (e.g. raising the cap, swapping the encoding) silently
  breaks interop. This is the failure mode the crate exists to prevent.
- **Lib + bin split of `flintshot-app`.** Make `flintshot-app` a lib
  crate with a thin bin wrapper, then have `flintshot-cosmic` depend on
  the lib. Pulls the entire daemon (`SubscriberBus`, `IpcServer`,
  `handle_connection`, the worker dispatch, the tray, the hotkeys) into
  a lib that the applet would link against — a massive coupling
  surface for one transport module. Inverts the dependency direction
  (the applet would transitively depend on the daemon's internals).
- **Move the transport into `flintshot-core`.** The core boundary test
  (`no_gui_types_in_core.rs`) explicitly bans `interprocess` from core;
  the schema (`Request`/`Event`) is transport-agnostic by design
  (ADR-0011). Putting the socket plumbing there would collapse the
  schema/transport boundary.
- **A new `flintshot-ipc` workspace lib crate.** Owns only the transport
  primitives + framing tests; depends on `flintshot-core` (for the
  schema) + the existing workspace deps (`interprocess`, `postcard`,
  `tokio`, `color-eyre`). Both bins depend on it. Minimal surface,
  single source of truth, byte-identical wire format.

**Decision.** Extract `flintshot-ipc` as the 5th workspace member (NOT
in `default-members`, matching `flintshot-cosmic`'s pattern — the
default gate and `cargo build` stay unaware of it). Move the framing
primitives, socket helpers, and client helpers out of `flintshot-app`
into the new crate. The framing tests move *unchanged* — they are the
proof of byte-identity with the Phase 2 wire format. Daemon-side state
(`SubscriberBus`, `IpcServer`, `handle_connection`, `DaemonState`,
`decode_request`, `wait_for_socket_to_free`) stays in `flintshot-app`;
only the transport moves.

A new boundary test (`no_daemon_types_in_ipc.rs`) asserts `flintshot-ipc`
exposes the transport but pulls no daemon-side types (no
`tokio::sync::broadcast`, no `SubscriberBus`, no `IpcServer`), keeping
the crate minimal and the daemon/applet split clean.

**Consequences.**

- Both bins (`flintshot-app`, `flintshot-cosmic`) depend on the same
  `flintshot-ipc` lib; the wire format has one implementation. Changing
  the cap / encoding / framing in one place updates both.
- `flintshot-ipc` is NOT in `default-members`: `cargo build` and the
  default CI gate don't compile it unless `--workspace` is passed. The
  `cosmic-build` CI job and the `--workspace` test/clippy gates cover
  it. This mirrors `flintshot-cosmic`'s opt-in shape.
- `flintshot-app::ipc_server` and `flintshot-app::ipc_client` re-import
  the transport from `flintshot_ipc`; their public surfaces (to the
  rest of the app) are unchanged. The Phase 2 daemon `#[ignore]`'d
  real-socket integration tests pass unchanged after the re-point —
  behavioural identity is proven.
- The crate re-uses existing workspace deps only; no new external
  crates, no version bumps.
- The extraction is atomic (single commit): moving half the transport
  leaves two sources of truth — the very drift the crate exists to
  prevent.

**Cross-references.** ADR-0010 (`interprocess` transport); ADR-0011
(postcard encoding); the `flintshot-core::ipc` schema (transport-
agnostic); Phase 3b Task 001 (this extraction); Task 006 (full text
finalization).
