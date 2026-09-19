# Task 006: COSMIC applet mirrors the swap

**Description:** The applet's *Open config* menu item becomes
*Settings*, sending `Request::OpenSettings` — the same IPC request
the tray sends. The local `open_config` / `editor_command` code is
removed (the GUI path to the config file is the settings page now).

**Acceptance criteria:**
- [ ] `MenuMessage::OpenConfig` → `MenuMessage::Settings`;
  `action_for(Settings)` → `Action::Request(Request::OpenSettings)`
- [ ] `Action::OpenConfig` variant removed; `open_config()` +
  `editor_command()` deleted from `menu.rs` (and their tests)
- [ ] `app.rs` popup renders the *Settings* button (daemon-gated
  like other request buttons; the always-enabled slot dies with
  OpenConfig)
- [ ] `request_spawns_capture` set unchanged (its test's `others`
  list may need `OpenSettings` added to pin `false`)
- [ ] Module docs refreshed (menu inventory + no more `$EDITOR`
  shelling)

**Verification:**
- [ ] `cargo build -p flintshot-cosmic` green
- [ ] `cargo test -p flintshot-cosmic` green (headless menu tests)

**Dependencies:** Task 2

**Files likely touched:**
- `crates/flintshot-cosmic/src/menu.rs`
- `crates/flintshot-cosmic/src/app.rs`

**Docs:**
- None beyond module docs.

**Estimated scope:** Small: 2 files
