# Task 001: `+` popup mirrors home

**Description:** Replace the `+` chip popup's Region / Fullscreen
entries with the home window's three actions — Capture / Open… /
Paste — with home-parity labels and the Replace-semantics hint on
Open and Paste. Capture exits through the existing `NewCapture`
path (mode label irrelevant; keep `NewCaptureMode::Region` as the
internal default); Open and Paste route the page bridge's existing
`open_image` / `request_paste` contracts, exactly like the chrome
strip buttons today. Core types (`NewCaptureMode`,
`EditorOutcome::NewCapture`) stay untouched — the worker recapture
loop still uses them. Refresh the stale "Interim (task 004)"
comment in `arm_editor_exit` (`app_window.rs`) while there.

**Acceptance criteria:**
- [ ] The `+` popup shows exactly Capture, Open…, Paste (no
      Region / Fullscreen entries); Open and Paste carry the
      Replace-semantics hover hint
- [ ] Each entry routes the same action its home twin / chrome-strip
      twin does: Capture → `NewCapture` exit; Open → file dialog +
      bridge `open_image`; Paste → bridge `request_paste`
- [ ] Ctrl+N still triggers Capture; C / O page accelerators and
      Paste-remains-button-only (P is Pencil) are unchanged
- [ ] Unit tests: popup click-mapping per entry (the existing
      two-pass click-test pattern in `toolbar.rs` tests), plus the
      popup shows no capture-kind variants

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui` green
- [ ] Full gate: `cargo fmt --all --check` &&
      `cargo clippy --workspace --all-targets -- -D warnings` &&
      `cargo test --workspace --all-targets` &&
      `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual: `cargo run -- app` → editor (via Capture or Open) →
      `+` popup shows the three actions and each behaves like home

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-editor-egui/src/toolbar.rs`
- `crates/flintshot-editor-egui/src/app.rs` (popup action wiring)
- `crates/flintshot-app/src/app_window.rs` (comment refresh only)

**Docs:**
- None expected; a popup-behavior note goes into the CHANGELOG at
  close-out (Task 010).

**Estimated scope:** Small: 2–3 files
