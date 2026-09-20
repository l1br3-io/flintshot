# Task 004: day-grouped recents + tear-line eyebrows

**Description:** Restructure `recents_section` into a two-pass render:
**decode pass** (resolve thumbnails for every path first — dead paths
drop out before layout), **render pass** (group the surviving paths by
`day_bucket(thumb.mtime, now)`, chunk each group into adaptive rows,
render `TODAY` / `YESTERDAY` / `EARLIER` eyebrow labels — `TYPE_XS`
caps, `INK_SOFT` — each sitting on a dotted `PAPER_DIM` rule running
to the section edge: the tear line). Newest-first within groups;
groups appear in calendar order. Empty groups render nothing; a
history entirely from earlier days shows only `EARLIER`.

**Acceptance criteria:**
- [ ] Group labels render above their rows in Today → Yesterday →
      Earlier order (render test with seeded mtimes for all three)
- [ ] Within a group, cards keep the recents order (newest first)
- [ ] A card with today's mtime never lands under `EARLIER` (unit +
      render tested)
- [ ] Dead paths vanish before grouping — no orphan group labels, no
      empty groups rendered
- [ ] Tear line: one dashed-stroke painter call per label, `PAPER_DIM`,
      extending from the label to the section's right edge; zero new
      `Color32` literals (palette-of-record scan stays green)
- [ ] Card response ids stay path-keyed; existing click/chip tests
      pass without id changes
- [ ] `day_bucket` calls read `RecentThumb.mtime` (no new stats)

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: seed captures today / yesterday / a week old; confirm
      three labeled groups, order, tear lines

**Dependencies:** Task 1 (`day_bucket`), Task 2 (`RecentThumb.mtime`),
Task 3 (grid geometry)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/home.rs`

**Docs:** Gotcha (egui holds dropped/state across frames — decode pass
must stay idempotent per frame) → inline comment at the pass boundary.

**Estimated scope:** Medium — 1 file
