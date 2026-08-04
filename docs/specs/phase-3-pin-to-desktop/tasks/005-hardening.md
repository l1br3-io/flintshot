# Task 005: Hardening — boundary test, CHANGELOG, ADRs, docs

**Description:** Close out Phase 3. The code is done (Tasks 001-004); this task locks in the lasting decisions (ADRs), records user-visible changes (CHANGELOG), extends the manual test matrix, documents the cross-platform caveats (Wayland always-on-top, macOS window level), and confirms the boundary test stays green. No new features; this is the gate before review.

**Acceptance criteria:**
- [ ] `crates/flintshot-core/tests/no_gui_types_in_core.rs` passes unchanged (the pin primitive is a trait method in `flintshot-editor-egui`, not a core type; no `eframe`/`egui`/`interprocess` types entered core's public API). Re-run and confirm green. No new banned imports needed (the only new IPC variant `ReportSaved` carries `PathBuf`).
- [ ] **ADR-0023** drafted at `docs/adrs/ADR-0023-pin-as-separate-process.md`: Context (pin must outlive editor + daemon; deferred egui viewport dies with its root window), Decision (spawn `flintshot pin <path>` as a detached process via the Phase 2 `worker::CommandSpawner` pattern), Consequences (pin survives editor crash / daemon restart; pin's event loop never contends with editor's; reuses ADR-0019's model). Cross-link ADR-0019.
- [ ] **ADR-0024** drafted at `docs/adrs/ADR-0024-worker-reports-saved-to-daemon.md`: Context (Phase 2 daemon emits `EditorOpened` as a process-started proxy; `Event::Saved` never reaches subscribers; `PinLast` needs the saved path), Decision (fire-and-forget `Request::ReportSaved { path }` over the existing socket; non-fatal on no-daemon), Consequences (subscriber stream gets real outcomes; `PinLast` reads `last_saved`; worker→daemon direction inversion is contained to one variant). Considered alternatives: side-channel file (racey, needs a watcher), daemon-side directory watch (fragile, ignores user-chosen save paths). Cross-link ADR-0010 (interprocess), ADR-0011 (postcard).
- [ ] `docs/adrs/README.md` index updated with ADR-0023 + ADR-0024 (one line each, matching the existing entries' format).
- [ ] `CHANGELOG.md` `[Unreleased]` section gains a `#### Phase 3 — Pin-to-desktop` subsection under `### Added` with entries for: the pin primitive (`EguiShell::run_pin` + `flintshot pin`), `EditorOutcome::Pinned` + editor Pin button, `Request::ReportSaved` + worker→daemon feedback (note it closes the Phase 2 `Event::Saved` gap), `Request::PinLast` daemon handler, `pin_cache_dir()` helper. And a `### Notes` bullet recording the Phase 3 ship date + the COSMIC-applet deferral to `phase-3b-cosmic-applet`.
- [ ] `docs/TEST_MATRIX.md` gains rows for: pin window open/drag/close (X11, Wayland, macOS, Windows), always-on-top behaviour per platform, editor *Pin* button, `flintshot pin <path>` CLI, multi-pin concurrency, `PinLast` over IPC. Mark untested platforms as "pending" rather than inventing results.
- [ ] Cross-platform notes added to `README.md` (and `AGENTS.md` if the existing pattern puts them there): Wayland always-on-top is compositor-dependent (borderless works; always-on-top may not — layer-shell is the real fix, deferred to a future effort); macOS `with_always_on_top()` maps to `NSWindow.level = .floating` (verify via manual check and record whether `.statusBar` is needed for fullscreen-above); Windows always-on-top works via `WS_EX_TOPMOST` (handled by winit).
- [ ] `docs/ROADMAP.md` table updated: Phase 3 row splits into "Phase 3 — Pin-to-desktop" (this effort, In flight → Shipped) and "Phase 3b — COSMIC applet" (Next). The milestone note at `ROADMAP.md:9-10` ("CURRENT MILESTONE: Phase 3 — Pin-to-desktop + COSMIC applet") is updated to reflect the split.
- [ ] The `flintshot-cosmic` crate's stub comment at `crates/flintshot-cosmic/src/main.rs:5-6` ("Phase 3 fills in the panel button…") is updated to point at `phase-3b-cosmic-applet` instead of "Phase 3" (the milestone split moved it).
- [ ] Any `// TODO: Phase 3` / `// Phase 3:` / `not implemented yet (Phase 3)` comments in the codebase are resolved or re-pointed. Grep: `rg -n "Phase 3" crates/` — every hit should either be done (remove the marker) or explicitly deferred to `phase-3b` (update the marker).

**Verification:**
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-targets` — full suite green (no new tests in this task; this is the regression gate for the whole effort).
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] `cargo build -p flintshot-cosmic` — opt-in crate still builds.
- [ ] `python3 .opencode/harness/scripts/lint-plan.py` — the Phase 3 plan passes the thin-plan check (and prior efforts' plans still pass).
- [ ] Manual (needs display): run the full Phase 3 exit-criteria checklist from `spec.md` §Exit criteria (steps 1-8) on at least one platform. Record results in `docs/TEST_MATRIX.md`.

**Dependencies:**
- Task 001 (pin primitive — what the ADR documents).
- Task 002 (ReportSaved — what ADR-0024 documents).
- Tasks 003, 004 (their user-visible changes go in CHANGELOG).

**Files likely touched:**
- `docs/adrs/ADR-0023-pin-as-separate-process.md` — **NEW**.
- `docs/adrs/ADR-0024-worker-reports-saved-to-daemon.md` — **NEW**.
- `docs/adrs/README.md` — two new index lines.
- `CHANGELOG.md` — `#### Phase 3 — Pin-to-desktop` subsection + Notes bullet.
- `docs/TEST_MATRIX.md` — pin-related rows.
- `README.md` — cross-platform pin caveats.
- `AGENTS.md` — cross-platform note pointer (one line; don't inline prose — the rules file stays lean per the Definition of Done).
- `docs/ROADMAP.md` — Phase 3 / 3b split.
- `crates/flintshot-cosmic/src/main.rs` — comment re-point (one line).
- Code-level `// Phase 3` marker cleanup (grep-driven; touch only the stale markers).

**Docs:**
- This task IS the docs task. The ADRs follow the established `Context · Decision · Consequences` format (see `docs/adrs/ADR-0019-daemon-spawns-worker-processes.md` for the closest analogue).
- The CHANGELOG subsection mirrors the `#### Phase 2` / `#### Phase 2b` structure already in `CHANGELOG.md` (`:288`, `:376`).

**Estimated scope:** Medium (8-10 files, but all docs/comments — no production code changes except the cosmic stub comment and stale-marker cleanup).

**Notes for the implementer:**
- The ADRs should be drafted from the `// RATIONALE:` comments left in Tasks 001 and 002 — those are the decision records' raw material. If a rationale comment and the ADR diverge, the ADR is canonical; update the comment to point at the ADR.
- The ROADMAP split is a doc-only change but it's the signal that `phase-3b-cosmic-applet` is a real follow-up, not a forgotten half of Phase 3. The next effort starts on a fresh branch (`git checkout -b phase-3b-cosmic-applet`) with `/spec` writing to `docs/specs/phase-3b-cosmic-applet/spec.md`.
- Don't flip `spec.md`'s frontmatter to `status: shipped` here — that happens in the merge commit when the PR lands, per the Effort File Lifecycle. This task leaves it at `status: active`.
- If the manual test matrix reveals a platform where pin doesn't work (e.g. always-on-top silently ignored on a Wayland compositor), record it honestly in TEST_MATRIX + the cross-platform notes — don't pretend. The spec already scoped layer-shell out.
