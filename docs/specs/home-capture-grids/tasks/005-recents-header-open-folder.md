# Task 005: recents header — `N of LIMIT` + Open folder chip

**Description:** The recents section header becomes a row: the
`RECENT CAPTURES` eyebrow, the count term `· N of LIMIT` (`INK_SOFT`),
and — pushed right — an **Open folder** cutout chip. `HomeBridge`
gains two defaulted methods: `recents_limit(&mut self) -> usize`
(default `DEFAULT_RECENTS_LIMIT`) and `open_recents_folder(&mut self)`
(default no-op). The app bridge answers the limit from
`capture_flow::recents_limit()` and opens the folder by mirroring
`reveal_recent`'s opener + reaper-thread pattern over
`OutputConfig::resolve_save_dir()`; failure logs and pushes a
`SetStatus` directive. `RecordingBridge` gains both (recordable).

**Acceptance criteria:**
- [ ] Header renders `RECENT CAPTURES · N of LIMIT` with the bridge's
      numbers (render test: bridge with limit 50 and 3 recents →
      `· 3 of 50` laid out; content asserted via a pure
      `recents_header_caption(count, limit)` helper)
- [ ] Clicking the Open folder chip routes exactly one
      `open_recents_folder` (bridge test, chip-center pattern)
- [ ] Trait defaults keep every existing test double compiling
      unmodified
- [ ] App impl: resolve save dir → spawn opener → reap; error path
      logs + sets status, never panics
- [ ] Empty-recents state is unchanged (friendly empty card, no
      header count row changes needed there)

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui -p flintshot-app`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: click Open folder — the file manager opens the resolved
      save dir

**Dependencies:** None hard (parallel-safe with 003/004; lands with
the recents phase)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/home.rs`
- `crates/flintshot-app/src/app_window.rs`
- `crates/flintshot-app/src/capture_flow.rs` (only if the helper needs
  a visibility tweak)

**Docs:** Gotcha (opener zombie reaping — copy the `reveal_recent`
pattern, do not invent a new one) → inline comment at the impl.

**Estimated scope:** Medium — 2-3 files
