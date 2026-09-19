---
status: active
started: 2026-09-20
---

# Spec: Recent Captures v2 — standard storage, richer cards, clipboard-copy persistence

## Objective

Upgrade the "recent captures" experience along three axes. **User**: Linux
screenshot users (X11/Wayland) who capture, occasionally annotate, and hand
off images fast — and increasingly want their shot history to be a real
archive, not a lucky cache.

### 1. Industry-standard, configurable storage location

Every capture written by flintshot lands somewhere predictable and
user-changeable:

- Empty `[output] save-dir` resolves **at runtime** to
  `dirs::picture_dir()/flintshot` — `~/Pictures/Flintshot` on Linux
  (XDG `user-dirs` aware), `~/Pictures/Flintshot` on macOS,
  `…\My Pictures\Flintshot` on Windows. This implements a fallback that
  is already **documented in three places but never coded**
  (`config.rs` field docs, `app.rs` `direct_save_path` doc comment, and
  the settings row's readonly "Pictures (default)" label) — today an
  empty `save-dir` with a filename template silently writes to the
  process CWD.
- The resolved directory is created on first write (`create_dir_all`),
  failure surfaces as a toast, never a panic.
- The `rfd` save dialog prefills the resolved directory when
  `save-dir` is empty (and the configured one otherwise).
- **Settings gains a real directory picker** — the "Save location" row
  is currently readonly (settings.rs:321); it becomes a
  `rfd::FileDialog::pick_folder()` browse button + path display +
  "Use default" reset.

### 2. Richer recent-capture cards (popup-book design language)

The home-page recents grid stays visually native to the shipped
pop-up-book system (`theme/palette.rs` tokens, `CUTOUT_RADIUS`,
`SHADOW_PAPER`, Jakarta Sans) — no parallel visual system. Changes:

- **Metadata caption** per card: `1920×1080 · 240 KB · 5 mins ago`,
  with the filename line above (today: stem + age only). Dimensions
  come free from the existing thumbnail decode; size from one
  `fs::metadata` stat — **`recents.toml` schema stays path + mtime**
  (no migration).
- **Hover actions**: hovering a card reveals a row of cutout chips
  (the `floating.rs` `cutout_chip` pattern) over the thumbnail —
  **Copy** (file → clipboard via arboard, honoring the Wayland
  keep-alive rules), **Reveal** (open the parent folder in the file
  manager — `xdg-open`/`open`/`explorer` via `std::process::Command`,
  no new dep), **Delete** (two-step confirm on the chip itself —
  click turns it into "Sure?" — removes the file from disk and drops
  it from the recents index).
- Click anywhere else on the card = open in editor (unchanged).
- **Scrollable grid**: the section scrolls once cards overflow the
  window; still 4 per row.

### 3. Clipboard copies are persisted

Flintshot's **own** copy-to-clipboard actions (editor **Copy** button,
`after-grab = clipboard`) optionally also write the image to disk so it
lands in recents — today a copy is invisible history.

- New config: `[output] save-clipboard-copies` (bool, **default
  `true`**), toggleable in Settings.
- Naming: the filename template if set, else the built-in
  `flintshot-{date}-{time}` scheme; same collision suffixing
  (`-2`, `-3`) and resolved save directory as direct saves.
- A save failure **never fails the copy** — the clipboard write
  already succeeded; log + toast, move on.

### 4. Configurable capacity

- New config: `[ui] recent-captures-limit` (int, **default `50**`,
  clamped 1–200). Drives `Recents` truncation and the grid.

### Acceptance criteria

1. With default config, a template-driven save or clipboard-copy save
   writes to `~/Pictures/Flintshot/`, creating it if missing.
2. Setting `save-dir` in `config.toml` **or** via the Settings picker
   redirects all subsequent writes; the change survives restart and
   hot-reload.
3. Copying from the editor (Copy button) with
   `save-clipboard-copies = true` produces a file in recents within
   one refresh cycle; with `false`, no file is written and the copy
   still succeeds.
4. Each recents card shows filename, dimensions, size, age; hover
   reveals Copy / Reveal / Delete; Delete removes file + index entry;
   Copy puts the image on the clipboard; Reveal opens the folder.
5. Recents truncate at the configured limit; old 20-entry indexes
   load unchanged and grow to the new limit on next record.
6. `recents.toml` written by any prior version loads with zero
   migration steps.

## Technologies

Existing stack only — no new dependencies:

- `flintshot-core`: `dirs`, `serde`/`toml_edit`, `thiserror`
- `flintshot-editor-egui`: `egui`/`eframe`, `image`, `arboard`
- File-manager reveal: `std::process::Command` (`xdg-open` /
  `open` / `explorer`), selected by `std::env::consts::OS`

## Commands

```sh
cargo build                                # default members
cargo fmt --all --check                    # format gate
cargo clippy --workspace --all-targets -- -D warnings   # lint gate
cargo test --workspace --all-targets       # unit + integration
cargo test --doc -p flintshot-core -p flintshot-editor-egui  # doc tests
cargo run -- app                           # manual verify (home + settings)
cargo run -- capture                       # manual verify (after-grab=clipboard)
```

## Project Structure

| Change                                     | File(s)                                                |
| ------------------------------------------ | ------------------------------------------------------ |
| Save-dir resolution (`resolve_save_dir`)   | `flintshot-core/src/config.rs` (+ unit tests)          |
| New config fields + default template + docs| `flintshot-core/src/config.rs`, `config.default.toml`  |
| Recents limit plumbing (record/truncate)   | `flintshot-core/src/recents.rs`                        |
| Clipboard-copy persistence (editor path)   | `flintshot-editor-egui/src/app.rs` (`copy_to_clipboard`) |
| Clipboard-copy persistence (capture path)  | `flintshot-app/src/capture_flow.rs`                    |
| Save-dialog prefill + dir creation         | `flintshot-editor-egui/src/app.rs`                     |
| Card redesign + hover chips + scroll       | `flintshot-editor-egui/src/home.rs`                    |
| New `HomeBridge` methods (copy/reveal/delete) | `flintshot-editor-egui/src/home.rs` (trait) + `flintshot-app/src/app_window.rs` (impl) |
| Settings: picker row, clipboard toggle, limit field | `flintshot-editor-egui/src/settings.rs` (+ bridge `SettingsAction` if needed) |
| Config schema doc fix (stale snake_case)   | `docs/ARCHITECTURE.md`                                 |
| CHANGELOG                                  | `CHANGELOG.md` `[Unreleased]`                          |

Spec + plan + tasks: `docs/specs/recent-captures-v2/`.

## Code Style

Repo conventions (see `AGENTS.md`) — representative of the seams this
effort touches:

```rust
// core: pure resolution, kebab-case serde, non_exhaustive, documented
#[serde(default)]
pub save_clipboard_copies: bool,

/// Resolve the effective save directory: the configured path, or the
/// platform pictures dir (`~/Pictures/Flintshot`) when unset.
#[must_use]
pub fn resolve_save_dir(&self) -> PathBuf { … }

// editor-egui: bridge default-methods keep test doubles compiling
/// Copy a recent capture's file to the clipboard. Default: no-op.
fn copy_recent(&mut self, _path: &Path) {}
```

- `#![forbid(unsafe_code)]`, no `unwrap()`/`expect()` outside tests
- `thiserror` in core, toast-style surfacing in UI; failures degrade
  (dead card → skipped, failed save → warn)
- No comments restating code; "why" comments only
- `max_width = 100` rustfmt

## Testing Strategy

- **`flintshot-core`** (pure logic, the bulk):
  - `resolve_save_dir`: explicit path passthrough; empty →
    pictures-dir join (inject the base dir as a parameter so the
    test doesn't depend on the host env)
  - `Recents`: truncation at configured limit, dedupe/front-move
    order preserved, old-format TOML fixture loads, growth from
    legacy 20-entry index
  - Config: new fields default correctly, round-trip through
    `toml_edit` comment preservation (existing test pattern),
    default template contains the new keys
- **`flintshot-editor-egui`**: caption formatter unit tests
  (dims/size/age strings); existing home-page smoke tests extended
  for scroll + chips presence; bridge no-op defaults compile
- **`flintshot-app`**: capture-flow copy persistence exercised where
  the existing test seams allow; otherwise manual
- **Manual pass** (acceptance 1–4 above) via `cargo run -- app` /
  `capture` on Wayland

## Boundaries

- **Always**: run the four finish gates (fmt / clippy / test /
  doc-test) before declaring done; kebab-case TOML fields;
  `#[serde(default)]` on new fields; `#[non_exhaustive]` where
  plausible; core stays GUI-type-free (`no_gui_types_in_core` test);
  update `CHANGELOG.md`; keep clipboard Wayland ownership rules
  (fresh handle per read, keep-alive for writes)
- **Ask first**: new dependencies (e.g. a `trash` crate — see Open
  Questions); changing the default save behavior away from
  dialog-first; any `recents.toml` schema change; touching in-flight
  efforts' specs
- **Never**: persist thumbnails (settled decision in
  `popup-book-ui`); `unsafe`; break/migrate existing `recents.toml`
  or `config.toml`; system-wide clipboard watching in this effort

## Success Criteria

- All four finish gates pass
- Acceptance criteria 1–6 verified (tests for 1, 3-partial, 5, 6;
  manual for 2, 4)
- Settings shows the effective save path and can change it; UI never
  displays a path the save path won't honor
- CHANGELOG entry under `[Unreleased]`; ARCHITECTURE config schema
  now matches the real struct

## Open Questions

1. **Delete: trash or permanent?** Spec'd as permanent with two-step
   chip confirm (zero deps). A `trash` crate would be friendlier —
   new dependency, so: ask first. Default if unanswered: permanent.
2. **Format for clipboard-persisted files**: use `[output] format`
   (PNG default) like direct saves — assumed; flag if JPEG-default
   copies should stay PNG for losslessness.
3. **Reveal on failure** (no file manager): toast with the raw path —
   assumed sufficient.
