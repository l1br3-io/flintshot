# Task 008: COSMIC applet — single Capture action

**Description:** Mirror the tray collapse in the COSMIC panel
applet: the *Capture region* / *Capture fullscreen* buttons and the
per-monitor entries (`CaptureFullscreenMonitor`) become a single
**Capture** action sending `Request::Capture`. Applet status/other
controls unchanged. This crate is opt-in (`flintshot-cosmic`,
system deps per AGENTS.md) — build and clippy-verify against it
explicitly; its tests run only when the crate builds.

**Acceptance criteria:**
- [ ] Applet shows one Capture action; per-monitor entries and the
      old `MenuMessage` variants removed
- [ ] Capture action → `Request::Capture` → daemon flow (manual)
- [ ] `cargo build -p flintshot-cosmic` + clippy green
- [ ] Applet tests updated (button set, message handling)

**Verification:**
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` green
      (includes the cosmic crate when its deps are present)
- [ ] `cargo build -p flintshot-cosmic` green
- [ ] Manual (COSMIC session with the applet): click → OS UI →
      editor

**Dependencies:** Task 5 (single request)

**Files likely touched:**
- `crates/flintshot-cosmic/src/app.rs`

**Docs:**
- None beyond the ADR in Task 10.

**Estimated scope:** Small (1 file)
