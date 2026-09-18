# Task 004: Copy feedback — the hero's Copied state

**Description:** A successful Copy in the editor page currently
produces zero visible change (`self.copied` set at app.rs:1619,
rendered nowhere since the panel `copy_chip` retired). Give the topbar
Copy hero a "Copied" state: while inside a ~2 s window after a
successful copy, the hero paints the check glyph (Task 2) and the
label reads "Copied", with the gold accent; then it reverts. Plumbing:
`EditorApp` records the copy instant alongside `copied` (egui input
time), exposes a pure `copied_visible(now) -> bool`, and requests
repaints for the remaining window so the revert lands without input. A
fresh editor route constructs a fresh `EditorApp`, so the state resets
for free. The router-drawn topbar (routed pages) queries the live
editor; `draw_worker_topbar` passes its own state. Note: the click
chain itself is proven (probe in spec §2) — do NOT add a
clipboard-dependent click test; CI asserts the rendering and
time-window seams, the real paste stays on the manual protocol.

**Acceptance criteria:**
- [ ] Pure seam: `copied_visible` is true only within the 2 s window
      after a copy (unit test at 0 s / 1.9 s / 2.1 s)
- [ ] Headless render: with the copied state armed, the topbar paints
      a "Copied" galley and the check glyph on the Copy hero; unarmed,
      it paints "Copy" and the copy glyph (shapes-based assertions,
      clipboard untouched)
- [ ] Both lifecycles feed the hero: routed page (window.rs queries
      the editor app) and worker window (`draw_worker_topbar`)
- [ ] The revert repaint fires: while visible, a
      `request_repaint_after` ≤ the remaining window is issued
      (assert via repaint-after output headlessly)
- [ ] A fresh editor route starts unarmed (existing fresh-route
      reset semantics; assert via a new EditorApp construction)

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui` (quiet)
- [ ] `cargo fmt --all` && `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual protocol: `cargo run -- app` → editor page → Copy →
      hero shows "Copied" ~2 s; `wl-paste --list-types` lists
      `image/png`; paste into another app

**Dependencies:** Task 3 (glyph hero signature)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/app.rs` (copy instant, `copied_visible`, worker topbar pass-through)
- `crates/flintshot-editor-egui/src/topbar.rs` (`show`/`hero` copied param + rendering)
- `crates/flintshot-editor-egui/src/window.rs` (router queries the live editor's state)
- `crates/flintshot-editor-egui/src/widgets.rs` (only if the copied variant lands in `ghost_button`)

**Docs:**
- GOTCHA comment at the copy instant: the state is time-windowed, not
  latched, so a stale "Copied" can never survive into the next session
  (fresh EditorApp) — the reason the reset needs no explicit clear.

**Estimated scope:** Medium: 3-4 files

**Git Commit:** `git commit -m "feat(editor): Copy hero shows a Copied state — the copy lands visibly now"`
