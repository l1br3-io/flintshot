# Task 009: Final sweep + full CI-parity verification

**Description:** Catch stragglers the focused tasks missed (doc
comments, test names, examples across `flintshot-app`), then run the
complete CI-parity gate from the spec and the manual smoke checks.
This task's acceptance list is the effort's Definition of Done.

**Acceptance criteria:**
- [ ] Sweep scrubbed: `rg -in "pin" crates/ justfile README.md
      docs/VISION.md docs/ARCHITECTURE.md docs/ROADMAP.md
      docs/TEST_MATRIX.md` returns only false positives
      (`std::pin`, `pin-project-lite`/`pin-utils` in Cargo files,
      "Snipping", "pinned MSRV", libappindicator, `.opencode/`
      harness docs, `docs/specs/phase-3-*`, CHANGELOG history,
      existing ADRs)
- [ ] `cargo fmt --all --check` green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` green
- [ ] `cargo clippy -p flintshot-app --features tray --all-targets -- -D warnings` green
- [ ] `cargo test --workspace --all-targets` green
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui` green
- [ ] `cargo build -p flintshot-cosmic` green
- [ ] Manual smoke: `cargo run -- --help` lists no `pin`;
      `cargo run -- pin x.png` fails as unknown subcommand
- [ ] Manual smoke: `cargo run -- region` overlay renders the output
      island without a Pin chip (X11/Wayland session required; skip
      with a note if headless)
- [ ] `docs/specs/remove-pin-to-desktop/todo.md` fully checked

**Verification:**
- [ ] Every command above exits 0; smoke results recorded in the
      task file or commit message

**Dependencies:** Tasks 001–008

**Files likely touched:**
- Whichever files the sweep finds (expected: a handful of one-line
  comment edits in `crates/flintshot-app/src/`; no logic)

**Docs:**
- None.

**Estimated scope:** Medium (grep-driven; edits are one-liners)
