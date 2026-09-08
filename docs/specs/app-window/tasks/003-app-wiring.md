# Task 003 — App crate wiring: `flintshot app` subcommand

## What

- `crates/flintshot-app/src/app_window.rs` (new):
  - **Ensure daemon**: attempt the subscription (below). On
    no-daemon: spawn `flintshot daemon` detached (stdin/stdout
    null, stderr inherited — tray-helper precedent), retry the
    subscription until it answers (bounded, e.g. 10 s), status
    line reflects "starting daemon…".
  - **Subscription pump** (background thread): sync
    `interprocess` client (extend `ipc_client`'s connect+frame
    path with a blocking `subscribe` reader). Forwards each
    `Event` into the shared `HomeState`.
  - **`HomeState`** (pure, unit-tested): saturating in-flight
    counter (`CaptureStarted` +, `SessionEnded` −), `hidden`
    flag, status. Emits `HomeDirective::Show` when in-flight hits
    0 while hidden; on reader disconnect (daemon death) →
    `Show` + error status.
  - **Bridge impl** (`HomeBridge` for the egui crate): request
    methods run on the UI thread → hide is the UI's own viewport
    command; the *delegate* happens on the pump thread after the
    250 ms hide grace (`HIDE_GRACE_MS`, comment tying it to the
    tray icon-grace bound), then `CaptureRegion` /
    `CaptureFullscreen` via `ipc_client::delegate_to` +
    in-flight tracking.
  - `MonitorEntry` sourcing: `flintshot_core::monitors()` on
    (re)show, cached for the combo box.
- `cli.rs`: `Command::App` → `app_window::run()`.
- `single.rs`: expose `acquire_app_lock()` (named-lock seam).
  `flintshot app` holds it for its lifetime; a held lock →
  delegate `Request::OpenApp` (focus ping) and exit 0.
- Headless (`--no-default-features`) fallback error mirroring
  the editor's.
- Wrap `run_home` in `crate::ambient::with_ambient_runtime`
  (accesskit/zbus, same as editor/pin paths).

## Where

- `crates/flintshot-app/src/{app_window.rs,cli.rs,single.rs,ipc_client.rs}`
  (+ `main.rs` module list).

## Verify

- Unit: `HomeState` transitions (hide → Started → Ended → show;
  Started-before-subscribe clamp; disconnect → show + status),
  lock-held path, grace constant sanity.
- Manual: exit-criteria rows 1–4 of the spec.
