# ADR-0050: `Capturer` trait — `xcap` default behind a registry, portal native on Wayland

**Context.** ADR-0003 chose `xcap` for capture on the strength of its
cross-platform story, flagging its Wayland path (the `ScreenCast`
portal, per-session prompts) for re-evaluation. Two spikes measured
the alternatives on a live session (findings:
`docs/specs/native-capture-backends/tasks/001-spike-gnome-shell.md`,
`002-spike-portal.md`):

- `org.gnome.Shell.Screenshot` is allowlist-restricted for
  third-party callers since GNOME 41 (`DBusSenderChecker`; GNOME 49
  admits only MediaKeys and GNOME's own portal implementation). A
  GNOME-Shell backend would be dead code on every reachable session
  — Spike A verdict: `drop-to-portal`.
- `org.freedesktop.portal.Screenshot` (non-interactive) is
  prompt-free on COSMIC, 62–106 ms handle→Response plus 12–14 ms
  decode. The returned image composites **all outputs** at logical
  coordinates (single output keeps physical px); the protocol exposes
  no cursor option; response code 1 is a distinguishable
  "cancelled"; the cosmic backend emits 4-slash `file:////` URIs.
- Capture was also the last direct-dependency surface in
  `flintshot-core` with no trait boundary; every other pluggable
  concern already had one.

**Decision.**

- Capture moves behind a `Capturer` trait in
  `flintshot-core::capture` with a runtime registry
  (`registered_backends()`) and selection (`select_backend`):
  CLI `--backend` > config `[capture] backend` > auto by session
  kind (X11/Other → `xcap`; Wayland → `portal` → `xcap`). Auto never
  fails — `xcap` is the terminal fallback; a pinned backend whose
  probe fails is a hard error naming the backend and detail.
- `XcapCapturer` wraps the prior behavior unchanged; the old free
  functions (`monitors`, `capture_fullscreen`, `capture_region`)
  remain as delegating compatibility wrappers for
  `flintshot-app` / `flintshot-cosmic`.
- The single native slot is `PortalScreenshotCapturer`: raw
  `org.freedesktop.portal.Screenshot` over `zbus` 5 blocking
  (`cfg(target_os = "linux")`-gated dependency) — handle token
  derived from the unique bus name, match rule installed **before**
  the method call, the client stays on the bus until the `Response`
  signal, every phase time-bounded (2 s probe, 5 s method, 5 s
  response). URI parsed robust to 3/4-slash forms; file is read,
  deleted, then decoded from memory. Multi-monitor crops derive the
  effective scale from composite-size ÷ reported-bounding-box ratio —
  never `scale_factor()` (xcap's geometry on COSMIC is ~3× inflated).
- `use zbus` is permitted in core **only** under `src/capture/` — a
  path-scoped rule in `no_gui_types_in_core.rs`. The `ashpd` ban
  stays global (ADR-0024 precedent: D-Bus is a transport, not a GUI
  toolkit). No GNOME-Shell backend exists.
- `flintshot backends` prints every backend's probe result and the
  live `auto` pick — a diagnostic; probes only, never grabs, exits 0.

**Consequences.**

- Wayland sessions with a healthy portal get prompt-free,
  ~75–120 ms grabs by default; `[capture] backend = "xcap"` (or
  `--backend xcap`) restores the pre-portal path exactly.
- Portal grabs are always cursor-less (protocol gap, documented at
  the trait impl); the xcap path still composites the cursor.
- Multi-monitor `HiDPI` portal grabs are limited to logical
  resolution (the portal downscales; unavoidable via this protocol).
- Monitor enumeration stays on `xcap` unconditionally (spike-verified
  prompt-free) — the portal round-trips once per grab only.
- GNOME/KDE/wlr portal implementations are unmeasured (Spike B
  tested COSMIC only); a prompting or wedged impl degrades to `xcap`
  via probe/timeout. `docs/TEST_MATRIX.md` carries the per-desktop
  rows.
- Core gains a Linux-gated `zbus` dependency — the same workspace
  entry `ashpd` resolves to, so the graph carries one zbus major.

Supersedes ADR-0003: `xcap` remains the default provider and
universal fallback, now behind the trait.
