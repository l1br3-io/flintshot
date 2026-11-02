# Feature-landscape research — what the ecosystem just unlocked

Research notes, 2026-11-02. Primary sources only (projects' own trackers,
release pages, and repos; official protocol specs; crates.io / GitHub /
GitLab APIs); every claim carries a link, and flameshot 👍 counts cited
below were re-pulled today via the GitHub issues API. Scope: what changed
since the last pass ([`2026-10-10`](2026-10-10-next-valuable-additions.md))
and what the prior notes ([`2026-09-26-phase7`](2026-09-26-phase7-adaptation-research.md),
[`2026-10-05`](2026-10-05-feature-gap-analysis.md),
[`2026-10-07`](2026-10-07-ai-capability-features.md)) left uncovered —
already-queued work (MCP capture tool, OCR region → clipboard, action
pipeline, searchable history, export formats, watermark, crash-resume,
scripting surface) is built on, never re-proposed. Method note: GitHub's
HTML issue list rendered stale reaction counts today (e.g. [#5](https://github.com/flameshot-org/flameshot/issues/5)
showed 49👍 on the web page vs 122👍 via the API); API counts are the
authoritative ones cited here. Claims I could not verify from a primary
source are flagged in §7.

## 1. TL;DR — ranked candidates

Ranked by (evidence-backed demand × fit with VISION) ÷ effort. Items 1, 4,
6, 7 materially upgrade proposals already on file; items 2 and 10 are new.

| # | Feature | Evidence (primary, pulled today unless noted) | Effort | Verdict |
| --- | --- | --- | --- | --- |
| 1 | **Window + active-window capture via Screenshot portal v3** | Portal spec v3 adds a `target` option — Screen=1, Window=2, Area=4, **Active Window=8** — plus an `AvailableTargets` capability property ([spec](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Screenshot.html), [XML](https://github.com/flatpak/xdg-desktop-portal/blob/main/data/org.freedesktop.portal.Screenshot.xml)); "target selection in the Screenshot Portal" landed in [1.21.2](https://github.com/flatpak/xdg-desktop-portal/releases); demand: flameshot [#5](https://github.com/flameshot-org/flameshot/issues/5) 122👍 (API today), [#334](https://github.com/flameshot-org/flameshot/issues/334) (34👍, 10-10 note) | S–M | **Upgrades 10-05 §3.1**: window capture had "no Wayland path"; now it has a *sanctioned portal path*. Blocker: [ashpd 0.13.0](https://github.com/bilelmoussaoui/ashpd/releases) doesn't expose `target` yet — contribute upstream, we're the flagship consumer |
| 2 | **Replay buffer** — `flintshot record --replay 30s` (daemon keeps a rolling window; hotkey/tray saves the last N seconds) | [gpu-screen-recorder](https://flathub.org/apps/com.dec05eba.gpu_screen_recorder) 6.1.6 sells "instant replay, where only the last few seconds" ShadowPlay-style (~14.8k Flathub downloads/mo); OBS ships Replay Buffer as a headline output mode ([KB overview](https://obsproject.com/kb/obs-studio-overview), [frontend API](https://docs.obsproject.com/reference-frontend-api)) | M | New (never proposed in our notes); reuses the shipped record engine + daemon IPC + tray; the one recording feature that fits "short snips" perfectly — no muxer changes, just a retention policy + finalize-on-demand |
| 3 | **Sharpen the queued MCP capture server against the 2026 incumbent wave** | Six months ago the 09-26 note said "no Linux-native incumbent"; today crates.io lists [`computer-use-linux`](https://crates.io/crates/computer-use-linux) 0.7.36 (MIT, updated 2026-11-01, AT-SPI trees + screenshots across GNOME/KDE/Hyprland/niri/COSMIC/i3/X11), [`zeuxis`](https://crates.io/crates/zeuxis) (read-only screenshot server), [`screenshot-mcp`](https://crates.io/api/v1/crates?q=mcp+screenshot)/`mcp-screenshot`/`rmcp-screenshot`; [peekaboo](https://github.com/openclaw/Peekaboo/blob/main/docs/MCP.md) (macOS) is the depth benchmark; [screenpipe](https://github.com/mediar-ai/screenpipe) ships a history-search MCP; the [official servers repo](https://github.com/modelcontextprotocol/servers) still lists **no** screenshot server | M (queued) | Nobody combines capture → **annotate** → **ask** on Linux; ship the queued tool with a `target` parameter mirroring portal v3, then the `ask` tool already specced in 10-07 #5. [`rmcp` 3.8.0](https://crates.io/crates/rmcp) (2026-10-26, 43.4M downloads) is current |
| 4 | **OCR: pure-Rust `ocrs` engine beside the tesseract wrapper** | [`ocrs` 0.13.1](https://crates.io/crates/ocrs) (2026-09-13, 513,893 downloads, MIT/Apache-2.0) runs on the author's own [RTen](https://github.com/robertknight/rten) Rust NN engine — no `ort`, no C linkage, no system tesseract ([repo](https://github.com/robertknight/ocrs)); demand unchanged: flameshot [#702](https://github.com/flameshot-org/flameshot/issues/702) 61👍 (API today) | M | **Revises 09-26 §6**, which never considered ocrs: an `OcrEngine` trait with *two* impls — tesseract CLI (system, all languages) and ocrs (bundled-rust, Latin-only) — keeps VISION's "never a linked dependency" honest while making the queued OCR feature dependency-free. Caveats: repo self-describes as "early preview" with "more errors than commercial OCR engines"; models auto-download to `~/.cache/ocrs` (a network call — gate it behind explicit first-use consent per ADR-0069's user-initiated rule) |
| 5 | **Audio in recordings** (ADR-0070 deferred item — revisit) | [Kooha](https://github.com/SeaDve/Kooha) records mic + desktop audio on the same GStreamer+PipeWire+portal stack we already link; [ShareX 22.0.0](https://getsharex.com/changelog) ships system audio + mic; [PipeWire](https://gitlab.freedesktop.org/pipewire/pipewire) is at 1.6.11 (2026-10-29) on a steady cadence, and its GStreamer plugin provides the generic `pipewiresrc`/`pipewiresink` ([src/gst tree](https://github.com/PipeWire/pipewire/tree/master/src/gst)) in the `gstreamer1.0-pipewire` package flintshot already installs | M–L | The *dependency* cost is now ~zero (same plugin, already linked); the real cost is unchanged — ADR-0070's fMP4 finalize + muxer work. Evidence says users expect audio (Kooha, ShareX, Spectacle all have it); re-rank when the muxer budget exists |
| 6 | **GIF export for recordings** (VISION non-goal — revisit the small version) | [`gif` 0.14.2](https://crates.io/crates/gif) is a pure-Rust encoder (2026-04-09, 166M downloads, MIT/Apache-2.0); decode side already exists (the record engine's GStreamer); demand: flameshot [#172](https://github.com/flameshot-org/flameshot/issues/172) 126👍 (API today, open since 2018); [Kooha](https://github.com/SeaDve/Kooha) ships GIF among its four output formats; ShareX 22 added a GIF trimmer + adaptive quantizer ([changelog](https://getsharex.com/changelog)) | S–M | The non-goal was set when GIF implied `gifski` — which is **AGPL-3.0+** ([repo](https://github.com/ImageOptim/gifski)), correctly out. A GStreamer-decode → `gif`-encode post-process (MP4 in, GIF out) is pure-Rust on the encode side, no new link deps, and serves the 126👍 ask without touching the Recorder contract |
| 7 | **Eyedropper via the portal's `PickColor`** | `org.freedesktop.portal.Screenshot.PickColor` is a *shipped portal method* returning an sRGB triple ([spec](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Screenshot.html)); ShareX 21 added "screen color picking" ([changelog](https://getsharex.com/changelog)) | S | Upgrades 10-05 §3.7 from "sample the magnifier" to portal-native: on Wayland ask the portal (compositor-owned picker), elsewhere sample the captured frame. Pairs with the already-proposed ruler |
| 8 | **Searchable history on `tantivy`** (queued; name the engine) | [`tantivy` 0.27.0](https://crates.io/crates/tantivy) (2026-10-17, 21.4M downloads) — pure-Rust local search index, exactly the store-adjacent index ADR-0067 lacks | S delta on the queued L | Removes the last unnamed dependency from the queued OCR-history effort; index lives beside the session store, local-only |
| 9 | **QR/barcode decode** (reaffirm 10-05 §3.4) | [NormCap v0.6.0](https://github.com/dynobo/normcap/releases) (Aug 31) now ships QR **and barcode** detection via zxing-cpp ([README](https://github.com/dynobo/normcap)) — the flagship OCR-snipping tool just validated the selection→decode flow; `rqrr` remains our pure-Rust pick (10-05 §3.4) | S | Unchanged verdict; NormCap's move is fresh corroboration, and barcode (not just QR) is the delta to fold in |
| 10 | **`ext-image-copy-capture-v1` Wayland backend** (platform depth, long) | Protocol is live in wayland-protocols staging: captures **outputs and toplevels** (window sources), composites cursors via the `paint_cursors` option, and exposes dedicated pointer-cursor sessions ([v1 XML](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/blob/main/staging/ext-image-copy-capture/ext-image-copy-capture-v1.xml)); wayland-protocols shipped 1.50 on 2026-10-29 ([releases API](https://gitlab.freedesktop.org/api/v4/projects/wayland%2Fwayland-protocols/releases)); still v1-only, no v2 XML in-tree | M–L | A grim-class pure-Rust backend unlocks three shipped asks at once: **cursor in capture** (flameshot [#604](https://github.com/flameshot-org/flameshot/issues/604) 119👍, 10-10 note), window-rect capture, and portal-free scripted grabs on wlroots/COSMIC-adjacent compositors. Not stable ("testing phase") and compositor adoption unverified (§7) — design the `Capturer` trait to accept it, ship behind a probe |

Hygiene rider (no upstream issue evidence, cheap): the portal's
GlobalShortcuts grew a `ConfigureShortcuts` method and activation tokens in
[1.21.0](https://github.com/flatpak/xdg-desktop-portal/releases) — fold into
`flintshot hotkeys bind` UX when we next touch that code.

## 2. What changed in the comparable landscape (10-10 → today)

Everything below is from the projects' own release pages, verified today.

- **flameshot** shipped [v14.0.0](https://github.com/flameshot-org/flameshot/releases)
  (Jun 19) and [v15.0 RC1](https://github.com/flameshot-org/flameshot/releases)
  (Sep 19): portal-first Linux capture (required for Wayland), a unified
  monitor-selection rework, a grim-based Wayland adapter (v13, 2025), a
  COSMIC clipboard fix, and a legacy-X11 escape hatch for minimal WMs.
  **No** OCR, window capture, or scrolling — the v14/v15 investment went
  to multi-monitor correctness, the opposite of our differentiators. Top
  open-issue cluster today is still Wayland breakage: #3700 93👍, #3365
  80👍 (API today); the feature asks are unchanged (#1130 scrolling 137👍,
  #5 window 122👍, #172 GIF 126👍, #702 OCR 61👍).
- **ShareX** jumped [18 → 22](https://github.com/ShareX/ShareX/releases) in
  ~14 months. Two changes matter to us. First, **ShareX now ships AI**:
  "Analyze image" with opt-in, user-supplied keys (OpenAI, Gemini,
  OpenRouter) arrived in 19.0.1, with local-provider options in 20.0.2
  ([changelog](https://getsharex.com/changelog)) — the 10-07 note's "no AI
  anywhere" incumbent table is stale; flintshot's AI surface is no longer
  unique, *but* ShareX's is Windows-only and not annotation-integrated.
  Second, **22.0.0** (Oct 20, pre-release) ships native screen recording
  without FFmpeg — system audio, microphone, camera overlays, a
  pre-recording bar (start delay, cursor, mouse highlighter), 120 fps —
  plus HDR screenshot color correction, a Ruler tool, an animated-GIF
  trimmer, and a redesigned OCR tool.
- **Greenshot** revived: [1.3.328](https://github.com/Greenshot/greenshot/releases)
  stable (Oct 19) with a credentials-leak security fix, and continuous
  1.4.408 builds shipping every few days — the "Greenshot is dormant"
  assumption no longer holds.
- **ksnip** [v1.11.0](https://github.com/ksnip/ksnip): the sole maintainer
  is publicly **seeking a co-maintainer** ("can no longer dedicate
  sufficient time"). Its unmined features for us: stickers, watermarks,
  PDF/PS export, FTP/script uploaders, post-capture actions, and OCR via a
  version-locked plugin. Maintenance distress on our closest Qt analog is a
  positioning opening, not a feature.
- **NormCap** [v0.6.0](https://github.com/dynobo/normcap/releases) (Aug 31):
  QR/barcode detection (zxing-cpp) added to the select → OCR → clipboard
  flow; tesseract remains an external dep even in the flagship OCR snipper
  ([README](https://github.com/dynobo/normcap)).
- **Spectacle**: apps.kde.org lists the capture modes (entire desktop,
  current monitor, active window, **window under cursor**, rectangular
  region) ([app page](https://apps.kde.org/spectacle/)); the Gear 25.08
  announcement doesn't mention Spectacle, so recording claims stay pinned
  to the 10-10 note's tracker evidence (not re-verified today).
- **Recorders**: [Kooha](https://github.com/SeaDve/Kooha) outputs
  WebM/MP4/GIF/MKV with mic + desktop audio (window recording behind an
  env flag, "flickering"); [gpu-screen-recorder](https://flathub.org/apps/com.dec05eba.gpu_screen_recorder)
  6.1.6 does GPU-only capture with instant replay, live streaming,
  H264/HEVC/AV1/VP8/VP9 (+HDR), Opus/AAC audio, on X11 and Wayland.
- **screenpipe** (21.9k★, YC S26) is now **source-available under a
  commercial license** ([README](https://github.com/mediar-ai/screenpipe)) —
  it changed from open comparables to a product; our REFERENCES framing
  ("capture pipeline at scale") should note the license. Its MCP surface
  is `npx screenpipe-mcp` (history search / recent context / transcripts —
  tool names not enumerated in the README, §7).

## 3. Platform openings (Linux/Wayland)

### 3.1 Screenshot portal v3 — `target` + `AvailableTargets` + `PickColor`

The headline. [org.freedesktop.portal.Screenshot](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Screenshot.html)
is now **version 3**: `Screenshot()` takes a `target` option with values
Screen(1), Window(2, user-picked), Area(4), **Active Window(8)**, and
exposes a readable `AvailableTargets` bitmask so clients can probe what the
backend supports; `PickColor()` returns an sRGB triple. The feature
("target selection in the Screenshot Portal") landed in xdg-desktop-portal
[1.21.2](https://github.com/flatpak/xdg-desktop-portal/releases) (May 6);
1.22.0/1.22.1 (Jun 8/17, latest) rolled the 1.21 line up. Consequences:

- **Window + active-window capture now has a sanctioned Wayland path** —
  the single biggest correction to 10-05 §3.1, which assumed Wayland had
  none. The 10-05 fallback design (portal backend composites outputs;
  window pick degrades to region select) becomes "pass `target=2` or `8`
  and let the compositor pick the window".
- **ashpd is the gap**: 0.13.0 (Feb) has no Screenshot-target support
  ([releases](https://github.com/bilelmoussaoui/ashpd/releases)). Either
  upstream a `target` parameter (small PR; we'd be a flagship consumer) or
  pass the option through raw — a decision for the window-capture effort's
  spec.
- Backend support is per-compositor and **unverified** (§7); the spec's
  `AvailableTargets` property exists precisely so `flintshot backends` can
  probe and report honestly (VISION principle 5).
- Related open upstream ask: "Bring non-interactive screenshot options to
  parity with interactive ones" (help-wanted, on the tracker's
  [screenshot label](https://github.com/flatpak/xdg-desktop-portal/labels/portal%3A%20screenshot))
  — tracks our headless `--geometry` desire; watch it.

### 3.2 Other xdg-desktop-portal 1.21/1.22 changes

From the [release notes](https://github.com/flatpak/xdg-desktop-portal/releases):
GlobalShortcuts gained `ConfigureShortcuts` + activation tokens (1.21.0);
InputCapture gained clipboard access and session persistence (1.21.1 —
remote-control flows, not ours); ScreenCast streams gained
`pipewire-serials` (1.21.2); 1.22.1 fixed FileTransfer/FileChooser
security issues. Nothing else on our path changed.

### 3.3 `ext-image-copy-capture-v1` — the cursor/window unlock, still testing

The [v1 XML](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/blob/main/staging/ext-image-copy-capture/ext-image-copy-capture-v1.xml)
(staging, "testing phase"; still v1-only in-tree, verified via the
[repo API](https://gitlab.freedesktop.org/api/v4/projects/wayland%2Fwayland-protocols/repository/tree?path=staging/ext-image-copy-capture&ref=main))
captures "outputs **and toplevels**" into client buffers, composites
cursors on request (`paint_cursors`), and carries a dedicated
pointer-cursor session (position + hotspot events). A pure-Rust backend
over it (via `wayland-client`, sources enumerated with
`ext-foreign-toplevel-list`) would unlock cursor-in-capture, window-rect
capture, and portal-free scripted grabs — on every compositor that
implements it. wayland-protocols is current at [1.50 (2026-10-29)](https://gitlab.freedesktop.org/api/v4/projects/wayland%2Fwayland-protocols/releases);
per-compositor adoption is unverified (§7), so gate on a runtime probe the
way `flintshot backends` already does.

### 3.4 PipeWire audio — mature; the flintshot-relevant shape

[PipeWire](https://gitlab.freedesktop.org/api/v4/projects/pipewire%2Fpipewire/releases)
is at 1.6.11 (2026-10-29), six months past 1.6.0 "Penicillin", on a
weekly-cadence stable series. Its GStreamer plugin provides the generic
`pipewiresrc`/`pipewiresink` for audio *and* video
([src/gst](https://github.com/PipeWire/pipewire/tree/master/src/gst)) and
ships in the `gstreamer1.0-pipewire` package flintshot already requires
(README build deps). Dedicated `pipewireaudiosrc`-style elements were
described as in-progress in 2024 engineering write-ups (Collabora,
Asymptotic) but are **not** in PipeWire's tree or gst-plugins-good today
(§7) — plan audio capture on generic `pipewiresrc` (target a node) or
`pulsesrc`, not on element names from blog posts.

### 3.5 COSMIC epoch

[cosmic-epoch](https://github.com/pop-os/cosmic-epoch/releases) went
1.2.0 (Jun 30) → 1.11.0 (Oct 20) in under four months: remote-desktop
portal support (1.7.0), EI + dmabuf-v6 (1.6.0), xdg-dialog-v1 (1.11.0),
plus a store screenshot gallery (1.10.0). The COSMIC portal backend
implements Screenshot, ScreenCast, RemoteDesktop, Settings, FileChooser,
and Access ([cosmic.portal](https://raw.githubusercontent.com/pop-os/xdg-desktop-portal-cosmic/master/data/cosmic.portal)).
No color management/HDR/night-light shipped yet — so ShareX 22's "HDR
screenshot color correction" is a *future* watch item for us (KWin-first),
not a current COSMIC need.

## 4. The MCP capture-tool landscape (Nov 2026)

The 09-26 note's "no Linux-native incumbent" is obsolete. Current field,
all verified today:

| Server | Platform | Tools / posture | Signals |
| --- | --- | --- | --- |
| [peekaboo](https://github.com/openclaw/Peekaboo/blob/main/docs/MCP.md) | macOS only | `image`, `see` (OCR, ROI, UI elements), `inspect_ui`, `analyze` (BYO key: OpenAI/Anthropic/xAI/Ollama via env), plus full input control (`click`, `type`, `drag`, `menu`, …) | The depth benchmark; BYO-key env posture matches ours |
| [screenpipe](https://github.com/mediar-ai/screenpipe) | macOS/Win/Linux | MCP over history search / recent context / transcripts; REST on :3030; event-driven 24/7 capture | Source-available commercial license now; memory, not annotation |
| [computer-use-linux](https://crates.io/crates/computer-use-linux) | Linux | AT-SPI semantic trees, multi-compositor window targeting (GNOME/KDE/Hyprland/niri/COSMIC/i3/X11), screenshots, input, agent indicator | MIT, 0.7.36 updated 2026-11-01 — the active Linux incumbent for *control* |
| [zeuxis](https://crates.io/crates/zeuxis) | Linux | read-only screen/window/region screenshot server | 0.2.0, 65 downloads — minimal |
| `screenshot-mcp`, `mcp-screenshot`, `rmcp-screenshot` | cross-platform | generic screenshot servers | 93/93/24 downloads ([search](https://crates.io/api/v1/crates?q=mcp+screenshot)) — noise, but the namespace is filling |
| [termshot](https://crates.io/crates/termshot) | terminal | terminal-screenshot MCP with ANSI rendering + **redaction** | 1.2.0, Sep 2026 |
| [selah](https://crates.io/crates/selah) | lib (AGNOS) | AI-native capture/annotation/redaction *library* | GPL-3.0-only — license-incompatible as a dep, interesting as UX proof |

The [official servers repo](https://github.com/modelcontextprotocol/servers)
still lists no screenshot/capture server (reference + archived sets), and
the directory has moved to the MCP Registry — so "screenshot tooling" is
community territory. **The gap**: no Linux server does capture → annotate →
ask, none expose region/window targeting cleanly, and all the small ones
are capture-only. The queued `flintshot mcp` bin (09-26 §5 shape: RMCP
stdio + daemon IPC) should spec its `capture` tool with a `target`
parameter (screen/window/active-window/area — mirroring portal v3 §3.1),
return PNG via `ContentBlock::image` (RMCP supports it natively, 09-26 §5),
and land the `ask` tool (10-07 #5) as the second tool so the differentiator
ships together. None of the incumbents can follow up a capture with "what
does this error say?" against a BYO key.

## 5. Rust ecosystem deltas

- [`rmcp` 3.8.0](https://crates.io/crates/rmcp) (2026-10-26; 43.4M
  downloads) — the queued MCP server's SDK, current and official.
- [`ocrs` 0.13.1](https://crates.io/crates/ocrs) (2026-09-13; 514k
  downloads; MIT/Apache-2.0) — see candidate #4. Runs on RTen (pure-Rust
  inference), Latin-script only so far, "early preview" accuracy caveat,
  models auto-download to `~/.cache/ocrs` ([repo](https://github.com/robertknight/ocrs)).
- [`tantivy` 0.27.0](https://crates.io/crates/tantivy) (2026-10-17; 21.4M
  downloads) — the pure-Rust index for the queued searchable history.
- [`gif` 0.14.2](https://crates.io/crates/gif) (2026-04-09; 166M
  downloads; MIT/Apache-2.0) — pure-Rust GIF encoder; makes the small
  GIF-export candidate (#6) real. [`gifski`](https://github.com/ImageOptim/gifski)
  stays rejected: AGPL-3.0+.
- The `miniscreenshot-*` family ([search](https://crates.io/api/v1/crates?q=mcp+screenshot);
  0.3.1, 2026-10-15) splits capture into wlr-screencopy/X11/portal crates
  plus its own MCP server — evidence the "capture library + MCP server"
  pattern is commoditizing fast; our moat is the editor + AI, not the grab.
- Unchanged since the prior notes: JPEG XL still has no pure-Rust encoder
  (10-10 §3), `rqrr` still the QR pick, `async-openai` still the streaming
  crib (Phase 7).

## 6. Explicitly not re-proposed

Queued in ROADMAP (MCP tool, OCR → clipboard, action pipeline, searchable
history, export formats, watermark, crash-resume, scripting surface,
annotation presets) and proposed in 10-05/10-07/10-10 (upload breadth,
cursor, backdrop, scrolling-deferred, PII-redact assist, aspect lock,
CI budgets) — all stand. Status updates only: ShareX now ships BYO-key AI
(so the "no incumbent AI" line in 10-07 §2 and the 10-05 matrix cell is
stale — our differentiator is now *annotation-integrated, Linux-native*
AI, not AI per se); ShareX 21/22's "screen color picker" + Ruler
corroborate 10-05 §3.7 rather than add anything. Scrolling capture stays
deferred with no Wayland input-synthesis path (unchanged, 10-05 §3.6).

## 7. What could not be verified from primary sources

- **Which portal backends implement Screenshot v3 targets.** The spec and
  the frontend release notes are clear; per-backend support (gnome, kde,
  wlr, gtk, hyprland, cosmic) was not checked against backend releases or
  sources. The `AvailableTargets` probe is the sanctioned runtime answer;
  treat backend coverage as unknown until probed.
- **xdg-desktop-portal release years/dates.** The releases page shows no
  year stamps; I dated 1.21.2→1.22.1 as 2026 from recency ordering and
  today's "latest" marker. That 1.22.0 contains the 1.21 features is
  inferred from "bug fixes only" notes, not confirmed against a tarball.
- **`cosmic.portal` interface list completeness.** The fetched file lists
  Access/FileChooser/RemoteDesktop/Screenshot/Settings/ScreenCast —
  GlobalShortcuts is *not* listed there, yet flintshot binds hotkeys via
  the portal on COSMIC (some other backend may serve it). Unresolved.
- **ext-image-copy-capture compositor adoption** (which compositors
  implement v1, and whether COSMIC does) — no per-compositor check done.
- **`pipewireaudiosrc` element.** Not found in PipeWire's `src/gst` tree
  or gst-plugins-good main (API listings today); its existence in any
  released plugin is unverified — the 2024 Collabora/Asymptotic posts are
  engineering blogs (secondary). Plan on generic `pipewiresrc`.
- **gpu-screen-recorder's own repo** 404'd on two fetch attempts today;
  the [Flathub listing](https://flathub.org/apps/com.dec05eba.gpu_screen_recorder)
  (developer-authored) is the source used. OBS's Replay Buffer is cited
  from its KB/API docs, not the (404ing) dedicated KB page.
- **screenpipe's MCP tool names** — README describes capabilities but
  does not enumerate tools; not verified against the `screenpipe-mcp`
  package.
- **Spectacle's recording option set** (audio/cursor toggles) — apps.kde.org
  doesn't list recording and the Gear 25.08 announcement omits Spectacle;
  the 10-10 note's tracker-MR evidence stands unre-verified.
- **Peekaboo repo identity/stars** — fetched docs/MCP.md on the `openclaw`
  org path (works); whether `steipete/peekaboo` still redirects there and
  current star count were not re-checked today.
- **ShareX changelog details** (22.0.0 feature bullets) are from the
  project's own [changelog page](https://getsharex.com/changelog) as
  summarized on fetch; the per-entry text was not read in full.
- **flameshot HTML reaction counts** disagreed with API counts (§ method
  note); every count cited here is API-pulled today, but counts not
  re-pulled today (e.g. #604 cursor 119👍, #334 34👍) are as of 2026-10-10.

## What we adapt

| Candidate | Source (primary) | What exactly we take | Effort |
| --- | --- | --- | --- |
| Window/active-window capture | [Screenshot portal v3 spec](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Screenshot.html) + flameshot [#5](https://github.com/flameshot-org/flameshot/issues/5) | `target` option values 2/8 behind `CaptureOptions`; probe `AvailableTargets`; upstream `target` support into [ashpd](https://github.com/bilelmoussaoui/ashpd/releases) | S–M |
| Replay buffer | [gpu-screen-recorder](https://flathub.org/apps/com.dec05eba.gpu_screen_recorder), [OBS KB](https://obsproject.com/kb/obs-studio-overview) | `--replay <secs>` retention mode on the shipped recorder + tray/hotkey "save last N s"; no muxer changes | M |
| MCP server spec | [peekaboo MCP docs](https://github.com/openclaw/Peekaboo/blob/main/docs/MCP.md), [computer-use-linux](https://crates.io/crates/computer-use-linux), [rmcp](https://crates.io/crates/rmcp) | Queued tool gains `target` param + second `ask` tool; avoid input-control territory (incumbents own it) | M (queued) |
| OCR engine trait | [ocrs](https://github.com/robertknight/ocrs) + flameshot [#702](https://github.com/flameshot-org/flameshot/issues/702) | `OcrEngine` trait with tesseract-CLI *and* ocrs impls; model download is explicit first-use consent | M |
| GIF export | [gif crate](https://crates.io/crates/gif) + Kooha/flameshot [#172](https://github.com/flameshot-org/flameshot/issues/172) | MP4 → GStreamer decode → pure-Rust `gif` encode post-process; gifski rejected (AGPL) | S–M |
| Audio in recordings | Kooha/ShareX 22 precedent + [PipeWire gst tree](https://github.com/PipeWire/pipewire/tree/master/src/gst) | Re-rank ADR-0070 deferral when muxer budget exists; capture via generic `pipewiresrc`, zero new system deps | M–L (deferred) |
| PickColor eyedropper | [Portal spec](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Screenshot.html) | `PickColor` on Wayland, magnifier sampling elsewhere → clipboard hex | S |
| History index | [tantivy](https://crates.io/crates/tantivy) | Named engine for the queued searchable history; index beside ADR-0067 store | S delta |
| ext-image-copy-capture backend | [v1 XML](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/blob/main/staging/ext-image-copy-capture/ext-image-copy-capture-v1.xml) | Probe-gated `Capturer` impl: `paint_cursors`, toplevel sources, cursor sessions | M–L |
