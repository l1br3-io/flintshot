# Task 002: `number_diameter` tool state + pub range constants

**Description:** Make the bubble diameter adjustable state. Add
`EditorState::number_diameter: f32` (pub field, same visibility
discipline as `next_number`), defaulted to `NUMBER_DEFAULT_DIAMETER`
(28.0, unchanged) wherever `EditorState` is constructed. Add
`NUMBER_MIN_DIAMETER` (16.0) and `NUMBER_MAX_DIAMETER` (96.0) as
`pub` consts beside the default (doc comments — the crate warns on
missing docs); the panel consumes them in Task 003.
`NUMBER_DEFAULT_DIAMETER` stays private. `place_number` commits
`diameter: self.number_diameter` instead of the constant, and its doc
comment's "per-state size control is currently out of scope" sentence
flips to describe the new field. No history entries for diameter
changes — snapshot discipline in `place_number` is untouched.

**Acceptance criteria:**
- [x] `EditorState::number_diameter: f32` pub field, doc-commented,
      defaulted to 28.0 at every construction site
      (`EditorState::new` + any test constructors that build state
      directly).
- [x] `pub const NUMBER_MIN_DIAMETER: f32 = 16.0` and
      `pub const NUMBER_MAX_DIAMETER: f32 = 96.0` in `editor.rs`,
      doc comments state the legibility floor / practical ceiling
      rationale; `NUMBER_DEFAULT_DIAMETER` remains private.
- [x] `place_number` uses `self.number_diameter`; state test: default
      state commits diameter 28.0.
- [x] State test: `s.number_diameter = 64.0` → next placement commits
      64.0; the one after (still 64.0) too.
- [x] State test: changing `number_diameter` after a placement does
      not mutate the committed annotation (per-annotation
      immutability).
- [x] State tests: counter increment + undo-decrement semantics
      unchanged (existing Task-004 suite still green, ideally one new
      assertion that a placement at a custom diameter still snapshots
     /restores `next_number` correctly).
- [x] Core's public API gains no GUI types (`no_gui_types_in_core.rs`
      still green).

**Verification:**
- [x] `cargo test -p flintshot-core --quiet`
- [x] `cargo clippy -p flintshot-core --all-targets -- -D warnings`
- [x] `just ci` (workspace gates incl. doc tests)

**Dependencies:** None (independent of Task 001; both touch core but
disjoint regions — `editor.rs` vs `render.rs`)

**Files likely touched:**
- `crates/flintshot-core/src/editor.rs` (field, consts, `place_number`, tests)

**Docs:**
- Doc comments on the new pub items (`#![warn(missing_docs)]`).
- CHANGELOG/TEST_MATRIX land at Task 004.

**Estimated scope:** Small (1 file)
