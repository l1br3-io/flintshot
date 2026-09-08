# Task 002 — Home window module (`flintshot-editor-egui`)

## What

- New `crates/flintshot-editor-egui/src/home.rs` (+ module wiring):
  - `HomeBridge` trait — the app crate's handle into the window:
    - `fn request_capture_region(&mut self)`
    - `fn request_capture_fullscreen(&mut self, monitor: Option<u32>)`
    - `fn poll_directive(&mut self) -> Option<HomeDirective>`
    - `fn monitors(&mut self) -> Vec<MonitorEntry>` (index + label;
      the bridge can source `flintshot_core::monitors()` or a
      cached list)
  - `HomeDirective` enum: `Show` (restore + focus), `SetStatus`
    (daemon/connection state string). Consumed by the UI each
    frame via `poll_directive`.
  - `HomeApp` (`impl eframe::App`): buttons *Capture region* (R) /
    *Capture fullscreen* (F), monitor `ComboBox` feeding
    fullscreen, status line. On click: send
    `ViewportCommand::Visible(false)` + call the bridge's request
    method. Monitors list refreshes when the window is (re)shown.
  - `EguiShell::run_home(&self, bridge)` concrete method
    (mirrors `run_overlay`'s precedent — no core trait change).
    Window: small fixed logical size, `centered`, titled
    `flintshot` (WM class distinct from editor/pin windows).
- Theme: reuse `crate::theme` tokens; keyboard shortcuts R/F
  via `egui::KeyboardShortcut` + `ctx.input` checks.
- No IPC, no sockets, no threads in this crate — the bridge is
  the only seam.

## Where

- `crates/flintshot-editor-egui/src/{lib.rs,home.rs,theme.rs}`.

## Verify

- `cargo test -p flintshot-editor-egui` (directive-consumption
  unit tests where headless-possible)
- `cargo test --doc -p flintshot-editor-egui`
- Manual smoke via Task 003's subcommand.
