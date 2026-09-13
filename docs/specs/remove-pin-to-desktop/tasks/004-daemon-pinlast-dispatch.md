# Task 004: Remove daemon `PinLast` dispatch + `PinIdSource`

**Description:** Delete the daemon-side `Request::PinLast` handling
in `worker.rs` — the dispatch arm, `pin_argv` builder, `PinIdSource`
counter + `pin_ids` state field, `Event::Pinned` emission, the
`register_pin` call at the spawn site, the `"pin-last"` label
mapping, and the dispatch tests covering it. Also drop the
`Request::PinLast` entry from the `other_clicks_keep_the_helper_alive`
test in `tray_helper.rs`.

**Acceptance criteria:**
- [ ] `Request::PinLast` match arm, `pin_argv` builder, and
      `PinIdSource` / `pin_ids` gone from `worker.rs`
- [ ] `Event::Pinned` no longer emitted anywhere
- [ ] `sessions.register_pin` no longer called from `worker.rs`
- [ ] `"pin-last"` request-label mapping removed; label test updated
- [ ] PinLast dispatch tests deleted (spawn, no-prior-save,
      fail-spawn, monotonic ids, supersede-exemption)
- [ ] `tray_helper.rs` test array no longer contains
      `Request::PinLast`; stale comment reworded
- [ ] Worker state struct + `dispatch` signature compile without pin
      members

**Verification:**
- [ ] `cargo test -p flintshot-app` green
- [ ] `cargo clippy -p flintshot-app --all-targets -- -D warnings` green
- [ ] `rg -n "PinLast|PinIdSource|pin_argv|Event::Pinned" crates/flintshot-app/src/worker.rs crates/flintshot-app/src/tray_helper.rs` empty

**Dependencies:** None hard; run after 002 (CLI no longer spawns pin
workers the daemon would track)

**Files likely touched:**
- `crates/flintshot-app/src/worker.rs`
- `crates/flintshot-app/src/tray_helper.rs`

**Docs:**
- None expected.

**Estimated scope:** Medium (2 files; worker.rs carries ~8 test fns)
