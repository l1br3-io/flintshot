# ADR-0057: Additive settings schema with live apply

## Context

The ux-redesign settings route needed real plumbing for
`capture.default-mode`, `ui.show-statusbar`, `ui.statusbar-size-mode`,
`ui.animations`, and `output.*` (format, JPEG quality, filename
template, save dir). Existing user TOMLs predate every field, and the
config watcher reloads the app's config on every write.

## Decision

1. **Additive, serde-defaulted, kebab-case.** Every new field carries
   a `#[serde(default = …)]`; missing sections deserialize to
   defaults, so old TOMLs load byte-for-byte and re-save gains the
   new keys without user action.
2. **String-tolerant enums.** Vocabulary fields
   (`default-mode`, `format`, `statusbar-size-mode`) store strings
   and resolve through `parse_*` functions that warn and fall back to
   the default on unknown values — matching the `backend` /
   `interaction-mode` precedent so a typo never bricks loading.
3. **Route owns a working copy; the window drains saves.** The
   settings page edits an in-memory `Config` and persists through the
   window's bridge at most once per debounce window (1 s), keeping
   the watcher's reload from machine-gunning. The final write always
   lands.
4. **Consumers read a snapshot; the router live-applies.** Editor
   windows snapshot the config at construction (workers have no
   watcher); the router pushes the saved config into an open routed
   editor via `apply_config`, so statusbar / animation toggles apply
   without a restart.
5. **Direct save is opt-in by template.** When
   `output.filename-template` is non-empty, Save writes
   `save-dir/rendered.ext` (format + JPEG quality honored, `-2`
   collision suffixes); empty template keeps the file dialog.
   `{app}` / `{date}` / `{time}` substitute; unknown placeholders
   pass through; the rendered name is sanitized to one path
   component.

## Consequences

- Round-trip and old-config tests pin the loading contract; the
  token-only scan is unaffected (no GUI types in core).
- The magnifier keys stay read-once-per-editor-construction
  (their precedent), so settings edits to them land next session —
  documented in `apply_config`.
- The statusbar's exact-PNG estimate is cached per image dims; the
  encode cost never lands per frame.
