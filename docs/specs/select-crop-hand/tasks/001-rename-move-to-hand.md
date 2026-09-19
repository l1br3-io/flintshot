# Task 001: Rename `Tool::Move` → `Tool::Hand`

**Description:** Pure identifier rename across the workspace. The tool's
behavior stays exactly today's select-and-move semantics; only the name (and
the rail chip label) changes. This shrinks every later diff and frees the
`Move` name from the codebase before the semantic work lands.

**Acceptance criteria:**
- [ ] `Tool::Hand` exists; `grep -rn "Tool::Move" crates/` returns nothing
- [ ] Rail chip label reads "Hand" (key hint may stay `V` until Task 002)
- [ ] All tests, renamed where they referenced `Tool::Move`, pass with no
      behavioral change
- [ ] Doc comments referencing the Move tool are reworded (module docs in
      `chrome.rs`, `editor.rs` Tool enum docs)

**Verification:**
- [ ] `cargo test --workspace --all-targets` green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `cargo fmt --all --check` clean

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-core/src/editor.rs` (variant + docs + tests)
- `crates/flintshot-editor-egui/src/{rail,panel,chrome,app,overlay,shell}.rs`
- `crates/flintshot-app/src/cli.rs` (test assertion + doc mention)

**Docs:** Gotchas → none expected (mechanical). `flip_to_move` field name
intentionally stays until Task 004.

**Estimated scope:** Medium (many files, sed-level edits)

**Notes:** Do NOT rename `AnnotationDrag`, `route_move_press`, `end_move`,
`update_move`, `move_chrome` — internal names, still accurate. `flip_to_move`
also stays (Task 4). The temporary inconsistency (flip targets Hand) is
config-gated off by default and fixed in Task 004.
