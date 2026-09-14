# Task 003: App window — presence connection

**Description:** the pump, once subscribed, spawns the presence
connection (`ipc_client::announce_app_window`) for its whole
connected life; failures warn and degrade to grace-only.

**Done:** `ipc_client.rs` gains the helper; `app_window.rs` spawns
it after `ensure_subscribed`.

## Verification

- [x] `cargo check --workspace --all-targets`
- [ ] Live: daemon log shows "app window presence connection
      established" when `flintshot app` connects (TEST_MATRIX row)
