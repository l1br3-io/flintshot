# Task 004: Tray menu swap — Show/Hide flintshot replaces Open app window

**Description:** The user-visible change: the tray menu's first item
becomes the static **Show/Hide flintshot** toggle sending
`Request::ToggleWindow`. The *Open app window* item and its
`flintshot.app` id retire — pinned dead by test, never reused. The
helper stays alive on the click (no SNI unregister; nothing needs
the panel popup closed).

**Acceptance criteria:**
- [ ] Menu reads *Show/Hide flintshot* / *Capture* / *Settings* /
      *—* / *Quit* (one static label; no flipping, no checkmark)
- [ ] `action_for` maps `flintshot.toggle-window` →
      `TrayAction::Request(Request::ToggleWindow)`
- [ ] `flintshot.app` resolves to `None` — pinned dead in the
      legacy-id test next to `flintshot.open-config` /
      `flintshot.bind-shortcuts`
- [ ] `exits_after_action(Request::ToggleWindow) == false` in the
      helper's staying list
- [ ] Tray module doc comment updated (it currently narrates the
      *Open app window* item)
- [ ] No other menu item changed: Capture / Settings / Quit ids and
      mappings untouched (existing tests stay green unchanged)

**Verification:**
- [ ] `cargo test -p flintshot-app --all-targets` green
- [ ] `cargo build` green
- [ ] Manual: `cargo run -- daemon`, click the tray item twice —
      window appears, then closes; daemon + tray survive both

**Dependencies:** Tasks 2 + 3 (the click must do the right thing in
both directions before the item exists)

**Files likely touched:**
- `crates/flintshot-app/src/tray.rs`
- `crates/flintshot-app/src/tray_helper.rs`

**Docs:**
- None (module doc refresh is in-file).

**Estimated scope:** Small: 2 files
