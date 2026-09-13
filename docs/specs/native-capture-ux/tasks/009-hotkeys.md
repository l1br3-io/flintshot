# Task 009: Hotkeys — single `capture` binding + legacy warn

**Description:** Collapse hotkeys to one action. Config:
`[hotkeys] capture = "…"` replaces `region`/`fullscreen` (default =
today's region default, e.g. `Ctrl+Shift+PrintScreen`). Legacy
`region`/`fullscreen` keys in existing configs must **warn and not
bind** — serde ignores unknown fields silently, so inspect the raw
`[hotkeys]` table via the `toml_edit` document before deserializing
(the load path already uses toml_edit; interaction-mode/`backend`
string-tolerance is the policy precedent). Bindings: X11
`global-hotkey` path (`hotkeys.rs`) binds one key; Wayland portal
path (`hotkeys_portal.rs`, ashpd `GlobalShortcuts`) registers one
shortcut and maps it to `Request::Capture`; the daemon's Wayland
advice text becomes "bind a compositor keybinding to `flintshot
capture`". Hot-reload (`hotkeys_changed`) tracks the single binding.

**Acceptance criteria:**
- [ ] `HotkeyConfig` has one `capture` field; round-trip + default
      tests
- [ ] Config containing legacy `region`/`fullscreen` keys loads with
      a `tracing::warn!` naming the rename and binds nothing from
      them (pinned by test using a written config file)
- [ ] X11: one binding registered, fires `Request::Capture`
- [ ] Portal: one shortcut; advice text names `flintshot capture`
      (all five current advice strings updated)
- [ ] Hot-reload swap of the `capture` binding works (existing
      watcher test pattern)

**Verification:**
- [ ] `cargo test --workspace --all-targets --quiet` green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` green
- [ ] Manual (COSMIC): portal shortcut fires; old config warns in
      the log; new config binds

**Dependencies:** Task 5 (single request)

**Files likely touched:**
- `crates/flintshot-core/src/config.rs` (`HotkeyConfig` + legacy warn)
- `crates/flintshot-app/src/hotkeys.rs`
- `crates/flintshot-app/src/hotkeys_portal.rs`
- `crates/flintshot-app/src/daemon.rs` (shared state field)

**Docs:**
- GOTCHA: the serde-ignores-unknown-fields trap → comment at the
  toml_edit table inspection.

**Estimated scope:** Medium (4 files)
