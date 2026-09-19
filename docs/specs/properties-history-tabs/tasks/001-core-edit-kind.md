# Task 001: Core — `EditKind` + labeled snapshots at every push site

**Description:** Give every undo snapshot a kind. New
`flintshot-core/src/history.rs` defines `EditKind` (+
`#[non_exhaustive]`, `Display`, `Copy`) with variants per spec:
`Add { tool }`, `AddNumber`, `Move { count }`, `Delete { count }`,
`Crop`, `EditText`, `RestyleText`, `Property { kind:
PropertyEditKind }`, `Generic`. `Snapshot` gains a `kind` field;
`anns_only` / `full` take it as their first parameter. Wire every
production push site, mirror the label across the `undo()` /
`redo()` swap, and re-export from `lib.rs`.

Production push sites (all in `crates/flintshot-core/src/`):
- `editor.rs` `commit_annotation` (~line 643) →
  `Add { tool: owning_tool(&ann) }` (`owning_tool` at ~line 2074)
- `editor.rs` `place_number` (~687) → `AddNumber`
- `editor.rs` move-gesture lazy push (~891) →
  `Move { count: selected.len() }`
- `editor.rs` `delete_selected` (~933) → `Delete { count }`
- `editor.rs` `crop` (~995, `Snapshot::full`) → `Crop`
- `editor.rs` text re-edit commit (~1785) → `EditText`
- `editor.rs` `end_style_gesture` (~1908) → `RestyleText`
- `inspector.rs` `begin_property_edit` (~115) →
  `Property { kind }` (the `kind` parameter)
- `editor.rs` `undo()` (~1025) and `redo()` (~1056): the `current`
  struct literal inherits the popped/next entry's `kind`

Test push sites (~10 in `editor.rs` `#[cfg(test)]`) pass
`EditKind::Generic`.

**Acceptance criteria:**
- [ ] Every production push site records its specific kind; a
      scripted mixed session shows one `EditKind` per gesture
- [ ] Undo→redo round trip preserves the entry's kind on the redo
      side (mirroring)
- [ ] `Display` yields spec labels: `Added arrow`, `Added number`,
      `Moved 3 items` / `Moved 1 item`, `Deleted 2 items`,
      `Cropped`, `Edited text`, `Text style`, `Colour` / `Stroke
      width` / `Fill` / `Text style` / `Blur` / `Number size`,
      `Edit` for Generic
- [ ] `EditKind` re-exported from `flintshot-core` root; all
      existing tests updated and passing; no public API breakage
      beyond the constructor signature change (workspace-internal)

**Verification:**
- [ ] `cargo test -p flintshot-core`
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo fmt --all --check`

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-core/src/history.rs` (new)
- `crates/flintshot-core/src/editor.rs`
- `crates/flintshot-core/src/inspector.rs`
- `crates/flintshot-core/src/lib.rs`
- `crates/flintshot-core/src/editor.rs` (test module — same file)

**Docs:**
- The `Snapshot` doc comment gains a `kind` paragraph (what it is,
  mirroring rule). No AGENTS.md change.

**Estimated scope:** Medium: 4 files, all changes mechanical bar
the mirroring logic
