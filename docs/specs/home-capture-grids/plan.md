# Plan: Home capture grids — adaptive layout, day-grouped recents, section headers

## Overview

Improve the home page's Recent captures and This session sections per
[`spec.md`](spec.md): an adaptive card grid (columns from window width,
cards fill their columns), recents grouped Today / Yesterday / Earlier
under tear-line eyebrows, headers that carry state (`12 of 50` + Open
folder, `THIS SESSION · 3`), caption truncation, and one stat per
decode instead of per frame. All work stays inside the pop-up-book
token system; no new dependencies.

Work lands on branch `home-capture-grids`.

## Architecture Decisions

- **Day buckets reuse the UTC civil-day convention** `chrono_date()`
  already bakes into filename templates (`civil_from_days`, promoted
  to `pub(crate)` in core). No timezone dependency; the far-from-UTC
  midnight skew is an accepted, documented simplification.
- **mtime lives on `RecentThumb`, gathered once at decode.** Grouping
  and age captions read it from there; the per-frame `age_seconds`
  stat is deleted. This forces a **decode-first, render-second** pass
  in the recents section (dead paths resolve before grouping, so group
  placement only sees renderable cards).
- **One shared geometry** (`columns_for_width` + card sizing) serves
  both grids; session cards adopt it in their own task.
- **Bridge grows by two defaulted methods** (`recents_limit`,
  `open_recents_folder`) — test doubles keep compiling; the app bridge
  answers from `capture_flow::recents_limit()` and mirrors the
  `reveal_recent` opener+reaper pattern over the resolved save dir.
- **Signature element (spec open question, defaulted in):** day-group
  eyebrow labels sit on a dotted `PAPER_DIM` tear line running to the
  section edge. Vetoable at the Phase 2 checkpoint — removal is one
  painter call.

## Task Index

### Phase 1: Foundation
- [ ] Task 1: `day_bucket` in core → `tasks/001-day-bucket-core.md`
- [ ] Task 2: mtime at decode, drop the per-frame stat → `tasks/002-mtime-at-decode.md`

### Checkpoint: Foundation
- [ ] `cargo test --workspace --all-targets` green; no visible UI change

### Phase 2: Recents section
- [ ] Task 3: adaptive recents grid → `tasks/003-adaptive-recents-grid.md`
- [ ] Task 4: day-grouped recents + tear-line eyebrows → `tasks/004-day-grouped-recents.md`
- [ ] Task 5: recents header — `N of LIMIT` + Open folder chip → `tasks/005-recents-header-open-folder.md`

### Checkpoint: Recents complete
- [ ] Render tests prove reflow at 800 vs 1600 px, groups land correctly
- [ ] Manual: resize window, seed mtimes today/yesterday/older, Open folder opens the save dir
- [ ] Tear-line signature shown to human — veto window

### Phase 3: Session section + captions
- [ ] Task 6: session grid + `THIS SESSION · N` + gap fix → `tasks/006-session-grid-header.md`
- [ ] Task 7: caption truncation + card polish → `tasks/007-caption-truncation.md`
- [ ] Task 8: finish — gates, CHANGELOG, manual pass → `tasks/008-finish-gates-changelog.md`

### Checkpoint: Complete
- [ ] All spec success criteria met; full gate commands pass
- [ ] Ready for review / PR

## Risks and Mitigations

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| Adaptive grid breaks existing render tests (fixed 112×148 rects, 4-per-row overflow asserts) | Med | Task 3 updates those tests explicitly; geometry asserted via `read_response` rects, not constants |
| Decode-first restructure changes card response ids / ordering | Med | Ids stay path-keyed; existing card-click and chip tests must stay green unchanged |
| egui `layout_no_wrap` overflow asserts are awkward headless | Low | Truncation is a pure fn (unit-tested); render test re-lays the truncated string and bounds it by the card rect |
| UTC day skew mislabels shots near midnight for non-UTC users | Low | Accepted spec simplification, documented at `day_bucket`; revisit only on user report |
| Tear line reads as decoration | Low | Single painter call, `PAPER_DIM`, vetoable at checkpoint |

## Open Questions

- Tear-line signature: included by default, veto at Phase 2 checkpoint.
- Middle truncation (`flintshot-…-10-00-00`): default; revisit at Task 7.
