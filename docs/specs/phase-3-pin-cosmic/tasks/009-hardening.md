# Task 9: Hardening — boundary test, CHANGELOG, ADR-0023, TEST_MATRIX

**Description:** Close out Phase 3. Extend the
`no_gui_types_in_core.rs` boundary test to also reject
`use cosmic` / `use libcosmic` / `use iced` / `use slint` in
`flintshot-core`'s public API (it currently checks `egui`/
`interprocess`/`global_hotkey`/`tray_icon`/`notify`). Finalise
`CHANGELOG.md` `[Unreleased]` for both deliverables. Confirm ADR-0023
(Task 3) is merged and referenced. Add `docs/TEST_MATRIX.md` rows for
pin (drag, close, hover-opacity, resize, copy-on-close, multi-pin,
cross-monitor) and the COSMIC applet (each action, event feed,
daemon-absent, per-monitor submenu). Final gate run.

**Acceptance criteria:**
- [ ] `crates/flintshot-core/tests/no_gui_types_in_core.rs` extended:
      the scanner regex/set includes `cosmic`, `libcosmic`, `iced`,
      `slint` (in addition to the existing set). A deliberate
      `use libcosmic::…` in core's `src/` makes the test fail (then
      removed). Verify the test still passes green against the real
      source.
- [ ] `CHANGELOG.md` `[Unreleased]` has coherent **Added** entries for:
      `PinConfig`/`[pin]` section; `flintshot pin` CLI; pin window
      (borderless always-on-top, 5 UX features); editor Pin button;
      `Request::ReportSaved` + worker save reporting; daemon last-saved
      tracking; daemon `PinLast`/`OpenLast` + `Event::Pinned`; COSMIC
      panel applet (replaces stub); COSMIC event stream + daemon-absent
      state. Any **Changed**/**Removed** entries if signatures moved
      (e.g. `EditorOutcome::Pinned` if Task 5 added it).
- [ ] `docs/adrs/ADR-0023-worker-report-saved.md` present, referenced
      from `docs/adrs/README.md` index, and linked from `CHANGELOG.md`.
- [ ] `docs/TEST_MATRIX.md` has Phase 3 rows: pin (per UX feature ×
      Linux/macOS/Windows where applicable), COSMIC applet (per action,
      event feed, daemon-absent, per-monitor submenu — Linux only).
- [ ] `docs/ARCHITECTURE.md` updated where Phase 3 changed the surface:
      §Config (the `[pin]` section fields), §Process model (the `pin`
      subcommand + optional `open`), §Crate responsibilities (the
      COSMIC applet is no longer a stub), §IPC protocol (note
      `ReportSaved` as a worker-originated variant). Keep descriptions
      in timeless "current state" language.
- [ ] `docs/ROADMAP.md` Phase 3 row moves to **Shipped** when the
      effort merges (the `/ship` step flips it + the spec frontmatter).
- [ ] Final standing gate green:
      `cargo fmt --all --check`,
      `cargo clippy --workspace --all-targets -- -D warnings`,
      `cargo test --workspace --all-targets`,
      `cargo test --doc -p flintshot-core -p flintshot-editor-egui`,
      `cargo build -p flintshot-cosmic` (Linux).

**Verification:**
- [ ] All four gate commands above green.
- [ ] `cargo build -p flintshot-cosmic` green on Linux.
- [ ] `python3 .opencode/harness/scripts/lint-plan.py` green (the plan
      stays a thin index).
- [ ] `python3 .opencode/harness/scripts/check-refs.py` (if it checks
      doc links) green — no broken cross-links in the new docs.
- [ ] Manual: walk the spec's exit-criteria checklist
      (`spec.md` §Exit criteria) — all six steps pass.

**Dependencies:** All prior tasks (1–8). This is the close-out.

**Files likely touched:**
- `crates/flintshot-core/tests/no_gui_types_in_core.rs` — extend the rejected-import set.
- `CHANGELOG.md` — Phase 3 entries.
- `docs/adrs/ADR-0023-worker-report-saved.md` — verify/finalise (drafted in Task 3).
- `docs/adrs/README.md` — index ADR-0023.
- `docs/TEST_MATRIX.md` — Phase 3 rows.
- `docs/ARCHITECTURE.md` — §Config, §Process model, §Crate responsibilities, §IPC protocol updates.
- `docs/ROADMAP.md` — Phase 3 → Shipped (at `/ship` time).

**Docs:**
- This task *is* the docs task. No additional gotchas file unless a
  non-obvious trap surfaced during 1–8 that has no code home.

**Estimated scope:** Medium (6–7 files; all documentation + test extension, no new logic).
