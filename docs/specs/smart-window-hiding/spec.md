---
status: active
started: 2026-09-09
---

# Spec — Smart window hiding: capture self-exclusion + single-session replace

> Fix effort. Stands on the tray-hide handshake (ADR-0036/0037/0042),
> the app window (ADR-0045/0047), and editor recapture (ADR-0046).
> It revisits none of them — it generalizes their orchestration
> pattern from "the surface that *initiates* a capture hides itself"
> to "the daemon clears **all** flintshot chrome from the screen
> before **any** grab", and closes the duplicate-editor gap the
> app-window ↔ tray interplay surfaced live.

## Objective

Two user-visible bugs, one root cause — the daemon has no notion of
an *active capture session* and no orchestration for surfaces it
didn't spawn for this grab:

1. **Self-capture.** A grab triggered from tray / hotkey / CLI /
   applet while another flintshot window is open (editor, overlay,
   app window, pin) freezes that window into the shot. Today only
   the *initiating* surface hides (tray icon handshake, app window
   self-close, editor self-recapture) — `flintshot-core/src/capture.rs`
   does raw xcap grabs with zero window filtering.
2. **Duplicate windows.** Every `CaptureRegion`/`CaptureFullscreen`
   dispatch spawns a fresh worker unconditionally (`worker.rs`
   dispatch table; deliberate per `single.rs` docs, but unbounded):
   an in-progress capture from the app window + a tray capture =
   two stacked editor windows.

**Decisions (confirmed with the user):**

- **Replace policy** — a new capture trigger *supersedes* any
  in-flight capture session: the old editor/overlay closes (outcome
  `Cancelled`, no save), the new capture runs. One annotation
  surface at any time; newest intent wins.
- **Exclusion scope** — editor windows, the fullscreen overlay
  selection window, the app window, and pin windows must **never**
  appear in a grab. (Tray icon: already solved, untouched.)
- **Mechanism** — close-then-reopen, the codebase's proven pattern
  (app window ADR-0047, editor recapture ADR-0046). No true-hide /
  unmap tricks; no xcap-level window filtering (impossible under
  Wayland portal captures anyway).
- **In scope extra** — make `Event::EditorOpened` real: workers
  report when a window actually opens, replacing the process-spawn
  proxy broadcast.

**Success looks like:** editor open, hit the tray capture → the old
editor closes, the new shot contains zero flintshot chrome, exactly
one editor window exists afterwards. App window open, hit the hotkey
→ the shot has no app window; it returns when the session ends.
Pinned shots stay pinned across captures. Double-triggering never
stacks windows.

## Mechanism (the decision)

Three coordinated changes, all daemon-orchestrated (workers stay
dumb spawn targets; the daemon owns cross-window truth).

### 1. Capture session registry + Replace (`flintshot-app::sessions`)

A `CaptureSessions` registry in the daemon, designed like `TrayGate`:
pure state machine (unit-testable, injected clock) + thread-safe
shell.

- `CommandSpawner` registers spawned **capture workers** and **pin
  workers** (argv + kill handle); the reaper deregisters on process
  exit — process death stays the crash-safe truth source (ADR-0046
  consequence preserved).
- **Dispatch ordering** (`worker::dispatch`, all entry points
  converge there): a capture request first **supersedes** —
  1. broadcast `Event::CaptureSuperseded`;
  2. live capture workers close gracefully (see worker side, below);
  3. wait bounded (~1.5 s) for reaper-confirmed exits;
  4. hard-kill stragglers via the stored handle, wait again;
  5. *then* spawn the new worker.
- **Serialization**: the supersede-then-spawn phase runs under a
  capture mutex so two simultaneous triggers can't race past step 4.
  Latest wins once serialized; no queue.
- **Worker side (graceful close)**: capture workers hold an IPC
  subscription for the session's lifetime; on
  `Event::CaptureSuperseded` a listener thread closes the window via
  the egui `Context` (`ViewportCommand::Close`) → normal exit path
  (RAII guards still report captured/saved-as-cancelled; no partial
  PNG writes, which is why graceful precedes kill).

Editor-hosted pins die with their superseded host process — a
documented limitation (see Open questions 1). Standalone pin workers
(`PinLast` spawns) are **not** superseded — they're hidden and
restored (below).

### 2. Generalized pre-grab handshake (`PrepareCapture`)

The worker pre-grab handshake grows from tray-only to
all-own-chrome. New appended IPC variants (postcard variant-index
rule, ADR-0011):

- `Request::PrepareCapture` — request/reply, mirrors
  `HideTrayForCapture`'s shape. The daemon:
  1. re-verifies no live capture sessions (dispatch already
     ensured it);
  2. broadcasts `Event::HideForCapture` → a subscribed app window
     closes itself and acks `Request::ReportAppHidden` (best-effort;
     sent by the app window's outer loop once its eframe call
     returns);
  3. closes registered pin workers (kill; respawn argv queued);
  4. drives the existing tray gate;
  5. replies `Event::CaptureReady` once tray-gate done **and**
     (app-hidden ack received **or** no app window known) **and**
     pin processes reaped — every wait bounded (existing 2 s client /
     3 s server pattern); on timeout, ack anyway with a warning
     (a capture must never fail because a window was slow — the
     ADR-0036 philosophy, extended).
- `Request::ReportCaptured` (existing, unchanged shape) now also
  **respawns queued pins** with their original argv (`--at/--size/
  --opacity` survive, so position is preserved). Tray restore
  unchanged. App-window restore is already driven by
  `SessionEnded`/`in_flight` (`HomeCore::maybe_show`) — no new
  daemon action.
- `HideTrayForCapture` keeps working (rolling-upgrade skew: old
  workers, new daemon).

**Worker flow** becomes: `TrayHiddenGuard`-style RAII generalized to
a `CaptureGuard` — `PrepareCapture` → ack → grab → drop →
`ReportCaptured`. The editor/overlay session beyond the guard holds
nothing hostage (unchanged principle).

**App window** (`app_window.rs` + `home.rs`): `HomeCore` sets
`hidden = true` on **every** `CaptureStarted` (today only
self-triggered captures hide it) and returns a new
`HomeDirective::Hide`; the outer loop learns to *wait for reopen*
when hidden-by-event (not only when a self-triggered capture is
pending — otherwise it would exit the process as a user quit), and
sends `ReportAppHidden` once closed.

### 3. Real `EditorOpened`

- New `Request::ReportEditorWindowOpened` (worker → daemon,
  fire-and-forget like `ReportSaved`): sent in the worker just
  before the editor/overlay window runs (after interaction-mode
  resolution in `run_capture_shell`, and in `run_open`). The IPC
  layer intercepts it and broadcasts `Event::EditorOpened`.
- The optimistic spawn-time broadcast in `worker::dispatch` is
  removed. `OpenLast`'s proxy goes too (the `open` worker reports).
  Recapture chains report before each reopen — one event per actual
  window, which is the truthful signal a future gate needs.
- `--out` headless captures never emit `EditorOpened` (nothing
  opened). Audit remaining subscribers (COSMIC applet) before
  removing the proxy.

## Scope (in / out)

**In:** core IPC schema (4 `Request`, 2 `Event` variants, appended);
`flintshot-app::sessions` registry (pure + shell) with supersede
ordering in `worker::dispatch`; `PrepareCapture` coordination in the
IPC layer + daemon loop; pin kill/respawn; worker-session
subscription + graceful editor/overlay close (editor-egui close
handle); app-window hide-on-any-capture + `ReportAppHidden` +
outer-loop wait; real `EditorOpened` wiring; ADR, CHANGELOG,
TEST_MATRIX rows.

**Out:** `--delay-ms` tray-hold semantics (ADR-0036 open question 2,
still deferred); xcap-level window filtering (impossible on Wayland
portal captures; orchestration is the portable mechanism); true
hide/unmap (X11-only, compositor-roulette on Wayland); restoring
editor-**hosted** pins after supersede (durable pins — follow-up);
standalone no-daemon multi-window orchestration (no coordinator
exists; unchanged, documented); COSMIC applet window changes;
per-platform grace tuning (consts, revisit if a platform ghosts).

## Commands

From the repo root (full detail in `AGENTS.md`):

```sh
cargo build                              # default-members
cargo test --workspace --all-targets     # unit + integration, all crates
cargo test --doc -p flintshot-core -p flintshot-editor-egui
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -- region                       # manual: trigger via tray/hotkey while open
cargo run -- app                          # manual: app-window interplay
```

## Project structure

No new crates. Touched modules:

| Area                              | Path                                                        |
| --------------------------------- | ----------------------------------------------------------- |
| IPC schema (+ tests)              | `crates/flintshot-core/src/ipc.rs`                          |
| Session registry (new)            | `crates/flintshot-app/src/sessions.rs`                      |
| Dispatch supersede + spawner      | `crates/flintshot-app/src/worker.rs`                        |
| Handshake interception/loop       | `crates/flintshot-app/src/ipc_server.rs`, `daemon.rs`       |
| Worker guard + subscription       | `crates/flintshot-app/src/cli.rs`, `ipc_client.rs`          |
| Graceful editor/overlay close     | `crates/flintshot-editor-egui/src/{app,overlay,shell}.rs`   |
| App-window hide/ack               | `crates/flintshot-app/src/app_window.rs`, `home.rs` (egui)  |
| Docs                              | `docs/adrs/`, `CHANGELOG.md`, `docs/TEST_MATRIX.md`         |

## Code style

House rules per `AGENTS.md` and `.opencode/tech/rust/` (no `unsafe`,
`#![warn(clippy::pedantic)]`, `thiserror`/`color_eyre` split, no
`unwrap()` outside tests, `#[non_exhaustive]` on public types,
kebab-case serde renames, comments only for non-obvious *why*).
New IPC variants append at the end of their enums. The registry
mirrors `tray_gate.rs`'s established pure-machine + shell split with
injected clock and graces as named consts.

## Testing strategy

- **Pure state machines** (`sessions.rs`, `HomeCore` additions,
  dispatch ordering): unit tests with a fake spawner/recording sink
  and injected clock — supersede ordering, straggler kill path,
  serialization, pin respawn queue, hidden-by-event restore.
- **IPC layer**: interception tests with a live-stepped gate (the
  `ipc_server.rs` pattern) for `PrepareCapture`/`ReportAppHidden`/
  `ReportEditorWindowOpened`; postcard round-trips for new variants.
- **Editor close handle**: editor-egui unit tests that the
  supersede signal resolves the outcome slot to `Cancelled`.
- **Manual matrix** (recorded in `TEST_MATRIX.md`): the two report
  scenarios above, hotkey-while-editor, applet trigger, kill -9 a
  straggler worker mid-supersede, standalone unchanged.

## Boundaries

- **Always:** every wait bounded (no unbounded daemon or worker
  blocking); process-exit stays the session truth (reaper); IPC
  variants append-only; all standing gates green before done.
- **Ask first:** new dependencies (none anticipated — kill handles
  via stored `Child`, no libc/nix); any change to `SessionEnded`
  semantics; changing the tray-hide handshake contract.
- **Never:** GUI toolkit types in `flintshot-core`'s public API
  (enforced by `tests/no_gui_types_in_core.rs`); `unsafe`;
  capture-level window filtering as the primary mechanism.

## Success criteria / exit criteria

1. Editor open (any source) + capture from tray, hotkey, CLI,
   applet, or app window → old editor closes unsaved-gracefully
   (or is killed after grace), exactly one editor remains, and the
   new frozen frame contains **no** flintshot surface.
2. App window visible + externally triggered capture → app window
   absent from the shot; it reopens when the session ends.
3. `PinLast` pins: absent from the shot; respawned at the same
   position/size/opacity after the grab.
4. Two near-simultaneous triggers → one session survives (serialized
   supersede; latest wins); no fork loop, no deadlock.
5. Kill -9 a worker mid-supersede: session still ends (reaper), the
   next capture proceeds, no leaked hidden state (tray/app/pins all
   restore within their bounds).
6. `EditorOpened` fires only when a real window opens (never for
   `--out`; once per recapture reopen; not at spawn).
7. Standalone (`--standalone`, no daemon): unchanged behavior, no
   new latency (one socket probe, vacuous handshake).
8. Standing gates: `fmt`, `clippy -D warnings`, workspace tests,
   doc tests.

## Open questions

1. **Durable pins** — editor-hosted pins die with their superseded
   host and are not restored. Acceptable for now (the session was
   abandoned); a follow-up effort could make all pins
   daemon-registered with saved image + geometry so Replace
   restores them. Flag in the ADR's consequences.
2. **Orphaned pins across daemon restarts** — a new daemon doesn't
   inherit the old daemon's pin registry, so pre-restart pins can
   appear in captures (pre-existing behavior, now explicit).
   Persisting the registry is a candidate follow-up.
3. **Supersede UX** — should the replaced editor get any
   acknowledgment (e.g. a status flash), or is silent replace fine?
   Default: silent (matches "newest intent wins").
4. **Grace values** — supersede grace ~1.5 s, kill fallback short;
   reuse the tray handshake's 2 s/3 s bounds for `PrepareCapture`.
   Consts now, config knobs later if L5 shows compositors ghosting.
