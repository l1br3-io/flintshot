---
status: draft
started: 2026-09-15
---

# Spec — App window: close latency + no-return-after-capture

> Fix effort. Live report (COSMIC, 2026-09-15): *clicking Capture in
> the app window sometimes closes it late (~0.25–0.75 s)*, and *the
> window reappears after the capture finishes* — the user expects it
> to stay closed. Stands on ADR-0045 (app window), ADR-0047 (close
> for capture), ADR-0048 (prepare handshake), ADR-0052 (presence).

## Objective

Two changes, one close path:

1. **Close latency (bug).** A Capture click (button or `C`) must
   make the window visibly gone **within 200 ms, every time** — the
   500 ms stuck-close watchdog must never be load-bearing in the
   healthy path. Today the user sees ~0.25–0.75 s stalls on COSMIC.
2. **No return after a self-triggered capture (design change).**
   Win11-Snipping-Tool semantics: clicking Capture is the last thing
   the window does. The window does **not** reopen when that capture
   session ends, and the `flintshot app` process **exits** once the
   request is delegated. External captures (hotkey/tray) that close
   an open window still reopen it afterward — the capture took the
   window from the user mid-use.

### Locked decisions (user, 2026-09-15)

| Question | Decision |
| --- | --- |
| Which captures don't reopen? | Self-triggered only |
| Process after the window is gone? | Exit after successful delegation |
| Close budget | ≤ 200 ms hard, click → gone |

## Root-cause hypotheses (the delay)

The ~0.25–0.75 s range straddles two distinct mechanisms; diagnosis
is a first-class task, not a guess:

- **H1 — deferred first `Close`.** `ViewportCommand::Close` sent
  from the click handler is queued into the eframe event loop; on
  Wayland the loop's next iteration can park on a compositor frame
  callback. If that callback lags (compositor busy at click time —
  tray pre-hide, portal churn), the close processing waits. If the
  first `Close` is dropped entirely, the watchdog heals it at
  500 ms + one `REPOLL_INTERVAL` (250 ms) of slop → 500–750 ms.
  Matches the upper half of the report.
- **H2 — compositor-side teardown/animation.** cosmic-comp may
  animate the surface destroy (~250–350 ms) or simply tear down
  slowly under load. The client destroyed the surface promptly;
  the pixels linger anyway. Matches the lower half; **cannot be
  fixed client-side** — see Open Questions for the budget risk.

**Instrumentation first:** tracing timestamps at (a) click, (b)
`Close` queued, (c) `run_home` returned. The (a)→(c) split cleanly
separates client-side latency (H1) from compositor-side lingering
(H2): if (a)→(c) ≤ ~50 ms but the user still sees the window, it is
H2.

## Mechanism (the decisions)

1. **Exit path for self-triggered captures.** The outer loop's
   `CloseReason::CaptureIntent` arm becomes: grace (250 ms,
   unchanged — the not-in-the-shot guarantee is not touched) →
   send the command → **wait for the pump's delegation outcome**
   (new rendezvous in `Shared`, bounded — reuse the IPC delegate
   timeout) → `Delegated` ⇒ break; the process exits, releasing the
   app lock and the ADR-0052 presence connection.
   Failure (no daemon / delegate error) ⇒ today's force-show +
   error status: the window reopens, the user closes it, the
   process exits. Never exit silently on failure.
2. **`SessionEnded` no longer reopens a self-closed window —
   structurally.** The outer loop is gone before the session ends;
   nobody is parked in `wait_for_reopen`. `HomeCore`'s show logic
   stays as-is for the external-capture and failure paths.
3. **A cancelled self-capture keeps the window closed.** Instant-Esc
   on the selection UI still ends the session with the process
   already gone. Summons are fresh launches (`flintshot app`, tray,
   launcher — the app lock makes this cheap). Accepted edge; flagged
   so it is a decision, not an accident.
4. **Latency fix per diagnosis.** H1 ⇒ tighten the close path
   (e.g. shorten the watchdog 500 → ~150 ms, request immediate
   repaints after the click so the loop cannot park on a frame
   callback) — exact shape follows the instrumentation. H2 ⇒
   document, keep the client-side work, and renegotiate the budget
   with the user before claiming the 200 ms number.
5. **Presence races the exit safely by construction.** Delegation
   returns before the worker's prepare handshake starts, so process
   death may close the presence connection mid-handshake: the gate
   sees `app_alive → false` and degrades to grace-only, or the pump
   acks before dying. Both resolutions are the ADR-0052 design; no
   new ordering rules.
6. **The window's own copy updates**: the label "captures hide this
   window and return when done" becomes false — reword (external
   captures still do return).

## What is explicitly not in scope

- The pump's degraded mode (no daemon auto-reconnect) — unchanged.
- External-capture hide latency (the ≤ 250 ms directive repoll +
  handshake) — the report was self-triggered; separate concern.
- The 250 ms unmap grace and handshake depth — capture-start
  latency while the window is already closed; not user-visible.
- COSMIC portal interactive-selection flakiness (Spike C, known).

## Commands

From the repo root (AGENTS.md is canonical):

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --doc -p flintshot-core -p flintshot-editor-egui
cargo run -- app            # the window under test (COSMIC session)
```

## Project structure

| Touchpoint | File |
| --- | --- |
| Outer loop, `Shared` rendezvous, delegation outcome | `crates/flintshot-app/src/app_window.rs` |
| Watchdog, close instrumentation, label text | `crates/flintshot-editor-egui/src/home.rs` |
| Shell close path (`run_home`) if eframe-level work needed | `crates/flintshot-editor-egui/src/shell.rs` |
| New ADR (exit-after-self-capture) | `docs/adrs/ADR-00NN-app-window-exit-after-self-capture.md` |
| Live verification rows | `docs/TEST_MATRIX.md` |
| Changelog | `CHANGELOG.md` |

Daemon (`daemon.rs`, `prepare_gate.rs`, `sessions.rs`) is expected
**untouched** — hypothesis 5 exists to prove that, not assume it.

## Code style

Repo conventions (AGENTS.md + `.opencode/tech/rust/`): no `unsafe`,
`clippy::pedantic` clean without silent allows, no `unwrap()` outside
tests, `thiserror`/`color_eyre` split, comments only for non-obvious
"why". New shared-state seams follow the existing pattern:

```rust
/// Block (bounded) until the pump reports the delegation's fate.
/// The exit path must not guess: a silent exit on a failed
/// delegate strands the capture intent invisibly.
fn wait_delegation_outcome(&self, bound: Duration) -> Option<DelegationOutcome> {
    let deadline = std::time::Instant::now() + bound;
    let mut guard = self.lock();
    loop {
        if let Some(outcome) = guard.delegation_outcome.take() {
            return Some(outcome);
        }
        if guard.pump_dead || std::time::Instant::now() >= deadline {
            return None;
        }
        let (g, _t) = self.changed.wait_timeout(guard, deadline.saturating_duration_since(std::time::Instant::now())).expect("shared mutex poisoned");
        guard = g;
    }
}
```

## Testing strategy

- **Unit** (`app_window.rs`, existing suite is the pattern): the
  delegation-outcome rendezvous (success → exit, failure → show,
  pump-dead → no hang); `close_reason` classification unchanged
  (pending capture still wins over a racing hide); exit path
  releases without waiting for `Show`.
- **Unit** (`home.rs`): watchdog bound change, reworded label
  reachable, click instrumentation hooks headless-callable.
- **Live** (COSMIC, per TEST_MATRIX): ≥ 20 Capture clicks, timings
  logged, watchdog-fire count asserted 0; self-capture → no reopen +
  process exit ≤ 1 s after delegation; delegate-failure reopen;
  external-capture reopen unchanged.

## Boundaries

- **Always:** run all four gates before declaring done; update
  `CHANGELOG.md` (user-visible); write the ADR (this revises
  ADR-0047's restore semantics — that is an architectural change);
  keep the not-in-the-shot guarantee intact (grace + handshake
  ordering untouched).
- **Ask first:** touching daemon-side handshake semantics
  (`prepare_gate.rs`, `sessions.rs`); moving shared constants
  (`UNMAP_GRACE_MS`, `HIDE_ACK_WINDOW`); new dependencies.
- **Never:** `unsafe`; `#[allow]` without a justifying comment; GUI
  types in `flintshot-core`; exiting the process on a *failed*
  delegation (must surface, not vanish); weakening the
  external-capture close/reopen behavior.

## Success criteria

- Click → window gone ≤ 200 ms, p100 over ≥ 20 COSMIC trials (or a
  renegotiated bound, user-signed, if H2 dominates and is proven).
- Watchdog fires **0** times across those trials (a firing is a
  logged anomaly, never the mechanism).
- After a self-triggered capture: no reopen on `SessionEnded`;
  process exits ≤ 1 s after `Delegated`, exit code 0, app lock and
  presence released.
- Delegate failure / no daemon: window reopens with the error
  status; closing it exits the process.
- External capture with the window open: closes for the shot
  (unchanged), reopens when the session ends (unchanged).
- `cargo fmt --check`, `clippy -D warnings`, workspace tests, doc
  tests: green.

## Open questions

1. **Budget risk (H2):** if instrumentation proves the compositor's
   destroy animation dominates, 200 ms click→*pixels gone* may be
   unmeetable. Fallback position: ≤ 200 ms click→surface-destroyed
   (client-side truth) + documented compositor animation. Needs the
   user's sign-off only if it comes to that.
2. **Summon affordance:** with exit-after-delegate, resummoning is a
   fresh launch. Does the tray / COSMIC applet already expose an
   "app window" item (the daemon's `register_app` path suggests a
   spawn route exists)? If not, is one wanted here or as a follow-up?
