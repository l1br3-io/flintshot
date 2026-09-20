---
status: active
started: 2026-09-20
---

# Spec: Session Captures — unsaved captures survive navigation until app exit

## Objective

**User**: Linux screenshot users (X11/Wayland) who capture, annotate, and
hand off images — and today lose their edits the moment they leave the
editor page.

**Problem**: When the user navigates from the editor to Home (topbar Home
button, a Settings route, or an external capture superseding the editor),
in-session annotations are **lost** — documented as "the Replace policy's
loss" (`app_window.rs:2062`). Only the pre-edit image is stashed for
restore, and `recents.toml` records nothing until a capture is *saved*.
The unsaved middle ground — "I captured something, maybe annotated it,
poked around Home, and want to come back" — has no representation.

**Solution**: A **session capture store**. Every capture that lands in the
app-window editor is tracked — raw image plus committed annotations —
until the user saves it, discards it, or fully closes the app. Navigating
to Home no longer destroys work; it just files it. Home gains a "This
session" section listing resumable captures above the saved-recents grid.

### User stories

- As a user editing a capture, I go Home (accidentally or on purpose),
  and my capture — with every committed annotation — is a card I can
  click to resume editing.
- As a user mid-edit when a new capture supersedes the editor, my edits
  are stashed, not destroyed.
- As a user, saving a capture moves it to my normal recents (disk);
  discarding removes it everywhere.
- As a user who fully closes the app, session captures vanish — they
  were scratch space, and the temp dir reflects that.

### Decisions (confirmed with the user)

| Question | Decision |
| --- | --- |
| Session capacity | **Multiple captures** accumulate as resumable cards |
| Storage backend | **Temp dir** (`$XDG_RUNTIME_DIR/flintshot/session/`), OS-cleaned, swept on startup |
| Preserved state | **Image + committed annotations**; undo/redo resets on resume |
| Eviction | **Save or explicit discard** (two-step confirm) removes a capture; no LRU |
| Home surface | **Own section above the recents grid** |
| App lifetime | **App-window process exit** is the cutoff; graceful exit clears the dir |

## Technologies

No new dependencies. Everything needed is already in the workspace:

- `flintshot-core`: `serde`/`serde_json` (annotations are already
  `Serialize + Deserialize` — `editor.rs:67`), `image` (PNG encode/decode),
  `dirs` + `std::env::temp_dir` (runtime dir resolution), `thiserror`.
- `flintshot-app`: existing `Shared` cross-epoch state in `app_window.rs`.
- `flintshot-editor-egui`: existing home-grid card + thumbnail patterns
  from `recent-captures-v2` (`home.rs`).

## Commands

From the repo root (per `AGENTS.md`):

```sh
cargo build                              # default-members
cargo fmt --all --check                  # formatting gate
cargo clippy --workspace --all-targets -- -D warnings   # lint gate
cargo test --workspace --all-targets     # unit + integration
cargo test --doc -p flintshot-core -p flintshot-editor-egui   # doc tests
cargo run -- app                         # manual verification (Home + editor flow)
```

## Project Structure

| Path | Role |
| --- | --- |
| `crates/flintshot-core/src/session.rs` | **New.** `SessionStore`: in-memory index + temp-dir persistence (`insert` / `update_anns` / `remove` / `list` / `clear` / `sweep_stale`). Pure logic, injected dir, no GUI types. |
| `crates/flintshot-core/src/lib.rs` | Re-export `session` types. |
| `crates/flintshot-app/src/app_window.rs` | Own one `SessionStore` per app-window process (in/next to `Shared`). Wire: insert-on-arrival, stash-annotations-on-editor-exit, remove-on-save/discard, `clear()` on graceful exit, `sweep_stale()` on startup. |
| `crates/flintshot-editor-egui/src/home.rs` | "This session" section: cards with on-demand thumbnails, click-to-resume, hover Discard chip (two-step confirm, matching recent-captures-v2's Delete chip). |
| `crates/flintshot-editor-egui/src/window.rs` | Resume routing: session card → `Route::Editor` with a `PendingEditor` reconstituted from the stored image + annotations. |
| `docs/specs/session-captures/` | This spec, then `plan.md` / `tasks/` in later phases. |

Out of scope (explicitly): standalone worker editors (`flintshot capture`
/ `open --standalone`) — they have no Home route; the daemon — it never
touches the store; persistence across app restarts.

## Data model & persistence

### Core types (`session.rs`)

```rust
/// Stable identity of one session capture within the process lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(u64); // monotonic within the process

/// One resumable capture.
#[derive(Debug, Clone)]
pub struct SessionCapture {
    pub id: SessionId,
    /// Unix-seconds capture arrival time (card captions).
    pub created_at: u64,
    /// Source screenshot (never mutated by annotations).
    pub image: image::RgbaImage,
    /// Committed annotations, in draw order.
    pub anns: Vec<Annotation>,
}
```

### On-disk layout

```
$XDG_RUNTIME_DIR/flintshot/session/     (fallback: std::env::temp_dir()/flintshot/session/)
  <id>.png    # source image, written once on arrival
  <id>.json   # { "created-at": …, "anns": […] }, rewritten on every commit
```

- PNG encode happens **off the UI thread** (arrival is not latency-
  critical); the small annotations JSON is rewritten synchronously on
  every committed change — commits are discrete user actions and the
  payload is tiny, and this makes annotations crash-safe for free.
- **Advisory, like recents**: any IO failure logs a warning and
  degrades to in-memory-only for that entry — never a user-facing
  error, never a panic.
- Resume path: decode PNG + parse JSON → `EditorState::new(image)` with
  `anns` restored; undo/redo start empty (per decision).
- Single app window is guaranteed (`single.rs`), so a fixed dir is safe
  against concurrent writers.

### Lifecycle

| Event | Store effect |
| --- | --- |
| Capture lands in the editor epoch | `insert(image)` → PNG written (background), card becomes listable |
| Annotation committed while editing | `update_anns(id, anns)` → JSON rewritten |
| Navigate Home / Settings / superseded | Nothing extra — annotations already flushed per-commit |
| Save in editor | `remove(id)` — the capture graduates to `recents.toml` as today |
| Upload initiated (editor exits) | **Keep** the card — no disk file is guaranteed, so removal could lose the only copy |
| Esc / Cancelled / window close while editor open | **Keep** the card — only the Home chip's explicit two-step discard destroys |
| Discard on Home card | `remove(id)` — files deleted |
| App-window graceful exit | `clear()` — whole dir removed |
| App-window crash | Dir lingers; next startup `sweep_stale()` deletes it; **no resume across restarts** |

### Behavior change to document

The Replace policy's annotation loss is gone: `app_window.rs`'s
"hidden_close_restores_the_page" test family and the ADR note backing it
must be updated — restore now yields image **plus annotations** when the
capture is still in the session store (the bare pre-epoch stash remains
only as the fallback for store misses).

## Code Style

Existing repo conventions apply (`AGENTS.md` + `.opencode/tech/rust/`):
`#![forbid(unsafe_code)]`, pedantic clippy, `#[non_exhaustive]` +
`#[serde(rename_all = "kebab-case")]` on persisted types, `thiserror`
in core, doc comments on all public core items, no comments restating
code. Illustrative shape:

```rust
impl SessionStore {
    /// Track a newly landed capture. The PNG write is deferred to the
    /// caller's background thread; the in-memory entry is listable
    /// immediately so Home never waits on disk.
    pub fn insert(&self, image: image::RgbaImage, now: u64) -> SessionId;

    /// Rewrite the committed-annotation sidecar. Small payload,
    /// synchronous by design: crash-safe annotations for free.
    pub fn update_anns(&self, id: SessionId, anns: &[Annotation]);

    /// Drop one capture (save or discard): memory entry + both files.
    /// Missing files are not an error (advisory).
    pub fn remove(&self, id: SessionId);
}
```

## Testing Strategy

- **Core unit tests** (`session.rs` `#[cfg(test)]` + `tempfile`):
  insert/list ordering (most recent first), update/remove round-trips,
  corrupt `<id>.json` → entry survives with empty annotations (advisory
  degrade), `sweep_stale` clears a fake leftover dir, `clear` empties
  everything, fallback dir when `XDG_RUNTIME_DIR` is unset.
- **App-window tests** (`app_window.rs` test module): the existing
  restore-page test family gains an annotations-preserved round-trip
  (capture → annotate → Home → resume → editor state carries `anns`);
  save and discard remove the store entry.
- **egui tests** (`home.rs` / `window.rs` patterns): session section
  renders cards from the bridge, resume click routes the editor, discard
  chip is two-step.
- **Gates**: the four commands above, plus the
  `no_gui_types_in_core.rs` boundary test stays green.

## Boundaries

- **Always**: keep the store advisory (IO failures degrade, never
  fatal); PNG encodes off the UI thread; no GUI types in
  `flintshot-core`; run the full verification command set before done;
  update `CHANGELOG.md` under `## [Unreleased]`.
- **Ask first**: any new dependency (none anticipated); any config
  schema addition (v1 ships no knobs); touching the Replace policy's
  supersede *timing* (only its loss semantics change here); changing
  `recents.toml`'s schema.
- **Never**: persist session captures across app restarts or resume
  from a previous process's leftovers; write outside the runtime/temp
  dir; block the UI thread on image encode/decode of large captures;
  `unwrap()`/`expect()` outside tests; `#[allow(...)]` without a
  justifying comment.

## Success Criteria

1. Capture → annotate → Home: a "This session" card exists with a live
   thumbnail; clicking it reopens the editor with all committed
   annotations intact.
2. Undo/redo history does not survive resume (by design); annotations do.
3. Mid-edit supersede (new capture while editing): previous edits are
   stashed as a card, not lost — both captures coexist on Home.
4. Saving moves the capture to recents and removes the session card;
   discarding (editor or card chip) removes it everywhere.
5. Graceful app exit leaves no files in the session dir; a crashed run's
   leftovers are swept on next startup without resuming.
6. `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --
   D warnings`, and both `cargo test` invocations pass.

## Open Questions

1. Should the session section cap its card count visually (scroll, like
   the recents grid)? — Assume yes, same scroll behavior.
2. Flush cadence for annotations: per-commit (spec'd) vs debounced —
   revisit only if profiles show churn (e.g. rapid freehand commits).
3. Card caption contents — mirror recent-captures-v2's
   `dimensions · age` plus an "unsaved" affordance; exact wording at
   implementation time.
4. Multi-monitor DPI: thumbnails downscale like recents; no extra work
   anticipated.
