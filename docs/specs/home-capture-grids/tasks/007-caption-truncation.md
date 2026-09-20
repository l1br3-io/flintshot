# Task 007: caption truncation + card polish

**Description:** Filenames longer than their card truncate with a
middle ellipsis via a pure `truncate_middle(s, max_chars) -> String`
(keeps head + tail — `flintshot-2026-…-10-00-00` stays identifiable),
applied where the name galley is laid out, with `max_chars` derived
from the card width so wide cards show more of the name. Metadata
captions (`dims · size · age`) get the same guard. Polish pass across
both card kinds: caption vertical rhythm at the new larger card
heights, hover strokes consistent (recent = `FOREST`, session =
`GOLD_DEEP`, as shipped).

**Acceptance criteria:**
- [ ] `truncate_middle` unit-tested: short (unchanged), exactly at
      max, long (head + `…` + tail, length ≤ max), max < 3 (returns
      the clamped ellipsis, no panic)
- [ ] A 200-char filename renders a name galley whose width stays
      within the card rect (render test re-lays the truncated string)
- [ ] Captions never overflow at the narrowest card width (1-column
      tiny window render test)
- [ ] Metadata captions bounded the same way
- [ ] No behavior change to clicks, chips, grouping, counts

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual: capture with a pathologically long template filename;
      confirm ellipsis, not overflow

**Dependencies:** Task 3, Task 6 (final card geometry)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/home.rs`

**Docs:** None expected.

**Estimated scope:** Small — 1 file
