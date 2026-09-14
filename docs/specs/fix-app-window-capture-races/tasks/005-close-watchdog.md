# Task 005: Home window — stuck-close watchdog

**Description:** `HomeApp` latches the Capture click; ≥ 500 ms later
it re-issues `ViewportCommand::Close` every frame (warn once). A
dropped close no longer wedges the armed capture intent.

**Done:** latch + watchdog in `home.rs`, pure bound predicate,
headless arming tests.

## Verification

- [x] `cargo test -p flintshot-editor-egui home`
