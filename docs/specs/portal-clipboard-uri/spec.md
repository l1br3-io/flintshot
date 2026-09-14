---
status: active
started: 2026-09-15
---

# Spec: portal `clipboard:///` grab is read back, not rejected

## Objective

xdg-desktop-portal-cosmic's interactive screenshot UI offers a
**Copy** confirm button. When the user picks it, the portal copies
the PNG to the system clipboard itself and answers the `Screenshot`
request with the pseudo-URI `clipboard:///` — no file exists. Every
post-grab action in flintshot then dies at
`parse_portal_uri` (`crates/flintshot-core/src/capture/portal.rs`):

```
capture error: portal returned a non-file uri: clipboard:///
```

Observed in the wild: three consecutive app-window captures on a
COSMIC session failed this way (daemon.log, 2026-09-14/15), each
invisibly — the error never reaches a notification or window, so
from the user's seat the capture just "doesn't refresh". This
blocks any workflow where the user confirms with Copy, including
the `after_grab = clipboard` flow the `clipboard-opens-editor`
effort just shipped.

Fix: treat `clipboard:///` as a successful grab delivered via the
system clipboard. The portal process (long-lived) still serves the
selection, so flintshot reads the pixels back with arboard and
continues the normal post-grab dispatch — every arm (`--out`,
editor, clipboard, upload) gets its image.

User story: *As a COSMIC user who confirms the OS screenshot UI
with Copy, I want flintshot to accept the grab, so my capture
completes exactly as it does when I confirm with Save.*

### Acceptance criteria

1. `parse_portal_uri("clipboard:///")` (via the portal capture
   path) is distinguishable from real errors: the portal backend
   returns a marker-bearing `FlintshotError::Capture` whose detail
   contains an exported canonical constant — not a bare
   "non-file uri" error.
2. `native_grab_prepared` detects the marker, then waits out the
   portal's Response-before-write race and reads the image back
   from the system clipboard (`arboard`): a pre-grab digest
   snapshot distinguishes the portal's write from an image already
   on the clipboard, the wait budget is ~5 s (observed >1 s from
   Response to readable selection on COSMIC — the write is queued
   behind the overlay teardown), and an image byte-identical to
   the snapshot at the deadline is accepted (two identical
   screenshots must not read as "never landed"). The normal
   dispatch (copy/notify/editor, `--out`, upload) proceeds
   unchanged.
3. A read-back failure (clipboard empty, wrong-size payload,
   arboard error) is a hard error with clear context — never a
   silently wrong image.
4. Every other URI form behaves exactly as before: `file://`
   (3- or 4-slash) parses, empty path and foreign schemes still
   error, cancellation/timeout still quiet-exits.
5. The fix lives at the grab seam; no `PostGrabAction` arm
   changes and no fallback (xcap) path changes.

## Technologies

No new **direct** dependencies. `arboard` is already a workspace
dep of `flintshot-app` (and `flintshot-editor-egui`); it is **not**
added to `flintshot-core` — the read-back happens in the app
layer. (Task 004 enables arboard's `wayland-data-control` feature —
without it arboard is X11-only and rides XWayland's lazy
clipboard bridge on Wayland sessions — which pulls the
`wl-clipboard-rs` transitive dep, pure Rust, no system libs.)

## Commands

```sh
cargo build
cargo test --workspace --all-targets
cargo test -p flintshot-core portal       # parse_portal_uri tests
cargo test -p flintshot-app clipboard     # classification tests
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -- capture                      # manual: confirm OS UI with Copy
```

## Project Structure

```
crates/flintshot-core/src/capture/portal.rs   # marker const + parse branch
crates/flintshot-app/src/cli.rs               # detection + arboard read-back
CHANGELOG.md                                  # [Unreleased] Fixed entry
docs/specs/portal-clipboard-uri/              # this spec
```

## Code Style

- Marker: `pub const PORTAL_GRAB_ON_CLIPBOARD: &str` in
  `capture::portal`, doc-commented (`#![warn(missing_docs)]`),
  used by both producer (detail formatting) and consumer
  (`detail.contains(...)`) so the string exists exactly once.
- Detection fn in `cli.rs` sits next to `quiet_capture_exit`
  (same classification style, same test shape).
- Read-back converts `arboard::ImageData` to `RgbaImage` via
  `from_raw`, mapping `None` (bad dimensions) to a capture error.

## Testing Strategy

Pure seams get tests; I/O does not (repo precedent — the dispatch
body is not headless-testable):

- `portal.rs` unit test: `clipboard:///` yields an error whose
  detail contains `PORTAL_GRAB_ON_CLIPBOARD`; `file://` forms,
  empty path, and foreign schemes unchanged.
- `cli.rs` unit test: the classifier matches the marker detail and
  rejects near-misses (plain "non-file uri", "cancelled",
  decode-failure strings) — mirroring
  `quiet_capture_exit_classifies_dismissal_and_timeout_only`.
- The arboard read-back itself is verified by the manual COSMIC
  check (portal serves the selection; no headless equivalent).

## Boundaries

- **Always:** finish gate (fmt, clippy, workspace tests, doc
  tests); `CHANGELOG.md` under `## [Unreleased]`.
- **Ask first:** changing `Capturer`'s trait shape; adding arboard
  to `flintshot-core`; touching the fallback path; new
  dependencies.
- **Never:** loosen the hard-error rule for read-back failures;
  special-case post-grab arms (the marker is resolved before
  dispatch); edit the daemon protocol.

## Success Criteria

- [ ] On a COSMIC session, `flintshot capture` confirmed with the
      OS UI's **Copy** button completes: pixels flow to the
      configured post-grab action (the clipboard write observed
      landing within the wait window), no error in daemon.log.
- [ ] Confirmed with **Save/Enter** still behaves as before.
- [ ] `cargo fmt --all --check`,
      `cargo clippy --workspace --all-targets -- -D warnings`,
      `cargo test --workspace --all-targets`,
      `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
      pass.
- [ ] `CHANGELOG.md` updated under `## [Unreleased]`.

## Open Questions

None — resolved at spec time with the user: marker through the
existing error channel (not a trait-shape change), read-back in
the app layer (not arboard-in-core), tracked as its own effort.
