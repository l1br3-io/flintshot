# Task 001: Spike C — live-verify interactive portal

**Description:** Research task resolving the spec's open question
#1 before anything builds on interactive mode. Re-run the spike-B
scratch zbus client (`/tmp/opencode/spike-b-portal/portal-shot`, it
already has an `--interactive` flag) against the live COSMIC portal
and record: (a) does the Response carry `uri` + code 0 after
select-and-Enter, (b) **is the returned image exactly the selection**
(dims vs a known-size selection) or a composite needing a crop,
(c) Escape/dismiss → code 1 with no file, (d) latency
call→handle→Response, (e) any dialog/prompt behavior. Amends the
spec's Open Questions with the answer and feeds Task 2's
implementation choice (passthrough vs crop reuse).

**Acceptance criteria:**
- [ ] Interactive grab measured on the live session: Enter path and
      Escape path both exercised, findings recorded under
      `## Findings` here
- [ ] Return-shape question answered explicitly: selection-only vs
      composite (with dims evidence)
- [ ] Latency numbers recorded (call→handle, handle→Response)
- [ ] `spec.md` Open Question #1 amended with the verdict

**Verification:**
- [ ] Findings contain concrete measurements (no repo code changes;
      scratch stays under `/tmp/opencode/`)

**Dependencies:** None (the merged portal backend is the substrate)

**Files likely touched:**
- `docs/specs/native-capture-ux/tasks/001-spike-interactive.md` (findings)
- `docs/specs/native-capture-ux/spec.md` (open-question verdict)

**Docs:**
- Findings feed Task 2 and Task 10's ADR.

**Estimated scope:** Small (2 files; scratch probe outside the tree)

## Findings

**Executed:** 2026-09-14, live Wayland/COSMIC session
(`xdg-desktop-portal` 1.18.4 + `xdg-desktop-portal-cosmic`
`0.1.0pop1~1788284590~24.04~abf258b`, Screenshot iface v2, single
2560×1440 output). Probe: scratch zbus-5.18 blocking client rebuilt at
`/tmp/opencode/spike-c-portal/` (the spike-B original was cleaned
from /tmp; identical dance — match-rule-before-call, per-request
path, 180 s Response wait).

### (a) Response shape after select-and-Enter

`Response` code **0** + `uri` key. Every Enter-path grab returned a
decodable PNG with **100% opaque, real screen content** (verified by
full defiltered alpha scan; an early "transparent" readout was a
probe-side PNG-defilter bug, not portal behavior).

### (b) Return shape: exactly the selection — passthrough, no crop

| Run | Interaction                          | Returned dims        | Content            |
| --- | ------------------------------------ | -------------------- | ------------------ |
| 1   | drag ~57%×68% + Enter                | 1472×984             | real, 100% opaque  |
| 5   | drag collapsed to a click + Enter    | 1×1                  | real (white px)    |
| 6   | slow deliberate drag ~⅓ screen + Enter | 2230×1091          | real, 100% opaque  |

Dims track the dragged rect (logical px). **No composite is
returned; Task 2 implements passthrough** (the non-interactive
`crop_composite_to_monitor` dance is NOT wanted on this path).

### (c) Escape / dismissal: unreliable on this build

Three distinct observed behaviors for "user backs out":

| Runs | Escape scenario                       | Result                                          |
| ---- | ------------------------------------- | ----------------------------------------------- |
| 2–3  | Escape after a selection existed      | **code 0 + uri** — spurious capture of the stale/default rect |
| 4    | Escape during mode/drag interaction   | **code 1**, no uri (proper `Cancelled`)          |
| 7    | Escape immediately, no prior input    | **no Response within 180 s** — request hangs    |

Master-branch source (`src/screenshot.rs`) maps Escape →
`Msg::Cancel` → `PortalResponse::Cancelled` (code 1); the installed
snapshot diverges. Consequences for Task 2:

- code 1 → keep flowing as the `"cancelled"` capture error (contract
  exists and was observed);
- the **no-Response hang** means the interactive path needs both a
  long Response budget (user think-time) and a hard outer bound with
  a quiet timeout-exit — a 5 s `RESPONSE_TIMEOUT` can never work
  here;
- spurious code-0-after-Escape captures can't be defended against
  client-side in general (degenerate-dims heuristics only catch the
  1×1 case). GOTCHA + TEST_MATRIX row; revisit when COSMIC ships a
  fixed portal build.

### (d) Latency

- method call → handle: **1.1–2.0 ms** (same as non-interactive).
- handle → Response: dominated by the user (1.9–32.6 s across runs);
  post-Enter save+respond is fast — no separate measurement possible
  from outside (bounded by human timing).
- Non-interactive baseline re-measured same session: 107.9 ms
  end-to-end, 2560×1440 composite — unchanged from spike B.

### (e) UI / dialog behavior and file location

- The selection UI is a keyboard-exclusive layer surface
  (`Anchor::all`, `Layer::Overlay`); no permission dialog on any
  path.
- COSMIC's drag mechanism is **DnD-based** (`start_dnd` in
  `rectangle_selection.rs`) and flaky on this build: run 5's
  deliberate drag collapsed to a 1×1 click-rect, run 4's drag+Enter
  became `Cancelled`. Compositor-side bug; recorded for the ADR and
  TEST_MATRIX (newer portal builds may fix).
- **Files land in `~/Pictures/Screenshots/Screenshot_*.png`** (cosmic
  `get_img_path`, default save location from its own config), *not*
  `/tmp` like the non-interactive path. Task 2 must read-then-delete
  (the dir is user-visible; leaving files litters it on every grab).

### Verdict for the spec's Open Question #1

**Interactive returns exactly the selection** (passthrough; no crop
reuse needed). Cancellation (code 1) is reachable but **not
reliable on the installed COSMIC build** — design for quiet-exit on
code 1 *and* on Response timeout.

### Addendum: the interactive path wedges under sustained hammering

After ~7 interactive requests in one session, every subsequent
interactive call hung (UI opens, user selects + presses Enter or the
capture button, **no Response ever arrives**). Began at run 7 and
persisted across: portal-impl restart, broker restart, cosmic
screenshot-config clearing (probe + `cargo test` clients alike).
The **non-interactive portal grab kept working** (2560×1440, ~0.1 s)
throughout — the wedge is specific to the selection-UI path, almost
certainly layer-surface/keyboard state in `cosmic-comp` (only
process not restarted; survives everything short of a comp restart
or reboot).

Implications (already reflected in Task 2):

- `capture_selection` must treat "no Response within budget" as a
  first-class quiet-exit outcome (`"timed out"` substring), not an
  error toast — on a wedged session every interactive grab ends
  there until the user reboots.
- The live `#[ignore]`'d test soft-skips on the hang with a
  diagnostic (verified twice on the wedged session).

Also recorded: cosmic persists screenshot UI state to
`~/.config/cosmic/com.system76.CosmicPortal/v1/screenshot`
(`choice: Rectangle`, `last_rectangle`) — restored selections explain
the "stale rect" spurious captures in runs 2–3. Clearing this file
was verified **not** to unwedge the session (backed up under
`/tmp/opencode/spike-c-portal/config-backup/`).
