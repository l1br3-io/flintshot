# Task 001: IPC append — `Request::ToggleWindow` + `Event::AppToggle`

**Description:** Add the wire vocabulary the whole effort rides on:
`Request::ToggleWindow` (client → daemon) and `Event::AppToggle`
(daemon → window). Both **appended** to their enums — postcard
encodes the variant index, so inserting would break wire compat
(every prior effort's rule). Doc comments state the pairing and the
window-side resolution contract. No behavior yet; nothing dispatches
or consumes them.

**Acceptance criteria:**
- [ ] `Request::ToggleWindow` is the last variant of `Request`,
      after `OpenSettings`, with a doc comment pairing it with
      `Event::AppToggle` and naming both resolutions (absent window
      → open; present window → forwarded toggle)
- [ ] `Event::AppToggle` is the last variant of `Event`, after
      `SettingsFocus`, with a doc comment: broadcast only when the
      presence oracle says a window is live; the window resolves it
      (visible → graceful exit, capture-hidden → reopen)
- [ ] No existing variant moved (append-only; the diff shows
      additions at the enum tails only)
- [ ] Round-trip: postcard encode→decode of both new variants
      round-trips to equality; the existing variants' wire bytes are
      unchanged (extend the ipc round-trip tests, the
      settings-improvement precedent)
- [ ] `flintshot-core` public API stays GUI-free
      (`tests/no_gui_types_in_core.rs` green)

**Verification:**
- [ ] `cargo test -p flintshot-core --all-targets` green
- [ ] `cargo build` green

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-core/src/ipc.rs`

**Docs:**
- None (doc comments carry the pairing contract).

**Estimated scope:** Small: 1 file
