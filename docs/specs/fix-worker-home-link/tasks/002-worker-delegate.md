# Task 002: Worker-side `open_home_from_worker` delegate + shell wiring

**Description:** Implement the real delegate in the app crate —
`open_home_from_worker` (try-delegate `Request::OpenApp`; `NoDaemon`
→ detached `flintshot app` spawn) plus the `spawn_home_app` fallback
helper — and wire `.with_open_home(open_home_from_worker)` onto both
shell constructions in `run_editor_session` (selection-first and
plain), completing the user-visible path: worker Home click → app
window focused/spawned on Home → worker editor closes.

**Acceptance criteria:**
- [ ] `open_home_from_worker` mirrors `open_settings_from_worker`: `Delegated` → no-op; `NoDaemon` → `spawn_home_app()` with failures logged as warnings (never a panic, never a dead click); `Err` → warning
- [ ] `spawn_home_app` mirrors `spawn_settings_app` (detached, stdin null, no `--standalone` — the settings precedent), spawning plain `app` (Home route elides `--route`)
- [ ] Both `EguiShell` constructions in `run_editor_session` carry `.with_open_home(open_home_from_worker)`
- [ ] No daemon-side changes; no changes to the app window's router / `RouteHost`

**Verification:**
- [ ] `cargo build` succeeds
- [ ] `cargo test --workspace --all-targets` green (the delegate itself is IPC-bound like its Settings twin — unit-test precedent is none; behavior covered by the manual matrix)
- [ ] Manual check (needs a graphical session): daemon + tray up, no `flintshot app` → tray Capture → Home link → app window opens on Home, editor closes; with an app window already running → it focuses; `pkill flintshot` first → standalone capture → Home link → `flintshot app` spawns. If no session is available, note it on the checkpoint and rely on Task 001's unit pins.

**Dependencies:** Task 001 (the hook seam this wires into)

**Files likely touched:**
- `crates/flintshot-app/src/cli.rs` (delegate + spawn helper next to the Settings twins; wiring in `run_editor_session`)

**Docs:**
- None beyond code comments (the twins' comments explain the
  choreography; mirror their shape).

**Estimated scope:** Small: 1 file
