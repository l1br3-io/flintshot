# Task 004: `flip-to-select` config rename + legacy migration

**Description:** Rename the post-commit flip policy from `flip-to-move` /
`Tool::Hand` to `flip-to-select` / `Tool::Select`. Config loads the legacy
key silently; re-saves write the new key and carry any comment with it.

**Acceptance criteria:**
- [ ] `UiConfig.flip_to_move` → `flip_to_select`; TOML key
      `flip-to-move` → `flip-to-select`; `EditorState.flip_to_move` →
      `flip_to_select`
- [ ] `commit_annotation` flips to `Tool::Select` when the policy is on and
      the ownership guard passes (`is_flip_eligible` unchanged)
- [ ] Loading a config with the legacy `flip-to-move` key reads its value
      into `flip_to_select`; re-saving writes `flip-to-select` and preserves
      any comment attached to the old key (toml_edit path)
- [ ] `config.default.toml` ships the new key with the updated doc comment

**Verification:**
- [ ] `cargo test -p flintshot-core --all-targets` green, including new
      migration + comment-preservation tests (mirror config.rs:1285, :1304)
- [ ] `grep -rn "flip.to.move\|flip_to_move" crates/` returns nothing

**Dependencies:** Task 003

**Files likely touched:**
- `crates/flintshot-core/src/config.rs` (field, defaults, toml_edit write, tests)
- `crates/flintshot-core/src/editor.rs` (field rename + flip target)
- `crates/flintshot-core/src/config.default.toml`
- `crates/flintshot-app/src/capture_flow.rs` or wherever the session seeds the
      field from config (grep `flip_to_move` in flintshot-app)

**Docs:** ADR lands in Task 008; the migration table lives there.

**Estimated scope:** Small-Medium (3-4 files)
