# Task 006: Rich cards — metadata caption, hover chips, scroll

**Description:** The recents grid redesign on the pop-up-book tokens.
Card captions become two lines: filename (stem) above, metadata below —
`1920×1080 · 240 KB · 5 mins ago` — dims from the thumbnail decode,
bytes from one `fs::metadata` stat, age now with a count ("5 mins
ago"). Hovering a card reveals a row of cutout chips over the thumbnail
(the `floating.rs` `cutout_chip` pattern): Copy (bridge `copy_recent`),
Reveal (`reveal_recent`), Delete (`delete_recent`) with a two-step
confirm on the chip itself (first click morphs it to "Sure?", second
clicks commits; anything else resets). Click anywhere else on the card
still opens in the editor. The section scrolls (`egui::ScrollArea`)
once cards overflow; still 4 per row.

**Acceptance criteria:**
- [ ] Caption formatter is pure and tested: `("shot", (1920,1080), 245_760, 300) →` filename line + `1920×1080 · 240 KB · 5 mins ago`; KB/MB thresholds; age labels now/mins/hours/yesterday/days with counts
- [ ] Chips register (id-ed responses) only while the card is hovered; clicking Copy/Reveal routes to the bridge (recorder test)
- [ ] Delete: first click swaps the chip label to "Sure?" (state pinned by test), second click calls `delete_recent` and refreshes the grid; clicking elsewhere resets
- [ ] A plain card click (not on a chip) still routes `open_image`
- [ ] With more recents than fit, the section scrolls (content height > viewport in a small-screen frame) while the hero/topbar stay put
- [ ] Existing home tests (thumb render, dead-path skip, empty card) stay green

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui` (RED first)
- [ ] fmt + clippy gates; workspace tests

**Dependencies:** Task 005 (bridge methods)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/home.rs`

**Docs:**
- None

**Estimated scope:** Medium
