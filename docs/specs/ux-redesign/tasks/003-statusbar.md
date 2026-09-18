# Task 003: Statusbar + editor wiring

**Description:** New `statusbar.rs` — the 30px forest-deep footer from
`reference/zone-map.html`: `Image: {w}×{h} px · {size}` (dims × 4
heuristic, humanized B/KiB/MiB) · per-tool hint in gold (from the
engine's `tool_hint`) · real `kbd` chips (Ctrl+Z, Ctrl+Y, Ctrl+S, Del).
Wired as a bottom panel in `EditorApp::ui_impl` (worker + routed).
Honors the animation gate (n/a yet) and leaves a `SizeMode` seam
defaulting to heuristic for Task 11.

**Acceptance criteria:**
- [ ] Full-width bottom panel under rail/panel/canvas (draw order:
      top → bottom → left → right; fresh id).
- [ ] Content per zone contract: cream-bold dims+size, gold centered
      hint (`tool_hint(selected_tool)`), cream kbd chips — tokens +
      `widgets::kbd_chip` only.
- [ ] Humanized-size formatter unit-tested (B/KiB/MiB boundaries).
- [ ] Hint switches with the selected tool.
- [ ] Worker-window resize target accounts for the 30px bar
      (`target_h + chrome-heights` seam in `app.rs`).
- [ ] Height pinned at 30 logical px (span assertion test).

**Verification:**
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: `cargo run -- app` → capture → real dims show; hint
      follows tool switches

**Dependencies:** Tasks 1, 2

**Files likely touched:**
- `crates/flintshot-editor-egui/src/statusbar.rs` (new)
- `crates/flintshot-editor-egui/src/app.rs`
- `crates/flintshot-editor-egui/src/lib.rs`

**Estimated scope:** Small (2-3 files)
