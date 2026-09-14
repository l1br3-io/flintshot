# Task 002: Daemon — presence in the prepare gate

**Description:** `PrepareGate` grows an atomic presence counter
(`app_window_connected/disconnected/present`); the IPC layer
intercepts `AnnounceAppWindow` with an RAII guard and parks the
connection (its lifetime is the signal); the daemon's main loop ORs
presence into the machine's `app_alive` input.

**Done:** interception + guard in `ipc_server.rs`; step-input
composition in `daemon.rs`; unit tests (counter, machine-hold) and a
duplex-stream integration test (announce registers, disconnect
withdraws).

## Verification

- [x] `cargo test -p flintshot-app prepare_gate`
- [x] `cargo test -p flintshot-app announce`
