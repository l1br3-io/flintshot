# Task 005: `HomeBridge` copy/reveal/delete + limited `recent_images`

**Description:** The bridge seam for the card actions. Three new
`HomeBridge` methods with no-op defaults (test doubles keep compiling):
`copy_recent(path)` (decode file → arboard image write, fresh handle per
the Wayland rule, handle parked in `Shared` for the window's lifetime —
X11 owner rule), `reveal_recent(path)` (spawn `xdg-open`/`open`/
`explorer` on the parent dir per `std::env::consts::OS`, failure →
status line with the raw path), `delete_recent(path)` (remove file,
drop from the recents index via `Recents::remove`, best-effort). The
app-crate `Bridge` implements them; `recent_images` truncates to the
clamped config limit.

**Acceptance criteria:**
- [ ] Default no-op methods exist; `RecordingBridge` and `IdleBridge` test doubles compile without changes
- [ ] `reveal_recent` picks the opener by OS (pure `opener_command` seam unit-tested per-cfg where practical, else by inspection) and spawns detached without blocking the UI thread
- [ ] `delete_recent` removes the file and the index entry, keeps the rest ordered (tempdir test through `Recents::remove` + fs probe)
- [ ] `copy_recent` keeps the clipboard handle alive beyond the call (X11 rule) — shape pinned by a unit test on the storage seam
- [ ] `recent_images` returns at most `recent-captures-limit` entries

**Verification:**
- [ ] `cargo test --workspace --all-targets` (RED first)
- [ ] fmt + clippy gates

**Dependencies:** Task 002 (`Recents::remove`, limit plumbing)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/home.rs` (trait)
- `crates/flintshot-app/src/app_window.rs` (impl, `Shared` clipboard slot)

**Docs:**
- None

**Estimated scope:** Medium
