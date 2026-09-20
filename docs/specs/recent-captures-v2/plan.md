# Plan: Recent Captures v2 — standard storage, richer cards, clipboard-copy persistence

## Overview

Four upgrades to the recents experience, per `spec.md`: (1) a real,
configurable save location — empty `save-dir` resolves to
`~/Pictures/Flintshot` at runtime, the directory is created on first
write, and Settings grows a folder picker; (2) richer recents cards
(filename + dimensions + size + age captions, hover Copy/Reveal/Delete
chips, scrollable grid); (3) flintshot's own clipboard copies persist
to disk (and thus recents) behind `save-clipboard-copies = true`; (4) a
configurable recents capacity (`recent-captures-limit`, default 50,
clamped 1–200) with the `recents.toml` schema untouched (path + mtime).

## Architecture Decisions

- **Shared save logic lives in core.** `flintshot-app`'s dep on
  `flintshot-editor-egui` is optional (behind the `egui` feature), but
  the capture path (`after_grab = clipboard`, daemon-spawned workers)
  needs the same filename/collision/encode logic as the editor. The
  date/time renderers, collision probe, and png/jpeg writer move to a
  new `flintshot-core::output` module; editor-egui's `save_direct`
  refactors onto it.
- **`[output]` must merge on re-save.** `Config::merge_into_existing`
  currently never writes `[output]` (and misses three `[ui]` keys), so
  a Settings picker change would not survive a re-save. Task 001 fixes
  the merge for the whole output table plus the missing ui keys — a
  prerequisite for acceptance criterion 2.
- **Limit is a parameter, not ambient state.** `Recents::record`
  grows a `limit: usize` param; callers pass the clamped config value.
  Core's recents module never loads config itself. Load never
  truncates (acceptance 5: legacy 20-entry indexes grow on next
  record).
- **Open questions resolved by the spec's stated defaults**: delete is
  permanent (two-step chip confirm, no `trash` dep); clipboard
  persistence uses `[output] format` (PNG default); reveal failure
  falls back to a toast carrying the raw path.
- **Clipboard handle lifetime (X11 owner rule)**: the home window's
  copy-recent keeps its `arboard::Clipboard` handle in shared state
  for the window's lifetime — same rule as the editor's copy slot.

## Task Index

### Phase 1: Foundation (core)
- [ ] Task 1: Config fields, `resolve_save_dir`, and the `[output]` re-save merge → `tasks/001-config-save-dir-and-fields.md`
- [ ] Task 2: Core `output` save helpers + recents limit/remove plumbing → `tasks/002-core-output-and-recents-limit.md`

### Checkpoint: Foundation
- [ ] Four finish gates pass; core tests cover resolution, defaults, merge, truncation, remove

### Phase 2: Clipboard-copy persistence
- [ ] Task 3: Editor Copy persists + save-dialog prefill → `tasks/003-editor-copy-persist.md`
- [ ] Task 4: Capture-path clipboard copy persists → `tasks/004-capture-copy-persist.md`

### Checkpoint: Persistence
- [ ] Acceptance 1 and 3 hold: default-config writes land in `~/Pictures/Flintshot`; copies produce recents entries

### Phase 3: Recents UI
- [ ] Task 5: `HomeBridge` copy/reveal/delete + limited `recent_images` → `tasks/005-bridge-card-actions.md`
- [ ] Task 6: Rich cards — metadata caption, hover chips, scroll → `tasks/006-rich-recents-cards.md`

### Checkpoint: UI
- [ ] Acceptance 4 holds (manual pass): captions, chips, delete, copy, reveal

### Phase 4: Settings + docs
- [ ] Task 7: Settings — save-location picker, clipboard toggle, limit row → `tasks/007-settings-rows.md`
- [ ] Task 8: Docs + CHANGELOG + full gate run → `tasks/008-docs-changelog.md`

### Checkpoint: Complete
- [ ] All acceptance criteria verified (tests for 1, 3-partial, 5, 6; manual for 2, 4)
- [ ] Ready for review

## Risks and Mitigations

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| `merge_into_existing` gap breaks picker persistence | High | Task 001 fixes the merge first, pinned by round-trip-into-existing-file tests |
| X11 clipboard clears after copy-recent (owner rule) | Med | Handle parked in `Shared` for the window's lifetime; core rule already proven by the editor slot |
| Blocking `rfd::pick_folder` on the UI thread | Low | Same precedent as `pick_image_path` / `prompt_png_path` (the eframe loop is already blocking) |
| Home window test doubles break from new trait methods | Low | New `HomeBridge` methods ship with no-op default impls (spec's stated pattern) |

## Open Questions

- None blocking — the spec's three open questions carry stated
  defaults, adopted above and flagged in the final summary.
