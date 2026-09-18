# Task 005: Topbar with hero actions on editor surfaces

**Description:** New `topbar.rs` replaces `header.rs` on editor
surfaces (worker + routed Editor route): 44px forest-deep cutout bar —
flame badge + `Flintshot | Editor`, breadcrumb `Home ▸ Editor`
(current stage gold+bold), ghost buttons **Capture** (hero-key) /
**Copy** (hero-key) / **Open**, `⋯` overflow (Save `Ctrl+S`, Upload,
Paste `Ctrl+V`, separator, About, status + `flintshot backends` note,
Settings entry — the entry renders inert until Task 10). `header.rs`
retires; tests migrate.

**Acceptance criteria:**
- [ ] Zone contract met (spec §Topbar): dims, fills, hero-key borders
      on Capture/Copy, cream-dim disabled styling.
- [ ] Capture → `NewCaptureMode::Region` exit (worker) / page capture
      (routed); Copy → clipboard path, gated as today; Open → page
      bridge `open_image` (routed), disabled on worker windows.
- [ ] Overflow items all fire their real actions (Save/Upload/Paste/
      About/note); Settings item present but inert.
- [ ] Worker resize target uses the 44px topbar height; pinned test.
- [ ] `header.rs` deleted; router + worker call sites updated; no AI
      slot; headless tests: hero buttons + overflow register, Copy
      disabled state, AI absent.

**Verification:**
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: Copy lands on clipboard; Capture starts a region
      recapture; Open routes; overflow Save/Upload/Paste work

**Dependencies:** Tasks 1-4

**Files likely touched:**
- `crates/flintshot-editor-egui/src/topbar.rs` (new)
- `crates/flintshot-editor-egui/src/header.rs` (deleted)
- `crates/flintshot-editor-egui/src/app.rs`
- `crates/flintshot-editor-egui/src/window.rs`

**Estimated scope:** Medium (4 files)
