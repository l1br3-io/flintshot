# Task 008: finish — gates, CHANGELOG, manual pass

**Description:** Close the effort: run the full gate battery, update
`CHANGELOG.md` under `## [Unreleased]` (user-visible change: adaptive
grids, day-grouped recents, section headers, Open folder), and do the
spec's manual verification pass end to end.

**Acceptance criteria:**
- [ ] `cargo fmt --all --check` clean
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `cargo test --workspace --all-targets` green
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui` green
- [ ] `CHANGELOG.md` has an `## [Unreleased]` entry describing the
      visible changes
- [ ] Manual pass per spec Commands section (resize reflow, group
      labels with seeded mtimes, Open folder, empty states)

**Verification:**
- [ ] The four gate commands above, in order

**Dependencies:** Tasks 1–7

**Files likely touched:**
- `CHANGELOG.md`

**Docs:** If any gotcha surfaced without a code home, it lands in
`docs/gotchas.md` with a one-line AGENTS.md index pointer.

**Estimated scope:** Small — 1 file
