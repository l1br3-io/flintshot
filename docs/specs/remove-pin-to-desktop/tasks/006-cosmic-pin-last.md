# Task 006: Remove Pin last from COSMIC applet

**Description:** Strip the pin surface from the opt-in COSMIC panel
applet: the *Pin last* menu message + button, its `Request::PinLast`
IPC mapping, and the `Event::Pinned` → "Pinned #N" status mapping,
with their tests. This is the last IPC *sender/consumer* of the pin
variants, unblocking task 007.

**Acceptance criteria:**
- [ ] `MenuMessage::PinLast` variant, its `Request::PinLast` mapping,
      and the "Pin last" button removed (`menu.rs`, `app.rs`)
- [ ] `Event::Pinned` status mapping + doc table row removed from
      `ipc.rs`
- [ ] Pin tests deleted or updated (menu action test, roundtrip
      entries, `status_for_event` assertions)
- [ ] Doc comments in `app.rs` / `menu.rs` no longer list Pin

**Verification:**
- [ ] `cargo build -p flintshot-cosmic` succeeds (needs system deps,
      see AGENTS.md)
- [ ] `cargo test -p flintshot-cosmic` green (if runnable headless)
- [ ] `rg -n "PinLast|Pinned" crates/flintshot-cosmic/src/` empty

**Dependencies:** None hard; must precede 007

**Files likely touched:**
- `crates/flintshot-cosmic/src/menu.rs`
- `crates/flintshot-cosmic/src/app.rs`
- `crates/flintshot-cosmic/src/ipc.rs`

**Docs:**
- None expected.

**Estimated scope:** Small (3 files, small diffs)
