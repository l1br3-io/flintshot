# Task 012 — Boundary test + ADRs + CHANGELOG + cross-platform docs

> Phase 2.F · size: M · depends on: all

## Description

Close out Phase 2: lock the architecture boundary, finalise the ADRs
drafted in Tasks 003/005/006, record the work in CHANGELOG, and
document the cross-platform behaviour the phase introduced (Wayland
hotkey gap, macOS permission expectations). This is the gate that
proves the abstraction held and the phase is review-ready.

## Acceptance criteria

- [ ] `no_gui_types_in_core.rs` `FORBIDDEN_PREFIXES` extended with
      `use interprocess`, `use global_hotkey`, `use tray_icon`, `use
      notify` (note: `tray-icon` → `tray_icon`, `global-hotkey` →
      `global_hotkey` in Rust ident form). Test green.
- [ ] ADRs finalized: ADR-0018 (event-loop bridge), ADR-0019
      (daemon-spawns-worker), ADR-0020 (single-instance on daemon
      only); ADR-0021 (framing) if split in Task 004.
- [ ] `CHANGELOG.md` `[Unreleased]` has Added/Changed/Fixed/Notes
      entries for every user-visible Phase 2 change (daemon, tray,
      hotkeys, IPC, config hot-reload, CLI delegation, new deps).
- [ ] Cross-platform docs updated: `docs/ARCHITECTURE.md` §Cross-
      platform concerns (Wayland hotkey gap → Phase 6, macOS
      hotkey permission); `docs/TEST_MATRIX.md` gains Phase 2 rows
      (tray, hotkey, single-instance, config reload, per platform);
      `README.md` / `AGENTS.md` get a one-line index pointer to the
      Wayland-keybind workaround if warranted.
- [ ] `docs/ROADMAP.md` Phase 2 line updated to reflect the real scope
      (system integration; annotation tools moved to phase-2b).

## Verification

- [ ] Standing gate passes.
- [ ] `cargo test -p flintshot-core --test no_gui_types_in_core` green
      (the extended ban list catches any leak).
- [ ] Reviewer can read CHANGELOG + ADRs + TEST_MATRIX and understand
      what Phase 2 delivered and what it deliberately deferred.

## Dependencies

All prior tasks (this task codifies their outcomes).

## Files likely touched

- `crates/flintshot-core/tests/no_gui_types_in_core.rs`
- `docs/adrs/ADR-001{8,9}*.md`, `ADR-0020*.md`, (optional `ADR-0021*.md`)
- `CHANGELOG.md`
- `docs/ARCHITECTURE.md`, `docs/TEST_MATRIX.md`, `docs/ROADMAP.md`
- `README.md` / `AGENTS.md` (one-line index pointer only)

## Size

M
