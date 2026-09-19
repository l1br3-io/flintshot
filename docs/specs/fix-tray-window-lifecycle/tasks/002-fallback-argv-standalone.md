# Task 002: No-daemon fallback spawns route through `worker::app_argv`

**Description:** The CLI's no-daemon fallback spawns
(`spawn_settings_app` / `spawn_home_app`) hand-build `["app",
"--route", "settings"]` / `["app"]` without the trailing
`--standalone`. Against an app-lock holder such a child errors on the
"stale `flintshot app` process?" path instead of exiting quietly.
Route both through `worker::app_argv` (which appends
`--standalone` and exists precisely for this delegation-loop rule).
Spec §Diagnosis B3 / AC 3.

**Acceptance criteria:**
- [x] RED first: a worker.rs test pinning `app_argv(AppRoute::Settings)` ends with `--standalone` and `app_argv(AppRoute::Home)` == `["app", "--standalone"]` exists and passes trivially once wired — the RED side is a cli.rs-level pin: extract each spawn's argv into a pure helper (e.g. `settings_app_argv()` / `home_app_argv()` returning `Vec<String>`, used by the `Command::args` call) and test it equals `worker::app_argv(...)` output
- [x] `app_argv` and `AppRoute` reachable from cli.rs (`pub(crate)` on `app_argv` in worker.rs; `AppRoute` is already `pub`)
- [x] `spawn_settings_app` / `spawn_home_app` spawn with the `app_argv`-built args (stdio behavior unchanged: stdin null; stdout/stderr inherited as today)
- [x] Existing `is_app_argv` classification tests stay green (the `--standalone`-suffixed argv must still classify as the app worker — `routed_app_argv_classifies_as_app_and_not_capture`)

**Verification:**
- [x] `cargo test -p flintshot-app -q` green
- [x] Full gate (four commands, quiet)
- [x] Manual check deferred to the manual matrix (no-daemon arm)

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-app/src/cli.rs` (spawn helpers, tests)
- `crates/flintshot-app/src/worker.rs` (visibility on `app_argv`, argv pin test)

**Docs:**
- None beyond code comments (the `--standalone` rule's why already
  lives on `app_argv`).

**Estimated scope:** Small: 2 files
