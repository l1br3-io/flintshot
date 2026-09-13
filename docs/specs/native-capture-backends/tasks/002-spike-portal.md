# Task 002: Spike B — portal Screenshot behavior per desktop

**Description:** Research task. Characterize the
`org.freedesktop.portal.Screenshot` call on every desktop this machine
can reach (GNOME, KDE, COSMIC, xdg-desktop-portal-wlr — whichever are
installed): does it prompt, how fast is it, what does the returned
image cover (one output vs full virtual desktop), is the cursor
included, and does the `interactive` option change any of that. Uses
`gdbus`/`busctl` one-liners plus a scratch zbus probe under
`/tmp/opencode/` — **no changes to the repo tree**. Findings gate
Task 6's scope (auto slot vs pin-only) and the crop-to-monitor
strategy.

**Acceptance criteria:**
- [ ] Per-portal-impl results recorded under `## Findings`: prompt
      behavior (none / first-call / every-call), end-to-end latency,
      returned URI format, image dimensions vs monitor count
- [ ] Cursor presence in the returned image verified (pixel-diff a
      with/without-cursor scenario or visual check)
- [ ] Coverage mapping: whether the image is one output, the primary
      monitor, or the whole virtual desktop — this decides how
      `capture_monitor(Some(n), ..)` crops
- [ ] Side-quest answered: does xcap *monitor enumeration* (not
      capture) ever trigger a portal prompt on Wayland?
- [ ] Verdict recorded: portal earns an `auto` slot, or `pin-only`;
      `spec.md` precedence amended if demoted

**Verification:**
- [ ] Findings contain per-impl concrete measurements
- [ ] Human confirms the verdict before Task 6 starts

**Dependencies:** None (parallelizable with Task 1, Task 3)

**Files likely touched:**
- `docs/specs/native-capture-backends/tasks/002-spike-portal.md` (findings appended)
- `docs/specs/native-capture-backends/spec.md` (only if precedence changes)

**Docs:**
- Findings feed Task 8's ADR.

**Estimated scope:** Small (1-2 files; scratch probe lives outside the tree)

## Findings

**Executed:** 2026-09-13 on this machine's live session: Wayland +
COSMIC (`cosmic-comp`), `xdg-desktop-portal` frontend +
`xdg-desktop-portal-cosmic` backend (Screenshot v2, per `portals.conf`
default `cosmic;gtk`). GNOME/KDE/wlr impls are **not installed here** —
their rows below are source/docs-derived, not measured; flagged as
such. Probe: scratch zbus-5 blocking client under
`/tmp/opencode/spike-b-portal/` (match-rule-before-call, thread +
recv_timeout deadline), plus `busctl`/`dbus-monitor` and a
`decode-timer` bin (image 0.25, release).

### Prompt behavior

- **COSMIC: never prompts.** The very first Screenshot call of the
  session returned success in ~100 ms with no dialog; backend source
  (`xdg-desktop-portal-cosmic` `src/screenshot.rs`) has no Access
  dialog on the non-interactive path — it goes straight to
  `screenshot_inner`. `interactive: true` opens COSMIC's own
  full-screen selection UI (layer surfaces, keyboard-exclusive;
  Enter=capture, Escape=`Cancelled`/code 1) — not usable for
  flintshot's programmatic path, and we don't need it (we have our own
  editor).
- **Client-lifetime gotcha:** the request handle is bound to the
  caller's unique bus name. If the client exits after receiving the
  handle, the frontend cancels the request (`Request.Close`) and the
  Response never arrives — even though the backend may still take the
  screenshot and write the file (observed: orphaned PNGs from
  short-lived `gdbus call` clients). The zbus blocking client must
  keep the connection alive until the Response signal.
- GNOME/KDE/wlr: unmeasured here. Per portal docs GNOME's impl may
  dialog/remember per app; treat as "may prompt first call" until
  measured on real sessions (TEST_MATRIX item for Task 8).

### Latency (2560×1440 single output, 6 runs)

| Phase | Measured |
| --- | --- |
| method call → handle | 1.4–1.5 ms |
| handle → Response signal (incl. grab + PNG encode + tmp write) | 62–106 ms |
| PNG decode (`image`, release) | 12–14 ms |
| **end-to-end budget for our backend** | **~75–120 ms** (vs 250 ms budget) |

### Returned URI

`file:////tmp/screenshot-XXXXXX.png` — note **four** slashes: the
cosmic backend builds `format!("file:///{}", path)` where `path` is
absolute (`/tmp/...`). Backend URI parsing must strip `file://` and
use the remainder as a path (robust to 3- and 4-slash forms). Files
land in `std::env::temp_dir()` with mode 0600, name
`screenshot-*.png` (tempfile crate). We must delete after decode.

### Coverage mapping (decides the crop)

From backend source (`screenshot_inner` + `combined_image`):
non-interactive Screenshot captures **all outputs** and composites
them into one image spanning the union of logical output rects.

- Single output: image = that output's full framebuffer, **physical
  px preserved** (crop path, no scaling).
- Multiple outputs: composite laid out at **logical** coordinates;
  frames whose px size ≠ logical size are **resized (Lanczos3)** —
  HiDPI multi-monitor captures get downscaled to logical resolution.
  Quality trap, unavoidable via this portal.
- Consequence for `capture_monitor(Some(n), ..)`: crop the composite
  by monitor n's logical rect (xcap `MonitorInfo` geometry divided by
  scale), never by any service index. Single-monitor fast path: crop
  is a no-op passthrough.

### Cursor

**Excluded, and not requestable.** Backend calls
`capture_source_shm(source, /* overlay_cursor */ false)`; visual
verification of captured PNGs shows no cursor anywhere; and the
`org.freedesktop.portal.Screenshot` protocol exposes no cursor option
at all (options: `handle_token`, `modal`, `interactive`). This is a
**permanent gap**: `CaptureOptions::include_cursor` cannot be honored
by the portal backend — document at the trait impl, always capture
cursor-less via this path. (Contrast: xcap path honors the flag.)

### Response codes

`0` = success (`uri` in results). `1` = cancelled/dismissed
(`PortalResponse::Cancelled` — e.g. Escape from interactive UI).
Other/backend errors surface as portal error replies. Task 6's
"cancelled" substring mapping is exercisable via code 1.

### Side-quest: does xcap monitor enumeration prompt on Wayland?

**No.** `xcap::Monitor::all()` returns in ~0 ms (this COSMIC session),
no portal traffic, no dialog. Monitor *capture* via xcap also
completed unattended (~450 ms first grab incl. PipeWire setup).
Safe to keep enumeration on xcap unconditionally (plan decision
confirmed).

Bonus xcap-on-COSMIC quirk worth a GOTCHA comment (not this effort's
bug): `MonitorInfo` reports 7680×4320 @ scale 0.333 for a physical
2560×1440 output — xcap's geometry/scale reporting on COSMIC is
inflated 3×. Any crop math that mixes portal-logical coords with
xcap-reported geometry must derive scale from
`capture_width / reported_width`, not from `scale_factor()`.

### Verdict: portal earns the `auto` slot on Wayland

Prompt-free (COSMIC), ~75–120 ms end-to-end, single-monitor
physical-px fidelity, cancelled-distinct errors, no new system deps.
Precedence table as amended by Spike A stands: `Wayland (any) →
portal → xcap`; no spec change needed from this spike. Residual risk:
GNOME/KDE/wlr portal impls unmeasured (may prompt); auto falls to
xcap on probe failure anyway, so a prompting impl degrades gracefully
only if we keep the 5 s timeout + availability probe — noted for
Task 6.
