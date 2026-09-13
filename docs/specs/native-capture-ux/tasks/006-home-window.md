# Task 006: Home window — Capture / Open / hidden Paste

**Description:** Rework the home window's control set. The two
capture buttons (+ keyboard capture shortcut + any per-monitor
affordance) become one prominent **Capture** control →
`request_capture()` → `Request::Capture` → window closes per the
ADR-0047 contract (unchanged). **Open** joins as a first-class
control: native file dialog (the `rfd` pattern already in the tree —
verify where dialogs live; if the open dialog only exists in the
editor's save path, lift it) → opens the picked image in the editor
(the `flintshot open` path via IPC or in-process). **Paste** is
built but hidden behind a compile-time `const SHOW_PASTE: bool =
false;` with a TODO referencing the future clipboard→editor feature.
Window title/status semantics unchanged.

**Acceptance criteria:**
- [ ] One Capture control (button + keyboard accelerator) → capture
      flow, window closes/reopens per ADR-0047 (manual)
- [ ] Open control → file dialog → image opens in the editor
      (manual; `.png`/`.jpg`)
- [ ] Paste button code exists, hidden by the const; flipping the
      const shows it (unit-test the const is false on this branch)
- [ ] Bridge trait: `request_capture_region`/`fullscreen` collapse
      to `request_capture`; `CaptureKind` enum removed or reduced
- [ ] Home tests updated: button set, directive handling

**Verification:**
- [ ] `cargo test -p flintshot-app --bins --quiet` green
- [ ] `cargo test -p flintshot-editor-egui --quiet` green
- [ ] Manual (COSMIC): full loop — Capture → OS UI → editor → close
      → home reopens; Open → editor

**Dependencies:** Task 5 (single `Request::Capture`)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/home.rs`
- `crates/flintshot-app/src/app_window.rs`

**Docs:**
- GOTCHA: dialog-reuse note if the open path had no rfd precedent.

**Estimated scope:** Medium (2 files)
