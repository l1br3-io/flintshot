# Task 003: Worker-table dispatch for `OpenSettings`

**Description:** Teach the daemon's dispatch table the new request:
broadcast `Event::SettingsFocus`, then spawn the app worker with the
route flag (`app --route settings`). Byte-for-byte the `OpenApp`
arm's shape with the event + argv changed.

**Acceptance criteria:**
- [ ] `dispatch(Request::OpenSettings)` emits exactly
  `[Event::SettingsFocus]` and spawns one worker with argv
  `["app", "--route", "settings"]` (RecordingSpawner pins both)
- [ ] Outcome is `DispatchOutcome::Continue`
- [ ] `app_argv(route)` grows the route parameter; the `OpenApp` arm
  keeps the plain `["app"]` argv (existing test stays green
  unchanged)
- [ ] `variant_name(OpenSettings)` → `"open-settings"`
- [ ] `is_app_argv(["app", "--route", "settings"])` is `true` (the
  spawned worker still registers as an app in the session registry —
  extend the argv-classification test)
- [ ] `spawns_capture_worker(OpenSettings) == false` (extend the
  request-side test: the icon must not vanish)

**Verification:**
- [ ] `cargo test -p flintshot-app --all-targets` green
- [ ] `cargo build` green

**Dependencies:** Task 2 (the IPC variants exist)

**Files likely touched:**
- `crates/flintshot-app/src/worker.rs`

**Docs:**
- None (mirrors an existing arm; comments per house style).

**Estimated scope:** Small: 1 file
