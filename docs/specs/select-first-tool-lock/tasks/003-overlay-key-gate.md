# Task 003: Overlay keyboard gate on usable selection

**Description:** The overlay's floating rail never renders without
a usable selection and `annotation_press_allowed` denies early
presses — but the single-key tool switch in `logic_impl` (the
`tool_for_key` block, overlay.rs ~1324) is ungated, so pressing
`P` before any region invisibly arms Pencil. Gate that block on
`usable_selection(self.state.selection).is_some()`, with `Select`
always armable (it must keep working without a region — pinned by
`select_and_move_presses_always_flow` on the pointer side). No
visual changes; `annotation_press_allowed` and its tests are
untouched.

**Acceptance criteria:**
- [ ] No selection: pressing a non-Select tool key leaves
      `state.tool` unchanged; pressing `S` arms Select.
- [ ] Degenerate selection (sub-pixel) counts as absent — tool
      keys inert (reuse the `usable_selection` contract).
- [ ] Usable selection: all tool keys arm their tools exactly as
      today.
- [ ] `annotation_press_allowed`, floating-rail visibility, and
      accept/close key behaviour unchanged (existing tests green).

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui --quiet`
- [ ] `cargo clippy -p flintshot-editor-egui --all-targets -- -D warnings`
- [ ] Manual (needs display, X11): `cargo run -- region --overlay`
      — pre-selection, pressing tool keys changes nothing; drag a
      region; keys and rail live inside it.

**Dependencies:** None (independent of tasks 001–002; ordered
after them so the editor path — the bulk of the effort — is
verifiable end-to-end first).

**Files likely touched:**
- `crates/flintshot-editor-egui/src/overlay.rs`

**Docs:**
- None.

**Estimated scope:** Small (1 file)
