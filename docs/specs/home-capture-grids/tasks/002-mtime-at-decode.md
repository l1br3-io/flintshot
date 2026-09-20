# Task 002: mtime at decode, drop the per-frame stat

**Description:** `RecentThumb` gains `mtime: u64`, gathered in
`thumbnail()` beside the existing one-time `fs::metadata` (bytes). The
recent card's age caption reads the decode-time mtime instead of
calling `age_seconds(path)` per frame; `age_seconds` is deleted. Card
visuals and ids are unchanged — this is a data-plumbing task that
unblocks grouping (Task 4) and removes a per-frame filesystem stat per
card.

**Acceptance criteria:**
- [ ] `RecentThumb` carries `mtime`; `thumbnail()` populates it in the
      same single `fs::metadata` read it already does for bytes
- [ ] `age_seconds` is gone; the recents caption's age term derives
      from `RecentThumb.mtime`
- [ ] No `fs::metadata` / filesystem call on the render path outside
      `thumbnail()`'s cache-miss branch
- [ ] Dead/undecodable paths still render nothing (existing test
      `dead_recent_path_is_silently_absent` stays green)

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: recents captions still show correct ages

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-editor-egui/src/home.rs`

**Docs:** None expected.

**Estimated scope:** Small — 1 file
