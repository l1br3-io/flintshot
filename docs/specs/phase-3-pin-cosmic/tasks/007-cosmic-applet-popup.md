# Task 7: COSMIC applet shell + popup menu

**Description:** Replace the `flintshot-cosmic` stub `main()`
(`main.rs:13`) with a real COSMIC panel applet: a panel button that
opens a popup with the action set mirroring the tray — *Capture region*,
*Capture fullscreen*, *Fullscreen on monitor →* (per-monitor submenu),
*Pin last*, *Open last*, *Open config*, *Quit*. Each action sends the
matching `Request` via the Task 6 IPC client. This is the highest-risk
task (libcosmic `master`); if it breaks, pin-to-desktop (Tasks 1–5) is
already shippable. The live event-stream display is Task 8 — this task
is fire-and-forget actions + the applet lifecycle.

**Acceptance criteria:**
- [ ] `crates/flintshot-cosmic/src/main.rs` rewritten to a
      `cosmic::applet` entry (the `cosmic::app::Daemon` / `cosmic::
      Application` template — match pop-os's own applet structure).
      Builds as a panel applet, not a stub.
- [ ] Panel button (applet icon) opens a popup window.
- [ ] Popup menu items (each sends a `Request` via Task 6's
      `send_request`):
      - *Capture region* → `Request::CaptureRegion { monitor: None, delay_ms: 0 }`
      - *Capture fullscreen* → `Request::CaptureFullscreen { monitor: None }`
      - *Fullscreen on monitor →* submenu, one entry per
        `flintshot_core::monitors()` result
        (`MonitorInfo` — same labels as `tray.rs:117`); click →
        `Request::CaptureFullscreen { monitor: Some(N) }`
      - *Pin last* → `Request::PinLast`
      - *Open last* → `Request::OpenLast`
      - *Open config* → shells out to `$EDITOR` on
        `flintshot_core::config_path()` (mirror `daemon.rs:237`'s
        `open_config_file`; ~10 LOC in the applet — no daemon coupling)
      - *Quit* → `Request::Quit`
- [ ] The applet is a pure IPC client: `Cargo.toml` depends on
      `flintshot-core` + `libcosmic` + (Task 6) `interprocess`/`postcard`/
      `tokio` only. **No** `flintshot-app`, **no** `egui`.
- [ ] `flintshot-cosmic` stays out of `default-members` (libcosmic
      system deps); the `cosmic-build` CI job stays green on Linux.
- [ ] If `libcosmic` `master` is broken at task time, pin to the last
      known-good commit and flag it (ask-first boundary); record the pin
      in `Cargo.toml` with a comment.
- [ ] `cosmic-build` CI job (`.github/workflows/ci.yml`) still passes
      (install the libcosmic system deps: cmake, libxkbcommon-dev,
      libfontconfig-dev, etc. — already documented in AGENTS.md).

**Verification:**
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy -p flintshot-cosmic -- -D warnings` (or
      `--workspace` if the gate covers it).
- [ ] `cargo build -p flintshot-cosmic` green on Linux.
- [ ] `cargo test -p flintshot-cosmic` — Task 6's tests still pass; any
      new action-mapping tests (menu item → `Request`).
- [ ] Manual (Linux + COSMIC or a libcosmic-compatible panel): run
      `flintshot daemon` + `flintshot-cosmic`; click the panel button;
      popup appears; *Capture region* → daemon spawns worker → editor
      opens; *Fullscreen on monitor → 1* → captures monitor 1; *Quit*
      → daemon stops. *Open config* → `$EDITOR` opens config.toml.

**Dependencies:** Task 6 (the IPC client). Soft-depends on Task 4 for
*Pin last*/*Open last* to actually do something (the menu can send the
`Request` regardless; the daemon's response depends on Task 4).

**Files likely touched:**
- `crates/flintshot-cosmic/src/main.rs` — REWRITE: applet entry.
- `crates/flintshot-cosmic/src/app.rs` — NEW: `cosmic::Application` impl, popup, menu, action dispatch.
- `crates/flintshot-cosmic/Cargo.toml` — libcosmic features as needed (already has `tokio`/`wayland`/`multi-window`); confirm.
- `.github/workflows/ci.yml` — verify the `cosmic-build` job deps (likely unchanged; double-check if new system libs appear).

**Docs:**
- ADR-0005 (libcosmic applet) already covers the "separate binary,
  IPC client" decision — reference it, don't rewrite.
- `CHANGELOG.md` `[Unreleased]` → Added: real COSMIC panel applet
  (replaces stub).
- If a libcosmic `master` pin is needed, a `Cargo.toml` comment
  recording the commit + date.

**Estimated scope:** Large (2–4 files; libcosmic applet lifecycle + popup is the substantive, highest-risk work). If the applet template is fiddly, split the popup-menu wiring from the per-monitor submenu.
