---
status: active
started: 2026-08-11
---

# Spec — Phase 4: Upload

> Draft effort spec. For product context, see
> [`../../VISION.md`](../../VISION.md); for sequencing, see
> [`../../ROADMAP.md`](../../ROADMAP.md); for the technical substrate
> this builds on, see [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md)
> (especially §Process model, §IPC protocol, §Upload `Backend`
> trait). The shipped phases this effort stands on:
> [Phase 1](../phase-1-capture-editor/spec.md) (capture + editor),
> [Phase 2](../phase-2-daemon-tray-hotkeys/spec.md) (daemon, tray,
> hotkeys, IPC), [Phase 2b](../phase-2b-annotation-tools/spec.md)
> (annotation tools + magnifier), [Phase 3](../phase-3-pin-cosmic/spec.md)
> (pin-to-desktop + COSMIC applet, which delivered the
> `Request::ReportSaved` worker→daemon channel Phase 4 depends on).

## Objective

Make `flintshot` able to *share* a capture without leaving the tool.
After Phases 1–3, a screenshot flows: daemon spawns worker → editor
opens → user annotates → Save/Copy/Pin → done. The one Flameshot-grade
output still missing is **upload** — get a public URL for the capture
so it can be pasted into chat, a bug report, or a README. This phase
lands that:

1. **Imgur backend** — the first concrete `UploadBackend` impl. Imgur
   is the de-facto image host for screenshot tools (Flameshot, ShareX,
   Spectacle all default to it), has a stable API, and supports both
   anonymous uploads (zero-config, rate-limited) and Client-ID
   authenticated uploads (lifts rate limits, enables ownership).
2. **Three entry points** — the editor toolbar, the `flintshot upload`
   CLI, and the daemon's existing `Request::UploadLast` (reachable
   from tray / COSMIC applet / IPC scripts). All three converge on the
   same worker process.
3. **Real credential storage** — `keyring` (OS keychain) for the
   Client-ID, never plaintext config. Anonymous uploads need no
   credential at all.
4. **Closing the loop** — URL → clipboard + desktop notification + IPC
   event broadcast, so subscribers (tray, COSMIC applet) reflect
   "Uploaded https://…" without polling.

**Target users:** the same power users from
[`VISION.md`](../../VISION.md) §Target users. The "I took a screenshot,
now I need to share it in Slack" workflow is the single most-cited
Flameshot use-case after pin-to-desktop; this phase makes `flintshot`
a viable Flameshot replacement for that workflow.

**Success looks like:**

- A user captures a region, annotates, clicks **Upload** in the editor
  toolbar. The editor closes, a desktop notification appears
  ("Uploaded to https://imgur.com/…"), and the URL is already on their
  clipboard. They paste into chat. Done.
- From a terminal, `flintshot upload /tmp/shot.png` prints the URL to
  stdout (machine-parseable, pipeable) and exits 0. A network failure
  prints a clear error to stderr and exits non-zero.
- From the COSMIC panel applet, the user clicks *Upload last* → the
  popup shows *"Uploading… → Uploaded https://imgur.com/…"*. They never
  opened a terminal.
- A rate-limited user runs `flintshot auth imgur` once, pastes their
  Client-ID, and subsequent uploads use the authenticated path
  automatically (no per-invocation flag).

### Why this scope, and what's explicitly out

ROADMAP names this "Phase 4 — Upload" as the last pre-1.0 milestone
(`ROADMAP.md:30`: "1.0.0 once Phase 4 (upload) lands and we have at
least 30 days of dogfooding"). The scaffolding is already in place:

- `UploadBackend` trait + `ImageMime` + `UploadContext` +
  `UploadOutcome` + `UploadError` declared in Phase 0
  (`crates/flintshot-core/src/upload.rs`).
- `Request::UploadLast { backend }` and `Event::Uploaded { url,
  delete_handle }` in the IPC schema
  (`crates/flintshot-core/src/ipc.rs:71,115`).
- `EditorOutcome::Uploaded(String)` variant on the shell trait
  (`crates/flintshot-core/src/shell.rs:19`).
- `flintshot upload <path>` CLI subcommand stub with arg parsing
  (`crates/flintshot-app/src/cli.rs:593`).
- `Request::ReportSaved` worker→daemon channel (landed in Phase 3,
  `crates/flintshot-app/src/cli.rs:352`) — the daemon already tracks
  the last-saved path, so `UploadLast` resolves cleanly.

But the trait currently uses **placeholder types** for credentials and
progress (`crates/flintshot-core/src/upload.rs:40-64`):

```rust
pub struct UploadContext<'a> {
    pub credentials: CredentialHandle<'a>,  // stub: only carries a backend name
    pub filename: Option<&'a str>,
    pub progress: ProgressSink<'a>,          // stub: wraps &'a mut f32
}
```

Phase 4 swaps those stubs for real `keyring::Entry` references and
drops the `progress` field entirely (see Decisions §2). This is a
breaking change to a `0.0.0`-versioned public type — the workspace
pre-1.0 contract allows it; consumers (`flintshot-app`) update in the
same PR.

**Out of scope** — kept tight so Phase 4 lands as the clean 1.0 capstone:

- **Other backends** (0x0.st, SFTP, custom HTTP, local file copy) →
  post-1.0 stretch goals (`ROADMAP.md:41`). The trait stays
  extension-friendly per ADR-0004.
- **Imgur OAuth user-login flow** (full three-legged OAuth with refresh
  tokens) → overkill for v1; Client-ID auth is enough for the
  "registered app" rate-limit lift.
- **Imgur album support, gallery submission, account queries** → not
  needed for the screenshot-share use-case.
- **Upload history UI** (list of past uploads with re-copy URL /
  delete) → a separate effort once there's a config/history store.
- **CLI `delete` subcommand** → deferred; the delete-handle is captured
  in `Event::Uploaded` so users/scripts can act on it later. The
  `UploadBackend::delete` trait method stays declared-but-unused.
- **Live overlay** (Flameshot-style fullscreen draw) → Phase 5.
- **Wayland global hotkeys via `ashpd`** → Phase 6.
- **macOS/Windows packaging** (MSI / notarized `.app`) → post-1.0;
  upload *works* cross-platform, signing/notarization is packaging work.

## Scope (in / out)

### In

- **Trait rework** (`flintshot-core::upload`) — swap the placeholder
  `CredentialHandle` / `ProgressSink` stubs for real types. Target
  shape lives in `ARCHITECTURE.md` §Upload `Backend` trait. The trait
  keeps its `Send + Sync + 'static` shape so backends can live behind
  `Arc<dyn UploadBackend>` in the daemon. The `progress` field is
  dropped and `ProgressSink` removed (Decisions §2).
- **`ImgurBackend`** — first concrete `UploadBackend` impl. Supports
  two auth modes:
  1. **Anonymous** (zero-config default): POST image to
     `https://api.imgur.com/3/image` with no auth header. Rate-limited
     (~50 uploads/hr per IP). Returns a delete-hash.
  2. **Client-ID authenticated** (opt-in): same endpoint with
     `Authorization: Client-ID <id>`. Lifts rate limits; delete-handle
     still returned. Client-ID is read from `keyring` (entry name
     `flintshot:imgur:client-id`); if absent, falls back to anonymous
     with a `tracing::info!` log.
  - Lives in `flintshot-app::upload::imgur` (worker-side, keeps
    `reqwest` out of `flintshot-core`'s dep graph). See Decisions §1.
- **`flintshot upload <path>` CLI body** (`cli.rs:run_upload`) —
   replaces the current stub (`cli.rs:593`). Loads the image, sniffs
   the format via `image::guess_format` and sends as-is (no `--mime`
   flag; Decisions §4),
  resolves the backend from `--backend` or `[upload] default-backend`,
  constructs the `UploadContext`, runs the backend's `upload()` future
  on a tokio runtime, prints the resulting URL to **stdout** (last
  line, machine-parseable), logs progress to stderr, exits 0 on
  success / non-zero on error.
- **Daemon `Request::UploadLast { backend }` dispatch** (`worker.rs`) —
  replaces the current `Event::Error { code: "unsupported" }` arm
  (`worker.rs:216`). Resolves the last-saved path from the daemon's
  `ReportSaved`-fed state (already tracked since Phase 3), spawns
  `flintshot upload <last-path> --backend <backend>` as a worker
  process, emits `Event::UploadStarted { backend }` optimistically
  (mirrors the Phase 2 `CaptureStarted` pattern).
- **Worker→daemon result reporting for uploads** — extends ADR-0023's
  "worker reports back" pattern: after upload completes, the worker
  connects to the daemon socket and sends the resulting URL +
  delete-handle so the daemon can emit `Event::Uploaded` to
  subscribers. Mechanism parallels `Request::ReportSaved { path }`; a
  new `Request::ReportUploaded { url, delete_handle }` variant is the
  natural fit (Decisions §3).
- **`Event::UploadStarted { backend }`** — new `Event` variant emitted
  on `UploadLast` dispatch (optimistic, before spawn completes).
  Mirrors Phase 2's `CaptureStarted` shape. Picked over percentage
  progress per the spec-confirmation interview.
- **Editor **Upload** toolbar button** (`flintshot-editor-egui`) —
  composes the final image via the existing `compose_final`, writes it
  to a temp file, spawns `flintshot upload <tmpfile>` as a detached
  child, closes the editor with `EditorOutcome::Uploaded(url)`. The
  spawned worker reports back to the daemon (so subscribers see the
  URL); the editor itself shows a one-shot toast/notification on
  spawn-confirmation, then closes.
- **`flintshot auth imgur` subcommand** — new CLI subcommand to set
  the Client-ID via keyring. Prompts for the ID (hidden input), writes
  it to `keyring::Entry::new("flintshot", "imgur:client-id")`, exits 0
  with a confirmation. `flintshot auth imgur --clear` removes the
  entry (reverts to anonymous). This is the **only** credential-entry
  path; no `[upload.imgur] client-id` config field (secrets-stay-
  out-of-config boundary).
- **Desktop notifications on completion / error** (`notify-rust`) —
  "Uploaded to <url>" on success, "Upload failed: <message>" on error.
  Named in `ARCHITECTURE.md` tech stack; wired into the upload worker.
- **`[upload]` config section** (`flintshot-core::config`) — adds
  `[upload]` with `default-backend = "imgur"` (the only valid value
  for Phase 4; `#[non_exhaustive]`-shaped for future backends). No
  per-backend sub-tables — Client-ID lives in keyring.
- **URL → clipboard on success** — the worker places the resulting
  public URL on the clipboard (text) via `arboard`, in addition to
  printing to stdout. Matches Flameshot's UX; user can immediately
  paste.

### Out (deferred)

- Other backends (0x0, SFTP, custom) → post-1.0.
- Imgur OAuth (user login) → post-1.0.
- Imgur album / gallery → post-1.0.
- Upload history UI → separate effort.
- CLI `delete` subcommand → trait method declared, unused in Phase 4.
- Live overlay (Phase 5), Wayland global hotkeys (Phase 6).
- macOS/Windows installer packaging → post-1.0.

## Deliverables

- [ ] `UploadContext` stub types replaced with real `keyring::Entry`
      reference; `progress` field dropped and `ProgressSink` removed
      (Decisions §2).
      All `flintshot-app` callers updated in the same PR.
      `no_gui_types_in_core.rs` stays green — `keyring::Entry` is not a
      GUI type (it's a secrets handle), so it may cross the core
      boundary.
- [ ] `ImgurBackend` impl (`flintshot-app::upload::imgur` by default)
      — supports anonymous + Client-ID modes; `id()` returns
      `"imgur"`, `name()` returns `"Imgur"`; `upload()` POSTs to
      `https://api.imgur.com/3/image` with optional
      `Authorization: Client-ID <id>` header; parses the response into
      `UploadOutcome { url, delete_handle }`; maps HTTP / network /
      auth failures to the four `UploadError` variants.
- [ ] `flintshot upload <path>` CLI body — loads image, resolves
      backend, runs upload, prints URL to stdout, exits 0 / non-zero.
      `--backend NAME` overrides config default. Headless (no GUI dep).
- [ ] Daemon `Request::UploadLast { backend }` dispatch resolves
      last-saved path, spawns `flintshot upload`, emits
      `Event::UploadStarted { backend }` optimistically, emits
      `Event::Uploaded { url, delete_handle }` on worker report-back.
      Worker→daemon report channel added (parallels `ReportSaved`).
- [ ] `Event::UploadStarted { backend: String }` variant added to the
      IPC schema (`flintshot-core::ipc`); postcard round-trip test
      extended (mirror the existing `Event::Uploaded` test).
- [ ] Editor **Upload** button (`flintshot-editor-egui`) — composes
      final, spawns `flintshot upload <tmpfile>`, closes editor with
      `EditorOutcome::Uploaded(url)`.
- [ ] `flintshot auth imgur [--clear]` subcommand — writes/removes the
      keyring entry; never writes to config.
- [ ] `[upload]` config section wired into `Config::load` / `Config::save`
      (comment-preserving, like `[pin]` in Phase 3).
- [ ] `notify-rust` wired into the upload worker — desktop notification
      on success and on error.
- [ ] URL → clipboard on success via `arboard` (text content; image
      content stays with the existing Copy path).
- [ ] `no_gui_types_in_core.rs` extended (if needed) to permit
      `keyring::Entry` while still rejecting `egui`/`cosmic`/etc.
- [ ] `CHANGELOG.md` `[Unreleased]` entry for upload.
- [ ] ADRs for: (a) the trait-rework decision (why `keyring::Entry`
      and not a trait-object credential handle), (b) the
      worker-reports-upload-result mechanism (extends ADR-0023), (c)
      the `reqwest` choice (extends the tech-stack ADRs).
- [ ] `docs/TEST_MATRIX.md` rows for: editor Upload button, `flintshot
      upload` CLI (success / no-such-file / network-error / anonymous
      / authenticated), daemon `UploadLast` dispatch, `flintshot auth`
      set / clear.

## Exit criteria

A user, in a single session with `flintshot daemon` running:

1. Runs `flintshot region`, annotates, clicks **Upload** in the editor
   toolbar. The editor closes; within seconds a desktop notification
   appears ("Uploaded to https://imgur.com/…"); the URL is on the
   clipboard. They paste into a chat client.
2. From the COSMIC panel `flintshot` button, clicks *Upload last* (or
   a tray menu item); the popup shows *"Uploading… → Uploaded
   https://imgur.com/…"*.
3. From a terminal, runs `flintshot upload /tmp/shot.png` and sees the
   URL printed to stdout (last line). Pipes it: `flintshot upload
   /tmp/shot.png | tail -1` yields the bare URL.
4. Hits the anonymous rate limit (simulated: rate-limit response from
   a mock Imgur), runs `flintshot auth imgur`, pastes their Client-ID,
   re-runs the upload — it succeeds via the authenticated path.
5. Triggers a network failure (e.g. `nmcli networking off` mid-upload)
   — the worker prints a clear error to stderr, the desktop
   notification reads "Upload failed: …", the daemon emits
   `Event::Error { code: "upload-failed", message }` to subscribers,
   the CLI exits non-zero. No panic, no hang.
6. Runs `flintshot auth imgur --clear`; subsequent uploads revert to
   anonymous (visible via `tracing` log).

The abstraction holds: the `UploadBackend` trait stays
toolkit-agnostic; `ImgurBackend` is the only concrete impl and lives
outside `flintshot-core` (default); `no_gui_types_in_core.rs` stays
green.

## Technologies

Adds three crates to the workspace dep set. All named in
`ARCHITECTURE.md` §Tech stack; none surprise the existing direction.

| Concern              | Crate                              | Status                              |
| -------------------- | ---------------------------------- | ----------------------------------- |
| Upload trait + types | `flintshot-core::upload`           | Existing (Phase 0); **reworked**    |
| HTTP client          | `reqwest` (default features +      | **New workspace dep**. Tokio-native |
|                      | `multipart`, `json`)               |                                     |
| Secrets storage      | `keyring` (≥ 3)                    | **New workspace dep**. OS keychain  |
| Notifications        | `notify-rust` (≥ 4)                | **New workspace dep**. Cross-plat   |
| Clipboard (URL copy) | `arboard` (already a workspace dep) | Reused from editor Copy path        |
| Async runtime        | `tokio` (already a workspace dep)  | Reused                              |
| Temp file (editor→   | `tempfile` (already a workspace    | Promoted from dev-dep to dep in     |
| upload handoff)      | dep; promote to dep)               | `flintshot-app` (pre-approved)      |
| Errors               | `thiserror` (core) / `color_eyre`  | Existing convention                 |
| Imgur API            | `https://api.imgur.com/3/image`    | REST; no SDK                        |

`reqwest`, `keyring`, `notify-rust`, and `tempfile` move into
`[workspace.dependencies]` in the root `Cargo.toml`; `flintshot-app`
pulls them via `{ workspace = true }`. `keyring` also becomes a
`flintshot-core` dep because `UploadContext` references
`keyring::Entry` as a parameter type (Decisions §1).

## Commands

All run from the repo root. Full reference in
[`AGENTS.md`](../../../AGENTS.md) §Commands; the standing gate is the
source of truth.

```sh
# Build (default-members: flintshot-app + deps)
cargo build

# Phase 4 entry points
cargo run -- region                       # capture → editor → Upload button
cargo run -- fullscreen
cargo run -- upload /tmp/shot.png         # direct upload, prints URL
cargo run -- upload /tmp/shot.png --backend imgur
cargo run -- auth imgur                   # set Client-ID (hidden input)
cargo run -- auth imgur --clear           # remove Client-ID

# Gate — run in order before declaring done
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
```

Upload network tests need either network access (smoke, `#[ignore]`'d)
or a mock HTTP endpoint. Headless CI exercises the pure-core surface
(ImageMime, config round-trip, IPC schema round-trip, error mapping
from a fake `reqwest::Client` via a trait seam) and the dispatch table
via `RecordingSpawner`. A `mockito`-backed `#[ignore]`'d integration
test covers the real Imgur response shape against a recorded fixture
(no real network in CI).

## Project structure

No new crates, no workspace-shape changes. Touched files:

```
crates/flintshot-core/src/
├── upload.rs          # REWORK — swap stub types; keep trait shape
├── config.rs          # EXTEND — new [upload] section (UploadConfig)
├── ipc.rs             # EXTEND — Event::UploadStarted; Request::ReportUploaded
└── lib.rs             # re-export UploadConfig; doc-test surface updates

crates/flintshot-app/src/
├── cli.rs             # EXTEND — real run_upload; new run_auth; AuthArgs
├── worker.rs          # EXTEND — UploadLast dispatch; ReportUploaded arm
├── daemon.rs          # EXTEND — track upload-in-flight; emit on report
├── ipc_server.rs      # EXTEND — handle ReportUploaded (mirror ReportSaved)
└── upload/            # NEW module
    ├── mod.rs         # backend registry (id → Arc<dyn UploadBackend>)
    └── imgur.rs       # NEW — ImgurBackend impl (reqwest + keyring)

crates/flintshot-editor-egui/src/
└── shell.rs (or toolbar.rs)  # EXTEND — Upload button → spawn → close

crates/flintshot-core/tests/
└── no_gui_types_in_core.rs   # UNCHANGED but must stay green;
                              # `keyring` is not a GUI toolkit — allowed
```

Rationale: the trait + types live where they've always been
(`flintshot-core::upload`). The concrete `ImgurBackend` impl lives
worker-side (`flintshot-app::upload::imgur`) so `flintshot-core`
doesn't pull `reqwest`. `keyring` becomes a core dep (justified by
being a parameter type on `UploadContext`, not a GUI type; see
Decisions §1).

## Code style

Unchanged from [`AGENTS.md`](../../../AGENTS.md) §Conventions and the
`.opencode/tech/rust/` docs (loaded at `/build` time). Two shapes
worth previewing — the surfaces this effort adds:

**Reworked `UploadContext`** (illustrative; final shape in Decisions
§1–2):

```rust
// crates/flintshot-core/src/upload.rs
pub struct UploadContext<'a> {
    /// OS keychain entry the backend reads credentials from. `None`
    /// for anonymous mode.
    pub credentials: Option<&'a keyring::Entry>,
    /// Optional filename hint sent to the backend.
    pub filename: Option<&'a str>,
}
// `progress` field dropped (Decisions §2); `ProgressSink` type removed.
```

**ImgurBackend dispatch** (illustrative):

```rust
// crates/flintshot-app/src/upload/imgur.rs
pub struct ImgurBackend {
    http: reqwest::Client,
}

impl UploadBackend for ImgurBackend {
    fn id(&self)   -> &'static str { "imgur" }
    fn name(&self) -> &'static str { "Imgur" }

    async fn upload(
        &self,
        image_bytes: &[u8],
        mime: ImageMime,
        ctx: &UploadContext<'_>,
    ) -> Result<UploadOutcome, UploadError> {
        let mut req = self.http
            .post("https://api.imgur.com/3/image")
            .header("Content-Type", format!("multipart/form-data; …"))
            .part("image", Part::bytes(image_bytes.to_vec()));
        if let Some(entry) = ctx.credentials {
            let id = entry.get_password().map_err(|e| UploadError::Auth(e.to_string()))?;
            req = req.header("Authorization", format!("Client-ID {id}"));
        }
        let resp = req.send().await.map_err(|e| UploadError::Transport(e.to_string()))?;
        // … map resp.status() / body → UploadOutcome | UploadError
    }
}
```

Conventions this effort locks in (all already standing): every public
type `#[non_exhaustive]`-safe, doc comments on all public items, no
`unwrap()` / `expect()` outside tests and `const`, comments only when
the "why" is non-obvious, errors via `thiserror` (core) /
`color_eyre` (app), no secrets in plaintext config (Client-ID via
keyring only).

## Testing strategy

Mirrors [`ARCHITECTURE.md`](../../ARCHITECTURE.md) §Testing strategy
and the Phase 2/3 split: heavy headless unit coverage in core + app
dispatch (`RecordingSpawner`); backend integration via a mock HTTP
endpoint (`mockito`); manual cross-platform test matrix for real
network + keychain behavior.

### Unit — `flintshot-core` (headless)

- **`ImageMime::as_str`** — already covered (Phase 0); ensure no
  regression.
- **`UploadContext` shape** — after the rework, the public surface
  (re-)compiles against the new types; doc examples build.
- **`UploadConfig` (de)serialization** — `[upload]` section round-trips
  through `Config::load` / `save`; missing section uses defaults;
  comment-preserving write leaves unrelated sections intact (mirror
  the Phase 3 `PinConfig` tests).
- **IPC roundtrip** — `Event::UploadStarted { backend }` and (if
  added) `Request::ReportUploaded { url, delete_handle }` round-trip
  through postcard; existing `Event::Uploaded` test stays green.

### Unit — `flintshot-app` (headless, no network)

- **Worker dispatch for `UploadLast`** — extend the `RecordingSpawner`
  tests in `worker.rs`: `UploadLast` spawns
  `upload <last-path> --backend <id>` and emits
  `Event::UploadStarted { backend }`; the test asserts the spawned
  argv and the emitted event.
- **`UploadLast` with no last-saved path** — mirror Phase 3's
  `PinLast` test: emits `Event::Error { code: "no-last-capture" }`.
- **`flintshot upload` arg parsing** — `--backend` overrides config;
  absent flag uses `[upload] default-backend`; absent config uses
  built-in default `"imgur"`.
- **`flintshot auth imgur` parsing** — `--clear` flag presence; hidden
  input via `rpassword` (or equivalent); success / failure of keyring
  write does not panic.
- **`ReportUploaded` arm** — daemon emits `Event::Uploaded` on receipt
  of `Request::ReportUploaded` from a worker (mirror the Phase 3
  `ReportSaved` test).
- **`ImgurBackend` response mapping** — inject a fake `reqwest::Client`
  (via a trait seam or `httpmock` / `mockito`) returning canned
  responses (success JSON, 429 rate-limit, 401 auth-error, transport
  failure); assert the backend returns the right `UploadOutcome` /
  `UploadError` variant. No real network.

### Integration — `#[ignore]`'d, opt-in (`--ignored`)

- **Real Imgur anonymous upload** — uploads a 1×1 PNG via the real
  Imgur API; asserts the response URL parses as `https://imgur.com/…`
  (or `https://i.imgur.com/<id>.png`). Skipped in CI (network +
  third-party rate limits); runnable locally with `cargo test --
  --ignored imgur_real`.
- **Real keychain round-trip** — `flintshot auth imgur` set →
  `flintshot upload` (authenticated path) → clear; asserts the
  keychain entry is created, used, removed. Cross-platform; requires
  an unlocked keychain in CI runners (CI skips with `#[ignore]`).

### Boundary

- `no_gui_types_in_core.rs` unchanged and green. `keyring::Entry` is
  not a GUI type (no `use egui` / `use cosmic` / `use libcosmic`); the
  scanner already permits arbitrary non-GUI types. Since the decision
  is a direct `keyring::Entry` reference (Decisions §1), the scanner
  needs no change.

### Manual test matrix

Tracked in [`docs/TEST_MATRIX.md`](../../TEST_MATRIX.md); Phase 4 adds
rows for:

- Editor **Upload** button (compose → spawn → notification → clipboard
  → close).
- `flintshot upload <path>` (success, no-such-file, network-error,
  anonymous rate-limit, authenticated).
- `flintshot auth imgur` (set, re-set, clear, clear-when-absent).
- Daemon `UploadLast` (via tray, via COSMIC applet popup).
- Cross-platform: Linux (X11 + Wayland), macOS (keychain unlock
  prompt), Windows (credential manager).

## Boundaries

- **Always:**
  - Run the four-command gate (`fmt`, `clippy`, `test`, doc tests)
    before declaring done.
  - Store the Imgur Client-ID in the OS keychain via `keyring` — never
    in `config.toml`, never in env vars persisted across invocations.
    The only exception is a one-shot `IMGUR_CLIENT_ID` env var read by
    `flintshot auth imgur` (interactive prompt fallback).
  - Print the upload URL to **stdout** as the last line (machine-
    parseable); all diagnostic output to **stderr**.
  - Exit non-zero on any upload failure (network, auth, rate-limit,
    backend-rejected); exit zero only on a successful URL return.
  - Spawn each upload as its own worker process (`flintshot upload
    <path>`) per ADR-0019 — an upload crash must never kill the
    editor or daemon, and concurrent uploads coexist as independent
    processes.
  - Report the upload result back to the daemon via the IPC channel
    (extends ADR-0023) so subscribers see `Event::Uploaded`; the
    daemon does not perform uploads in-process.
  - Preserve Phase 1/2/2b/3 behavior bit-for-bit (editor, daemon,
    tray, hotkeys, pin, COSMIC applet).
  - Map HTTP failures to the four `UploadError` variants
    (`Transport`, `Auth`, `Backend`, `UnknownBackend`) — no panics on
    any network or response shape.

- **Ask first:**
  - Adding `reqwest`, `keyring`, `notify-rust` as workspace deps (all
    named in `ARCHITECTURE.md` tech stack; the addition is expected,
    but surface the exact versions in `/planning`).
  - Adding `mockito` / `httpmock` as a *non-dev* dependency (should
    be dev-only; flag if a non-test path needs it).
  - Making `keyring` a `flintshot-core` dep (because it appears as a
    parameter type on `UploadContext`). Direct `keyring::Entry`
    reference chosen over a `CredentialSource` trait (Decisions §1).
  - Adding a new `Request` variant for worker→daemon upload-result
    reporting (`Request::ReportUploaded { url, delete_handle }`).
    Parallels the Phase 3 `ReportSaved` precedent (Decisions §3).
  - Adding `Event::UploadStarted` (or any new `Event` variant) —
    postcard schema stays `#[non_exhaustive]` so this is forward-compat
    for older clients, but the round-trip test must be extended.
  - Changing CI config (`.github/workflows/ci.yml`) — e.g. if
    `keyring` needs a running D-Bus / Secret Service on Linux for
    *tests*, surface the system-dep change.
  - New ADRs — draft during `/planning`, flag for review.

- **Never:**
  - Write the Imgur Client-ID (or any credential) to `config.toml`,
    a log file, or an unencrypted file. Keychain only.
  - Print credentials, full HTTP headers, or response bodies to logs
    at `info` level — only at `trace` with the credential fields
    redacted.
  - Introduce `egui` / `libcosmic` / `iced` / `slint` types into
    `flintshot-core`'s public API (boundary test enforces).
  - `unwrap()` / `expect()` outside tests and `const` contexts.
  - Silence a clippy warning with `#[allow]` without a justifying
    comment.
  - Add live-overlay, Wayland-hotkey, or other-backend features under
    this effort's banner — keep the slice honest.
  - Perform uploads in-process inside the daemon — ADR-0019 (worker
    spawn) is non-negotiable; the daemon only spawns and relays.
  - Hard-code a Client-ID in the binary or in any committed file.

## Success criteria

- **Imgur upload works end-to-end from three entry points**: editor
  Upload button, `flintshot upload <path>` CLI, and daemon `UploadLast`
  (tray / COSMIC applet / IPC scripts). All three converge on the same
  worker process and backend impl.
- **Anonymous mode is zero-config** — a fresh `flintshot` install
  completes an upload with no `flintshot auth` and no config edits.
- **Client-ID mode lifts rate limits** — `flintshot auth imgur` once,
  subsequent uploads send the `Authorization: Client-ID <id>` header.
- **URL → clipboard + notification on success** — both fire on every
  successful upload, regardless of entry point.
- **Clear error UX on failure** — desktop notification, `Event::Error`
  to subscribers, non-zero CLI exit, no panic, no hang.
- **Abstraction holds** — `UploadBackend` trait stays toolkit-agnostic
  and `Send + Sync`; `ImgurBackend` is the only impl and lives outside
  core; `no_gui_types_in_core.rs` stays green.
- **Worker→daemon result reporting extended** — `Event::Uploaded` is
  emitted to subscribers on every successful upload, regardless of
  entry point.
- The four-command gate is green on Linux; `cargo build` green on
  macOS and Windows for default-members.

## Decisions

All six open questions from the draft were resolved during spec review.
The recommendations were accepted unanimously; ADR drafts will
formalize them during `/planning`.

1. **`ImgurBackend` lives in `flintshot-app::upload::imgur`; `keyring`
   becomes a `flintshot-core` dep.** The COSMIC applet is a pure IPC
   client and must not link `reqwest`. The credential handle on
   `UploadContext` is `Option<&'a keyring::Entry>` (direct reference,
   not a trait object) — `keyring` is small, stable, and non-GUI, so
   the indirection of a `CredentialSource` trait isn't worth it for one
   concrete credential type.

2. **`UploadContext.progress` is dropped.** The `ProgressSink` stub
   type is removed. `UploadBackend::upload` resolves to the outcome;
   callers wrap timing externally. The trait is `0.0.0`-versioned and
   can re-add a progress channel when a real consumer (progress-bar UI)
   appears. YAGNI wins.

3. **Worker→daemon upload-result reporting uses a new
   `Request::ReportUploaded { url, delete_handle }` variant.** Matches
   the Phase 3 `ReportSaved` precedent exactly; keeps the schema
   statically typed. Extends ADR-0023.

4. **Image-format handling: sniff via `image::guess_format` and send
   as-is.** No `--mime` flag, no re-encode. The editor output is almost
   always already PNG; re-encoding loses quality for no gain.

5. **Notification click action: open the URL in the default browser
   where supported, no-op elsewhere.** Matches the "share and view"
   intent. `notify-rust` supports actions on Linux; macOS/Windows
   degrade gracefully.

6. **`UploadOutcome.raw` field: deferred.** No v1 consumer needs the
   full response body. URL + delete-handle cover every use-case.
   Re-add when a debug UI or upload-history feature wants it.
