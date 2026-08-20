# Task 002: Portal session — silent attach, activation dispatch, Wayland selection, fallback

**Description:** The core vertical slice and the risk-reduction
spike in one. Add `ashpd` (0.13, `default-features = false`,
features `["tokio", "global_shortcuts"]`) to the workspace and
`flintshot-app`. New module `crates/flintshot-app/src/hotkeys_portal.rs`
owns the portal lifecycle, spawned as a tokio task from
`run_tokio_side`: create a `GlobalShortcuts` session, call
`list_shortcuts` (silent attach to whatever the portal already has
bound under our ids `"region"` / `"fullscreen"`), and consume
`receive_activated()` — each `Activated` maps `shortcut_id` →
`Request` and goes through `worker::dispatch` with the shared
`DispatchState` + `SubscriberBus`, exactly like IPC requests on that
thread. Daemon startup changes: when `session_is_wayland()` is true,
skip `GlobalHotKeyManager` entirely (portal-first; XWayland X11
grabs can register-but-never-fire) and start the portal task
instead; the current compositor-keybind warn is replaced by
classified fallback notices. This task also extends
`no_gui_types_in_core.rs`'s deny-list with `use ashpd`.

**Acceptance criteria:**
- [ ] `ashpd` pinned in workspace `Cargo.toml` with
      `default-features = false, features = ["tokio",
      "global_shortcuts"]`; builds with no new system deps
      (zbus is pure Rust — CI ubuntu-latest must stay green
      unchanged).
- [ ] `no_gui_types_in_core.rs` fails if `flintshot-core` sources
      contain `use ashpd`.
- [ ] On a Wayland session the daemon no longer attempts
      `GlobalHotKeyManager::new()`; `HotkeyRegistry` code path is
      unchanged for non-Wayland (X11 tests pass byte-identical).
- [ ] Portal startup classifies failures distinctly — no session
      bus, portal missing, interface/version unsupported — each with
      its own `tracing::info!`/`warn!` naming the
      compositor-keybind workaround; all are non-fatal (daemon
      serves tray + IPC as today).
- [ ] Silent attach: `list_shortcuts` result for our ids is logged
      (bound set, or "nothing bound; run `flintshot hotkeys
      bind`"); **no dialog** appears at daemon start.
- [ ] `Activated` dispatches the mapped `Request` through
      `worker::dispatch` on the tokio side; `Deactivated` is
      ignored; unknown `shortcut_id` logs at debug and drops.
- [ ] id → Request mapping is a pure, unit-tested fn (no portal
      types in its signature) so the mapping logic is testable
      headless.
- [ ] Daemon shutdown drops the portal session cleanly (no hang on
      Ctrl-C; session closes when the connection drops).

**Verification:**
- [ ] `cargo test --quiet --workspace --all-targets` + doc tests
- [ ] `cargo clippy --quiet --workspace --all-targets -- -D warnings`
- [ ] Headless (CI-equivalent): `cargo run -- daemon` starts, logs a
      classified fallback notice, exits cleanly on Ctrl-C (manual,
      no bus / `DBUS_SESSION_BUS_ADDRESS` unset)
- [ ] Manual on Wayland compositor (COSMIC or KDE): pre-bind two
      shortcuts via a throwaway ashpd snippet or the compositor's
      settings → start daemon → pressing them spawns captures
      (verifies the app-id risk early)
- [ ] Manual on X11: daemon registers `global-hotkey` hotkeys as
      before (no behavior change)

**Dependencies:** Task 001 (not strictly code-coupled, but keeps the
wire contract landed first; can start in parallel after reviewing
this plan if desired)

**Files likely touched:**
- `Cargo.toml` (workspace dep)
- `crates/flintshot-app/Cargo.toml`
- `crates/flintshot-app/src/hotkeys_portal.rs` (new)
- `crates/flintshot-app/src/daemon.rs` (Wayland branch + portal task
  spawn)
- `crates/flintshot-core/tests/no_gui_types_in_core.rs`

**Docs:**
- If app-id resolution on a windowless daemon needs a workaround
  (e.g. desktop-file basename must match the binary), record it as
  a GOTCHA comment in `hotkeys_portal.rs` and flag it for ADR-0029
  (Task 005).

**Estimated scope:** Medium (4–5 files)
