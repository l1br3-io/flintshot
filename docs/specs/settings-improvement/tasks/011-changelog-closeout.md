# Task 011: CHANGELOG + full finish gates

**Description:** Record the user-visible changes under
`## [Unreleased]` and run the effort's standing gates end to end.

**Acceptance criteria:**
- [ ] CHANGELOG entry under `## [Unreleased]` covering: the tray /
  applet Settings swap (and the two removed items), `app --route
  settings`, the settings-page layout fix, the config-file links +
  embedded defaults, and the Wayland-gated rebind row
- [ ] `cargo fmt --all --check` green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` green
- [ ] `cargo test --workspace --all-targets` green
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
  green
- [ ] `cargo build -p flintshot-cosmic` green
- [ ] Spec's Success Criteria walked item by item; any gap either
  fixed or flagged to the user explicitly

**Verification:**
- [ ] The four finish commands above, from the repo root

**Dependencies:** Tasks 1–10

**Files likely touched:**
- `CHANGELOG.md`

**Docs:**
- ADR only if a non-obvious choice emerged during implementation
  that the spec + plan don't already cover (none expected).

**Estimated scope:** Small: 1 file
