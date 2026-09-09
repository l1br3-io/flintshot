# Task 001: IPC schema variants

**Description:** Append the new wire variants to `Request`/`Event` in
`flintshot-core/src/ipc.rs` per the spec, with doc comments in the
house style (postcard variant-index rule — append at the end). No
behavior change anywhere yet; the daemon treats unsent variants as
today's `_` arm does (ReportAppHidden will hit "unsupported" until
Task 005 intercepts it — nothing sends it yet, so this is inert).

New `Request` variants (appended, after `OpenApp`):
- `PrepareCapture` — worker → daemon, pre-grab request/reply. Daemon
  clears all flintshot surfaces (superseded sessions already gone,
  app window, pins, tray) and replies `Event::CaptureReady` only
  when the screen is clear or bounds expire. Supersedes (and
  internally includes) `HideTrayForCapture`.
- `ReportAppHidden` — app window → daemon, best-effort: "my window
  is destroyed". Pairs with `Event::HideForCapture`.
- `ReportEditorWindowOpened` — worker → daemon, fire-and-forget
  (mirrors `ReportSaved`): an editor/overlay window is about to
  run. The daemon broadcasts `Event::EditorOpened` on receipt.

New `Event` variants (appended, after `AppFocus`):
- `CaptureSuperseded` — a new capture replaced the live session;
  listening workers close gracefully.
- `CaptureReady` — reply-only to `PrepareCapture`: grab now. Never
  broadcast.
- `HideForCapture` — broadcast to the app window: close and ack
  `ReportAppHidden`.

**Acceptance criteria:**
- [ ] All six variants appended at the end of their enums with
      `#[non_exhaustive]` preserved; postcard round-trip tests
      extended and passing
- [ ] `worker.rs::variant_name` covers the three new requests (its
      covers-every-variant test updated)
- [ ] No other behavior change; full workspace gates green

**Verification:**
- [ ] `cargo test -p flintshot-core`
- [ ] `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-targets`

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-core/src/ipc.rs`
- `crates/flintshot-app/src/worker.rs` (variant_name + its test)

**Docs:** None — schema-only; the ADR lands in Task 009.

**Estimated scope:** Small: 2 files
