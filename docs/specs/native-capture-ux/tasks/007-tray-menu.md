# Task 007: Tray — single Capture item

**Description:** Collapse the tray menu's capture set: *Capture
region*, *Capture fullscreen*, and the per-monitor submenu
(`flintshot.fullscreen-monitor.<N>` IDs) become one **Capture** item
sending `Request::Capture`. Everything else (open last, open app,
config, quit, status line) unchanged. The tray-helper protocol
(ADR-0042) carries the new action; the helper's menu-construction
and the daemon-side handler update together. Menu grace behavior
(ADR-0037) and popup mirroring stay as-is — only the capture entries
change.

**Acceptance criteria:**
- [ ] Menu shows exactly one Capture entry; per-monitor submenu and
      the old IDs are gone (structure-pinned test where menu build
      is testable)
- [ ] Clicking Capture starts the capture flow (daemon dispatch,
      popup closes — manual)
- [ ] Old menu IDs are not constructed anywhere (grep-pinned test
      or code removal obvious)
- [ ] Open-last / config / quit / status unchanged (manual)

**Verification:**
- [ ] `cargo test -p flintshot-app --bins --quiet` green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` green
- [ ] Manual (COSMIC): tray click → OS selection UI → editor; no
      regression in popup behavior

**Dependencies:** Task 5 (single request)

**Files likely touched:**
- `crates/flintshot-app/src/tray.rs`
- `crates/flintshot-app/src/tray_helper.rs` (if menu is built there)

**Docs:**
- None beyond the ADR in Task 10.

**Estimated scope:** Small-Medium (2 files)
