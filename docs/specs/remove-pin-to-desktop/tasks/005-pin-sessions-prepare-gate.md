# Task 005: Remove pin sessions + `prepare_gate` pins condition

**Description:** Delete pin session tracking from `sessions.rs`
(`SessionKind::Pin`, `PinInfo`, the `pin` field, `register_pin`, and
the pin respawn/kill-for-capture paths) and simplify
`prepare_gate.rs`: the `pins_alive` condition is vacuous with no pin
sessions, so remove the parameter, the gate logic, the tests that pin
it, and update the `daemon.rs` call site. Stale pin mentions in
`prepare_gate.rs` doc comments go too.

**Acceptance criteria:**
- [ ] `SessionKind::Pin`, `PinInfo`, `register_pin`, and the `pin`
      field removed from `sessions.rs`; pin respawn/kill logic and
      tests deleted
- [ ] `prepare_gate.rs`: `pins_alive` parameter dropped from
      `step(...)`, the `!pins_alive` term dropped from the clear
      condition, `pins_alive_hold_the_reply` + pin-window test names
      / comments rewritten or deleted (keep equivalent coverage of
      the surviving conditions)
- [ ] `daemon.rs` `prepare.step(...)` call site updated to the new
      signature; pin-kill-on-prepare plumbing removed
- [ ] `ipc_server.rs` / `ambient.rs` doc comments that describe pin
      windows reworded (ambient: "`pin` → pin window" routing line)

**Verification:**
- [ ] `cargo test -p flintshot-app` green
- [ ] `cargo clippy -p flintshot-app --all-targets -- -D warnings` green
- [ ] `rg -n "SessionKind::Pin|PinInfo|register_pin|pins_alive" crates/flintshot-app/src/` empty

**Dependencies:** Task 004 (removes `register_pin`'s only non-test
caller)

**Files likely touched:**
- `crates/flintshot-app/src/sessions.rs`
- `crates/flintshot-app/src/prepare_gate.rs`
- `crates/flintshot-app/src/daemon.rs`
- `crates/flintshot-app/src/ipc_server.rs` (comments only)
- `crates/flintshot-app/src/ambient.rs` (comments only)

**Docs:**
- If the gate's clear-condition semantics get subtler without the
  pin term, a `/* GOTCHA */` at the condition is the right home.

**Estimated scope:** Medium (3 substantive + 2 comment files)
