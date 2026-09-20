---
status: active
started: 2026-09-20
---

# Spec: Home capture grids — adaptive layout, day-grouped recents, section headers

## Objective

Improve the home page's two capture sections — **Recent captures** and
**This session** — along four axes. **User**: Linux screenshot users who
scan their capture history to find "the one from this morning" and hand
it off; the grid is the page they actually read.

1. **Adaptive grid.** Cards are fixed 112×148 at a fixed 4-per-row
   regardless of window width — dead space on wide windows, cramped
   thumbnails everywhere. Column count derives from available width
   (`min` card width 150 px, same item gap), clamped 1–8; cards stretch
   to fill their column, thumbnails grow with the card. Both sections
   share the geometry.
2. **Day-grouped recents.** The grid groups into **Today / Yesterday /
   Earlier** under eyebrow labels. Capture history is time-scanned;
   the grouping encodes the scan, newest-first within each group. Day
   buckets reuse the UTC civil-day convention `chrono_date()` already
   bakes (no new dependency; a far-from-UTC skew near midnight is the
   same known simplification the filename template accepts).
3. **Section headers that carry state.** `RECENT CAPTURES · 12 of 50`
   with an **Open folder** chip (opens the resolved save dir); `THIS
   SESSION · 3` (count only). Empty recents keeps the friendly empty
   card; an empty session store keeps hiding the section — but the
   stray double gap it leaves today goes away.
4. **Caption + perf discipline.** Long filenames truncate (middle
   ellipsis, pure helper) instead of overflowing the card. mtime moves
   into the decode-time `RecentThumb` — today `age_seconds` stats every
   card every frame; after this effort, one stat per decode, never
   per frame.

**Out of scope (explicit):** the hero, secondary cards, settings page,
keyboard navigation of cards, search/filter, multi-select, SessionStore
schema changes, `recents.toml` schema changes.

## Technologies

- Rust 1.95 / edition 2021, workspace crates only — no new dependencies.
- UI: `flintshot-editor-egui` (`home.rs`), pop-up-book tokens from
  `theme/palette.rs` (palette of record — no new hexes).
- Contracts: `HomeBridge` trait gains two methods (defaults keep test
  doubles compiling).
- Core: `flintshot_core::output::civil_from_days` reused for day math
  (promoted to a small public helper if needed).

## Commands

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
```

Manual check: `cargo run -- app` — resize the window across ~700 →
~1800 px logical width and confirm columns reflow, groups land under
the right labels (seed history with files whose mtimes are today /
yesterday / older), Open folder opens the save dir.

## Project Structure

```
crates/flintshot-editor-egui/src/home.rs      # sections, cards, grid math, grouping render
crates/flintshot-core/src/recents.rs          # + pub fn day_bucket (pure, tested)
crates/flintshot-app/src/app_window.rs        # bridge impls: recents_limit, open_recents_folder
crates/flintshot-app/src/capture_flow.rs      # recents_limit() re-exported/used by the bridge
```

## Code Style

Pure, headless-testable helpers; rendering reads their results. Example
shape for the new grouping:

```rust
/// The recents day bucket for `mtime` (unix secs) against `now`.
/// UTC civil days — the same convention `chrono_date()` bakes into
/// filename templates.
pub fn day_bucket(mtime: u64, now: u64) -> DayBucket;

/// Column count for an adaptive grid: floor-fill at `min_card`,
/// clamped 1–8. Pure; unit-tested at 700/1024/1600 px.
fn columns_for_width(available: f32, min_card: f32, gap: f32) -> usize;

/// Middle ellipsis for long stems: keeps head + tail. Pure.
fn truncate_middle(s: &str, max_chars: usize) -> String;
```

Conventions per AGENTS.md: `#![forbid(unsafe_code)]`, no `unwrap()`
outside tests, every `Color32` from `theme/palette.rs` tokens only,
doc comments on public core items, `#[non_exhaustive]` on new public
enums where extension is plausible (`DayBucket`).

## Testing Strategy

- **Unit (pure fns, in-file `#[cfg(test)]`)**: `day_bucket` across
  midnight boundaries (same day, 1s-past-midnight, yesterday, week
  old, mtime > now); `columns_for_width` at 700/1024/1600 px + clamp
  ends; `truncate_middle` short/exact/long; header count strings.
- **Render tests (existing `welcome_frame` harness)**: adaptive grid
  lays out more columns at 1600 px than at 800 px; group labels render
  above their rows in order; header count reflects the bridge's limit;
  session section absent when empty *and* recents sit tight under the
  secondaries (no stray gap); long-filename card's name galley stays
  inside the card rect; dead paths still render nothing.
- **Bridge tests (RecordingBridge)**: `recents_limit` default; Open
  folder chip routes `open_recents_folder`; existing chip/discard
  two-step tests keep passing.
- Full gates above must pass; CI parity (`just ci` shape).

## Boundaries

- **Always**: run the four gate commands before done; keep visual
  language in pop-up-book tokens (`CUTOUT_RADIUS`, `SHADOW_*`, palette
  consts); stat files only at decode; keep ADR-0047 close/reopen flow
  untouched; update `CHANGELOG.md` under `## [Unreleased]`.
- **Ask first**: any new dependency (none planned); bridge trait
  changes beyond `recents_limit` + `open_recents_folder`; touching
  `SessionStore` or `recents.toml` schemas; promoting local-day math
  to real timezone handling.
- **Never**: per-frame filesystem stats; `unsafe`; new hex literals
  outside `palette.rs`; regressions to the existing chip two-step
  confirms or the resume/open click paths.

## Success Criteria

1. Window at 1600 px shows more columns than at 800 px; cards fill
   their columns (no fixed 112 px width, no fixed 4-per-row).
2. Recents render under Today / Yesterday / Earlier eyebrows in
   newest-first order within groups; a file with today's mtime never
   lands under Earlier (unit + render tested).
3. Recents header reads `RECENT CAPTURES · N of LIMIT`; the Open
   folder chip routes `open_recents_folder` to the bridge.
4. Session header reads `THIS SESSION · N`; empty store hides the
   section with no orphan gap above recents.
5. Filenames longer than the card truncate with an ellipsis; the
   caption galley never exceeds the card width.
6. `age_seconds`' per-frame stat is gone; mtime lives on
   `RecentThumb`, gathered once at decode.
7. All four gate commands pass; existing welcome/bridge tests pass
   updated where geometry changed.

## Open Questions

- Tear-line signature: group eyebrow labels sit on a **dotted
  `PAPER_DIM` rule** running to the section edge — a perforated-strip
  motif native to the pop-up-book skin. Include, or keep labels bare?
  (Default in the tasks: include; one-line removal if you veto.)
- Middle truncation keeps `flintshot-…-10-00-00` readable but hides
  the middle; alternative is head-only `flintshot-2026-09-2…`.
  (Default: middle.)
