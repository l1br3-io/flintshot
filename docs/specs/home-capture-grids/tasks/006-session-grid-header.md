# Task 006: session grid + `THIS SESSION · N` + gap fix

**Description:** The session section adopts the shared adaptive grid
(Task 3's geometry): cards stretch to their column, thumbnails grow.
Its label becomes a header row — `THIS SESSION · N` (count `INK_SOFT`,
label keeps its `GOLD_DEEP` accent) — with no actions. The section
still hides entirely when the store is empty, and the **orphan gap**
goes away: today `welcome_body` emits a `SPACE_SCALE[3]` after the
session section unconditionally plus `recents_section`'s own spacing,
so an empty store leaves a double gap; spacing moves into the sections
themselves so empty-session renders recents tight under the
secondaries. Session cards' discard/resume behavior is untouched.

**Acceptance criteria:**
- [ ] Session cards use the adaptive grid: more columns at 1600 px
      than at 800 px (render test, mirroring Task 3's)
- [ ] Header shows the live count `THIS SESSION · N` (render test
      with 2 cards; pure `session_header_caption(count)` helper)
- [ ] Empty store: no section, and the rect gap between the
      secondary cards row and the recents label equals the
      single inter-section spacing (render-test the absence of the
      double gap)
- [ ] Resume click and two-step Discard chip tests pass unchanged
- [ ] Pending-PNG repaint hint still fires while arrival thumbnails
      decode

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: open with unsaved session captures, verify count and
      grid; discard all, verify no orphan gap

**Dependencies:** Task 3 (geometry), Task 2 (card visuals settle)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/home.rs`

**Docs:** None expected.

**Estimated scope:** Medium — 1 file
