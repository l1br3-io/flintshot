# Task 001: `day_bucket` in core

**Description:** Add the pure day-bucketing primitive the recents
grouping renders from: `DayBucket` (`Today` / `Yesterday` / `Earlier`)
and `day_bucket(mtime: u64, now: u64) -> DayBucket` in
`crates/flintshot-core/src/recents.rs`. Buckets compare UTC civil days
via `civil_from_days` (promote it from private to `pub(crate)` in
`output.rs` — same crate, no public API churn). Guard the `mtime > now`
case as `Earlier`. `#[non_exhaustive]` on the enum. No UI changes.

**Acceptance criteria:**
- [ ] `day_bucket` returns `Today` for the same civil day, `Yesterday`
      for one day back, `Earlier` beyond that (unit-tested at: same
      instant, 23h ago, 1s past the day boundary, 25h ago, a week ago)
- [ ] `mtime > now` (clock skew) lands `Earlier`, no panic
- [ ] `civil_from_days` is `pub(crate)`; its algorithm is untouched
- [ ] `#![warn(missing_docs)]` satisfied — doc comments on the enum,
      variants, and fn

**Verification:**
- [ ] `cargo test -p flintshot-core`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-core/src/recents.rs`
- `crates/flintshot-core/src/output.rs`

**Docs:** Gotcha (UTC-only convention, deliberate) → doc comment on
`day_bucket` referencing `chrono_date`.

**Estimated scope:** Small — 2 files
