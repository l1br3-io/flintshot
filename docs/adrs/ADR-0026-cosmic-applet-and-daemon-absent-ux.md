# ADR-0026: COSMIC applet as a `.desktop`-discovered panel applet + daemon-absent *Start daemon* UX

**Status.** Accepted.

**Context.** Phase 3b fills the Phase 0 `flintshot-cosmic` stub with a
real COSMIC panel applet — a `libcosmic` panel button + popup that talks
IPC to a running `flintshot daemon`. The applet mirrors the Phase 2 tray's
actions (Capture region, Capture fullscreen, per-monitor submenu, Open
config, Quit), exposes the Phase 3 *Pin last*, and subscribes to the
daemon's `Event` stream to render a live status line. When no daemon
answers, the popup disables capture actions and offers a *Start daemon*
button that spawns `flintshot daemon` detached and re-probes.

**Decision.**

- **Entry: `cosmic::applet::run::<App>(())`.** Confirmed against the
  locked libcosmic revision (`ef162b8`,
  `examples/applet/src/{main,window}.rs`). `App` implements
  `cosmic::Application` with `APP_ID = "io.github.anomalyco.Flintshot"`.
  The `Cargo.lock` pin (not a source-pin in `Cargo.toml`) is the
  reproducibility anchor — `Cargo.toml` keeps `branch = "master"` per
  ADR-0005.
- **`APP_ID` ↔ `.desktop` filename contract.** `cosmic-panel` correlates
  a running applet with its config slot via the app id. The `.desktop`
  filename (Task 006) must be `io.github.anomalyco.Flintshot.desktop`
  (the id without `.desktop`) so the picker discovers it under
  `$XDG_DATA_DIRS/cosmic/applets/`.
- **Popup via `cosmic::surface::action::app_popup`.** The panel button's
  `on_press_with_rectangle` closure emits
  `Message::Surface(app_popup::<App>(live_settings, settings, view))`;
  a second press or `PopupClosed` emits `destroy_popup(id)`. The popup
  body is built inside the `app_popup` closure's third argument (the
  `BoxedView`), not in `view_window` — confirmed against the example,
  whose `view_window` returns a placeholder (`// GOTCHA:` in
  `app.rs::view_window`).
- **Click dispatch on a background `Task`.** Each click that maps to a
  `Request` dispatches via `flintshot_ipc::send_request(None, &req).await`
  on a background `iced::Task::future(...).discard()` — the UI thread
  never blocks on the socket. The daemon's reaction arrives via the
  subscriber stream (Task 004), not a response (fire-and-forget, matching
  the tray).
- **Daemon-absent UX: `ConnectionState` + *Start daemon* via `PATH`.**
  The `NoDaemon` vs `Connected` state machine (Task 002) gates the popup's
  action buttons (Task 005). *Start daemon* builds `["flintshot",
  "daemon"]` via `Command::new("flintshot")` — a `PATH` lookup, matching
  the tray's `$EDITOR` assumption for *Open config*. After spawn, a
  one-shot re-probe subscription transitions back to `Connected`.

**Consequences.**

- The applet is a legitimate `libcosmic` consumer (the `applet` feature
  must be enabled in `Cargo.toml`). This pulls `cosmic-panel-config` and
  `xdg-shell-wrapper-config` into the dep graph — additional compile time
  but no runtime cost for non-COSMIC users (the crate is opt-in).
- The popup content is built in the `app_popup` closure, not
  `view_window`. Any future maintainer expecting `view_window` to be the
  popup entry point will find the `// GOTCHA:` comment in `app.rs`.
- `APP_ID` is a hardcoded contract. Renaming the app (or the GitHub org)
  requires updating both the Rust constant and the `.desktop` filename in
  the same commit — a mismatch means the panel picker can't find the
  applet.
- *Start daemon* assumes `flintshot` is on `PATH`. Dev runs (`cargo run`)
  don't install the binary; the README documents this (Task 006). A spawn
  failure surfaces as a tracing warning, not a panic.
- The tray and the applet coexist when the daemon runs on COSMIC — both
  are harmless IPC clients. Tray suppression (detect a COSMIC session and
  skip tray init) is deferred to a future polish effort (spec Q5). Today
  the user may see both surfaces; neither interferes with the other.

**Cross-references.** ADR-0005 (`libcosmic` as a separate applet binary +
the master-tracking choice); ADR-0010 (`interprocess` transport the applet
reuses via `flintshot-ipc`); ADR-0018 (the side-thread/main-loop bridge the
daemon uses; the applet instead uses iced's `Subscription` for its
background stream); ADR-0025 (the shared `flintshot-ipc` transport this
applet consumes); ADR-0023 / ADR-0024 (the pin + `Saved` reporting the
applet's *Pin last* and status line surface). Phase 3b Task 003 (applet
shell), Task 004 (subscriber stream), Task 005 (daemon-absent UX), Task
006 (`.desktop` + this record).
