# Task 003: Spring easing + paper-grain primitives

**Description:** Two pure, unit-tested primitives in `theme/`:
`ease.rs` — a cubic-bezier evaluator for (0.34, 1.56, 0.64, 1) with
a `spring(t)` entry point and an animation-state helper driven by
`Context::animate_value_with_time`-style manual progress; `grain.rs`
— a deterministic procedural paper-grain tile generator (seeded,
pure function → same pixels every run) plus a painter helper that
tiles it at low alpha over a rect, never exceeding the alpha ceiling
token.

**Acceptance criteria:**
- [ ] `ease::spring(0.0)==0.0`, `spring(1.0)==1.0`, overshoot stays
      within a bounded envelope, monotone input → smooth output
      (unit tests)
- [ ] Grain tile is deterministic (same seed → identical bytes) and
      max alpha ≤ the token ceiling (unit tests)
- [ ] Neither module mutates egui state; both are callable headless
      in tests

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui` green with new tests
- [ ] Manual: no consumer yet — this task ships primitives only

**Dependencies:** Task 001

**Files likely touched:**
- `crates/flintshot-editor-egui/src/theme/ease.rs` (new)
- `crates/flintshot-editor-egui/src/theme/grain.rs` (new)
- `crates/flintshot-editor-egui/src/theme/mod.rs` (re-exports)

**Docs:**
- One-line rationale in `ease.rs` for why egui's built-in
  exponential ease-out can't express overshoot.

**Estimated scope:** Small (2 new files + wiring)
