# Task 001: zbus dep edge + `sni_watchdog` decision logic behind the `SniBus` seam

**Description:** Add the `zbus` dependency edge (workspace + app
manifests, mirroring the `gtk` optional/Linux-target pattern) and the
new `crates/flintshot-app/src/sni_watchdog.rs` module, gated
`#[cfg(all(feature = "tray", target_os = "linux"))]`. The module
defines a sync trait seam — `SniBus` with `watcher_owned() ->
Result<bool>` and `poke(name) -> Result<()>` — plus the pure decision
function `watchdog_tick(&mut bus)` that encodes the whole policy:
owned → do nothing; unowned → poke `org.kde.StatusNotifierWatcher`
then `com.system76.CosmicStatusNotifierWatcher`, stopping at the
first success; any probe error → log, no poke, no panic. The real
zbus-backed `SniBus` impl lands in Task 2 beside its only caller
(`-D warnings` dead_code forces it — an uncalled impl here would
fail the gate), while the decision logic is pinned by fake-bus
tests.

**Acceptance criteria:**

- [x] `sni_watchdog.rs` exists with module docs stating the *why*
      (watcher exited on L5 2026-08-30; passive waiting = invisible
      icon), gated `(feature = "tray", target_os = "linux")` and
      declared in `main.rs` under the same cfg
- [x] Watcher well-known names are named `const`s:
      `org.kde.StatusNotifierWatcher`,
      `com.system76.CosmicStatusNotifierWatcher`
- [x] `watchdog_tick` behavior: owned → zero pokes; unowned → poke
      standard then COSMIC, stop at first success; probe error → no
      poke, no panic; poke errors are logged (debug) and never
      propagate
- [x] Unit tests (fake `SniBus` recording calls): owned → no poke;
      unowned → both names in order; standard-success → COSMIC
      skipped; standard-failure → COSMIC still tried; probe error →
      no poke, no panic
- [x] `zbus = { version = "5", default-features = false, features =
      ["tokio"] }` in `[workspace.dependencies]`; optional +
      Linux-target-scoped in `flintshot-app`, referenced as `dep:zbus`
      from the `tray` feature (same shape as `dep:gtk`)
- [x] `cargo tree -p flintshot-app --features tray` shows exactly one
      zbus 5.x (unified with ashpd's 5.18; the pre-existing zbus 4.4
      under keyring is untouched)
- [x] `cargo check -p flintshot-app --no-default-features` and the
      default (no-tray) build stay clean — module and dep invisible
      there; no `unwrap()`/`expect()` outside tests

**Verification:**

- [x] RED first: write the fake-bus tests against a stubbed
      `watchdog_tick` (no-op) and watch them fail, then implement
- [x] `cargo test -p flintshot-app --features tray --quiet` green
- [x] `cargo check -p flintshot-app --no-default-features --quiet`
      clean; `cargo check --quiet` (default members, no tray) clean
- [x] `cargo tree -p flintshot-app --features tray -i zbus@5` shows
      the new edge resolving to the ashpd-unified 5.18

**Dependencies:** None

**Files likely touched:**

- `Cargo.toml` (workspace deps)
- `crates/flintshot-app/Cargo.toml` (optional, target-scoped zbus;
  `tray` feature list)
- `crates/flintshot-app/src/main.rs` (cfg-gated `mod sni_watchdog;`)
- `crates/flintshot-app/src/sni_watchdog.rs` (new: seam, decision
  logic, zbus impl, tests)

**Docs:**

- Module doc carries the why; full rationale lands as ADR-0032 in
  Task 4.

**Estimated scope:** Medium: 4 files
