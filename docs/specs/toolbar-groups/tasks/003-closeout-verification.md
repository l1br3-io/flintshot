# Task 003: Closeout — narrow-width matrix, CHANGELOG, review kit

**Description:** Final verification sweep across both surfaces,
including the narrow-capture stress the spec calls out (a ~500 px
region capture is wider than nothing but far narrower than the full
pod rail), plus the CHANGELOG entry and the PR review kit. No new
product code unless the matrix finds a defect (fix-forward, then
re-run the gates).

**Acceptance criteria:**
- [ ] All four gate commands green in one final run
- [ ] Manual matrix: `cargo run -- region` with a ~500 px-wide
      selection — editor-rail fallback (horizontal scroll) reaches
      every control; overlay toolbar stays on-panel with no clipped
      or overlapping chips
- [ ] Manual matrix: default `cargo run -- app` editor window at
      normal width — pods perceivable without tooltips on both
      surfaces
- [ ] `CHANGELOG.md` updated under `## [Unreleased]` (user-visible
      toolbar change)
- [ ] PR screenshots attached (editor + overlay, normal and narrow)
- [ ] Spec success criteria 1-4 checked against the built app; any
      miss is either fixed or recorded in the spec's Open Questions
      with the human's call

**Verification:**
- [ ] `cargo fmt --all --check` && `cargo clippy --workspace
      --all-targets -- -D warnings` && `cargo test --workspace
      --all-targets` && `cargo test --doc -p flintshot-core -p
      flintshot-editor-egui`
- [ ] Manual checks above, screenshots saved

**Dependencies:** Task 002

**Files likely touched:**
- `CHANGELOG.md`
- (Contingency fixes only where the manual matrix finds a defect)

**Docs:**
- If the narrow-width scroll behavior surprises, a why-comment at
  the ScrollArea site — it already documents the fallback, so
  likely nothing.

**Estimated scope:** Small: 1 file + manual checks
