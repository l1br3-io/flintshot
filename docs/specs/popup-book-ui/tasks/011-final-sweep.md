# Task 011: Token-only sweep + regression + CHANGELOG

**Description:** Close the loop against the spec's success criteria:
verify `rg "Color32::from_" crates/flintshot-editor-egui/src`
matches only `theme/palette.rs` (fix any strays); run the full
shortcut regression (every single-key tool shortcut, Esc/Enter,
copy/save/paste accelerators); confirm light theme pinned, grain
never over canvas, seams inert; update `CHANGELOG.md` under
`## [Unreleased]`; write an ADR only if a non-obvious choice emerged
(e.g., drag-gating re-derivation).

**Acceptance criteria:**
- [ ] Color-literal scan clean (palette-only)
- [ ] Spec success criteria 1–12 each verified or explicitly waived
      with reason
- [ ] CHANGELOG updated; spec status ready to flip on merge

**Verification:**
- [ ] `cargo fmt --all --check`; clippy `-D warnings`; full test
      suite + doc tests green
- [ ] Manual end-to-end: welcome → capture → annotate (all ten
      tools) → copy; open; recents; drop; pin; upload

**Dependencies:** Tasks 001–010

**Files likely touched:**
- Any stray call sites found by the scan
- `CHANGELOG.md`
- `docs/adrs/` (only if warranted)

**Docs:**
- ADR per repo convention if the gating/layout decision is
  non-obvious.

**Estimated scope:** Small
