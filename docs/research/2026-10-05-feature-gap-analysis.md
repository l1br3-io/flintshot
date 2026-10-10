# Feature-gap analysis — what else can we add?

Research notes, 2026-10-05. Primary sources only (official sites, upstream
repos, project issue trackers, crates.io API); every competitor claim below
carries a link, every flameshot 👍 count was re-pulled today via the GitHub
search API (`sort=reactions-+1-desc`). Scope: features **not yet shipped or
queued** in [`../ROADMAP.md`](../ROADMAP.md). Already queued (MCP capture
tool, OCR region → clipboard, after-capture pipeline, searchable history,
annotation presets, packaging) is only re-sliced, never re-proposed — the
demand evidence for those lives in
[`2026-09-26-phase7-adaptation-research.md`](2026-09-26-phase7-adaptation-research.md) §7.

## 1. TL;DR — ranked additions

Ranked by (upstream demand evidence × fit with VISION) ÷ rough effort.

| # | Feature | Demand evidence (primary) | Effort | Verdict |
| --- | --- | --- | --- | --- |
| 1 | **Window / active-window capture** | flameshot [#5](https://github.com/flameshot-org/flameshot/issues/5) (122👍) + [#334](https://github.com/flameshot-org/flameshot/issues/334) (34👍); shipped in Spectacle, ShareX, Shottr | M | Do next after the queued MCP tool; VISION already promises it |
| 2 | **Upload breadth + config-driven custom uploader** | flameshot [#50](https://github.com/flameshot-org/flameshot/issues/50) (40👍) + [#499](https://github.com/flameshot-org/flameshot/issues/499) (29👍) + [#27](https://github.com/flameshot-org/flameshot/issues/27) (21👍) | S per backend, M generic | `Backend` trait was designed for exactly this |
| 3 | **Cursor in the capture, surfaced** | flameshot [#604](https://github.com/flameshot-org/flameshot/issues/604) (119👍, 44💬) | S–M | `CaptureOptions::include_cursor` already threads the code; make native backends honor it |
| 4 | **QR decode from selection** | flameshot [#511](https://github.com/flameshot-org/flameshot/issues/511) (22👍); Shottr, CleanShot, ShareX all ship QR | S | `rqrr` is mature pure Rust; pairs with queued OCR |
| 5 | **Backdrop: padding, background, rounded corners, drop shadow** | Shottr and CleanShot both sell it as a headline; flameshot [#313](https://github.com/flameshot-org/flameshot/issues/313) shadows (23👍) | S–M | Pure `image`-crate work; polish differentiator |
| 6 | **Scrolling capture** | flameshot [#1130](https://github.com/flameshot-org/flameshot/issues/1130) (137👍, #2 open issue, open since 2019); CleanShot, Shottr, ShareX ship it | L | Highest demand, worst portability (input synthesis). Defer to a post-1.0, per-platform experiment — but decide explicitly |
| 7 | **Eyedropper + ruler off the magnifier** | Shottr ruler/color copy ([shottr.cc](https://shottr.cc)); ShareX "screen color picker" + "ruler" ([README](https://github.com/ShareX/ShareX/blob/develop/README.md)); Snipaste color picker ([snipaste.com](https://www.snipaste.com)) | S | Magnifier already samples pixels; clipboard write already exists |
| 8 | **`--print-geometry` (grim/slurp compose)** | slurp's whole contract is "select a region and print it to stdout" ([README](https://github.com/emersion/slurp)); Satty documents `grim -g "$(slurp)"` as *the* wlroots flow ([README](https://github.com/gabm/Satty)) | S | One flag; plugs flintshot into the Wayland scripting world |

Hygiene, not ranked (no upstream issue evidence gathered): autostart-on-login
for the daemon via [`auto-launch` 0.6.0](https://crates.io/crates/auto-launch)
(6.4M downloads, updated 2026-01-10) — table stakes for a daemon tool.

## 2. Feature matrix

✅ shipped · ⚠️ partial/qualified · ❌ absent · n/a platform-moot.
flintshot column = today (`main`), not the roadmap.

| Feature | flintshot | Flameshot | Satty | CleanShot/Shottr | ShareX | Spectacle |
| --- | --- | --- | --- | --- | --- | --- |
| Region + fullscreen capture | ✅ | ✅ | ⚠️ editor-only (stdin/file input) | ✅ | ✅ | ✅ |
| Window / active-window capture | ❌ | ⚠️ — [#5](https://github.com/flameshot-org/flameshot/issues/5) still open | ❌ | ✅ ([Shottr v1.5.0 notes](https://shottr.cc)) | ✅ ([README](https://github.com/ShareX/ShareX/blob/develop/README.md)) | ✅ ([apps.kde.org](https://apps.kde.org/spectacle/)) |
| Scrolling capture | ❌ | ❌ [#1130](https://github.com/flameshot-org/flameshot/issues/1130) | ❌ | ✅ (both) | ✅ ([docs](https://getsharex.com/docs/scrolling-screenshot)) | ❌ |
| OCR on selection | ❌ (queued) | ❌ [#702](https://github.com/flameshot-org/flameshot/issues/702) | ❌ | ✅ (both, on-device) | ✅ ([docs](https://getsharex.com/docs/ocr)) | ❌ |
| QR decode | ❌ | ❌ [#511](https://github.com/flameshot-org/flameshot/issues/511) | ❌ | ✅ (both) | ✅ | ❌ |
| Cursor in capture | ⚠️ option exists; portal path is a documented protocol gap ([ADR-0050](../adrs/ADR-0050-capturer-trait-native-backends.md)) | ❌ [#604](https://github.com/flameshot-org/flameshot/issues/604) | ❌ | ⚠️ unverified (not advertised) | ⚠️ editor-side "cursor insertion" only ([README](https://github.com/ShareX/ShareX/blob/develop/README.md)) | ❓ unverified |
| Pin-to-screen | ❌ removed ([ADR-0049](../adrs/ADR-0049-remove-pin-to-desktop.md)) | ✅ ([#954](https://github.com/flameshot-org/flameshot/issues/954) tracks improvements) | ❌ | ✅ (both) | ✅ ([docs](https://getsharex.com/docs/pin-to-screen)) | ❌ |
| GIF / screen recording | ❌ non-goal | ❌ [#172](https://github.com/flameshot-org/flameshot/issues/172) | ❌ | ✅ (both) | ✅ | ❌ (not on [app page](https://apps.kde.org/spectacle/)) |
| Capture history | ⚠️ session store + recents index, in-process ([ADR-0067](../adrs/ADR-0067-session-capture-store.md)) | ❌ | ❌ | ✅ CleanShot "Capture History" ([cleanshot.com](https://cleanshot.com)) | ❓ unverified | ❌ |
| Upload beyond Imgur / custom | ⚠️ Imgur only; trait ready | ⚠️ [#50](https://github.com/flameshot-org/flameshot/issues/50) | ❌ | ✅ CleanShot Cloud, Shottr S3 ([v1.9 notes](https://shottr.cc)) | ✅ custom uploaders ([docs](https://getsharex.com/docs/custom-uploader)) | n/a |
| After-capture automation | ⚠️ `--after` flag; full pipeline queued | ⚠️ | ⚠️ `actions-on-enter/escape` ([README](https://github.com/gabm/Satty)) | ✅ | ✅ workflows ([README](https://github.com/ShareX/ShareX/blob/develop/README.md)) | ❌ |
| Edit existing image | ✅ `flintshot open` | ❌ [#240](https://github.com/flameshot-org/flameshot/issues/240) (176👍, their #1) | ✅ | ✅ | ✅ | ❓ unverified |
| AI transforms on the shot | ✅ | ❌ | ❌ | ❌ | ❌ ([#8348](https://github.com/ShareX/ShareX/issues/8348) closed "No AI version") | ❌ |
| Backdrop / padding / beautify | ❌ | ❌ | ❌ | ✅ (both) | ✅ "image beautifier" ([README](https://github.com/ShareX/ShareX/blob/develop/README.md)) | ❌ |
| Ruler / measure | ❌ (magnifier only) | ❌ | ❌ | ✅ Shottr ([shottr.cc](https://shottr.cc)) | ✅ | ❌ |
| Screen color picker | ❌ | ⚠️ palette picker exists (issues track its UX) | ✅ palette | ✅ Shottr TAB-copy ([shottr.cc](https://shottr.cc)) | ✅ | ❌ |
| Wayland-first (portal capture + GlobalShortcuts) | ✅ | ⚠️ chronic breakage ([#3700](https://github.com/flameshot-org/flameshot/issues/3700) 93👍, [#3365](https://github.com/flameshot-org/flameshot/issues/3365) 80👍) | ⚠️ wlroots-focused | n/a | n/a | ✅ |

Read: flintshot's gaps cluster exactly where flameshot's top open issues
cluster (window, scrolling, cursor, upload breadth) — plus the power-user
polish tier (backdrop, ruler, QR) that the commercial tools monetize.

## 3. Candidates in detail

### 3.1 Window / active-window capture — M

**What.** `flintshot capture --window` (pick) and `--active-window`; the
VISION feature surface already lists "active window" — it's the one promise
still unshipped.

**Evidence.** flameshot [#5](https://github.com/flameshot-org/flameshot/issues/5)
"Selecting a window in GUI mode" — 122👍, 49 comments, issue #5, open since
the project's first days; [#334](https://github.com/flameshot-org/flameshot/issues/334)
"Capture focused window" adds 34👍. Spectacle ships active-window *and*
window-under-mouse ([apps.kde.org](https://apps.kde.org/spectacle/)); ShareX
and Shottr ship active-window capture.

**Mapping.** The `Capturer` trait (ADR-0050) grows a window path. xcap
supports `Window::all()` + `window.capture_image()` on Linux-X11, macOS,
Windows — Wayland is explicitly ⛔ in its
[implementation table](https://github.com/nashaofu/xcap). So: xcap backend
serves X11/Win/macOS; on Wayland the portal backend already composites all
outputs at logical coordinates (ADR-0050 spike) — a window *pick* degrades to
today's interactive region selection, which is honest parity. Optional later:
`ext-foreign-toplevel-list` via `wayland-client` for window rects on
wlroots/COSMIC compositors.

**Risks.** Multi-monitor coordinate mapping (flameshot's longest-lived bug
class — [#4969](https://github.com/flameshot-org/flameshot/issues/4969),
[#4928](https://github.com/flameshot-org/flameshot/issues/4928) are *this
week's* regressions there); test on mixed-DPI first. No VISION conflict —
it *is* the vision.

### 3.2 Upload breadth + config-driven custom uploader — S per backend, M generic

**What.** (a) One or two more built-ins (`0x0.st`, `catbox.moe`-style
multipart POST); (b) a config-defined generic HTTP backend: URL, method,
form fields, auth header from keyring, response → URL extraction.

**Evidence.** The upload cluster is flameshot's #9/#12/#16 open issues:
[#50](https://github.com/flameshot-org/flameshot/issues/50) (40👍),
[#499](https://github.com/flameshot-org/flameshot/issues/499) (29👍),
[#27](https://github.com/flameshot-org/flameshot/issues/27) (21👍) — ~90👍
combined, unmet since 2016. ShareX's answer (custom-uploader definitions,
[docs](https://getsharex.com/docs/custom-uploader)) is a documented retention
moat; Shottr's 2025 headline feature was S3 ([v1.9](https://shottr.cc)).

**Mapping.** Zero new plumbing: `Backend` trait + `flintshot auth` +
keyring all exist. (b) is the ShareX custom-uploader pattern done in TOML;
it also covers self-hosters, keeping the "no self-hosted image hosting"
non-goal intact (we ship the client, not the service).

**Risks.** Each third-party host needs a ToS/robustness check; response
parsing needs a small selector grammar (URL JSONPath-ish) — keep it to
"JSON pointer or regex" to avoid a config language.

### 3.3 Cursor in the capture — S–M

**What.** Honor `include_cursor` end-to-end and expose a `[capture] cursor`
config/flag.

**Evidence.** flameshot [#604](https://github.com/flameshot-org/flameshot/issues/604)
— 119👍, 44 comments. flintshot already threads the option; the portal
backend documents that `org.freedesktop.portal.Screenshot` exposes no cursor
option (`handle_token`, `modal`, `interactive` only — see
`capture/portal.rs` and [ADR-0050](../adrs/ADR-0050-capturer-trait-native-backends.md)).

**Mapping.** On the xcap path: composite the OS cursor sprite at the live
pointer position after the grab. Cursor *position* is cheap (`enf`-style
query or per-platform API); the sprite needs cursor-theme loading
(`libxcursor` on X11, `Cursor` APIs on Win/mac) — or a bundled fallback
arrow if fidelity matters less than correctness. Whether xcap itself
composites the cursor on any platform is **unverified** — spike first.

**Risks.** Wayland portal path stays cursor-less by protocol; document it
(the VISION's "honesty about platform limits" principle). HiDPI cursor
scaling is the classic wart.

### 3.4 QR decode from selection — S

**What.** Editor/overlay action + after-capture step: decode QR (and later
barcodes) in the selection, copy text/open URL.

**Evidence.** flameshot [#511](https://github.com/flameshot-org/flameshot/issues/511)
(22👍, 19 comments). Shottr's "OCR feature also reads QR codes"
([shottr.cc](https://shottr.cc)); CleanShot "Read QR codes instantly"
([cleanshot.com](https://cleanshot.com)); ShareX ships a QR tool
([README](https://github.com/ShareX/ShareX/blob/develop/README.md)).

**Mapping.** [`rqrr` 0.11.0](https://crates.io/crates/rqrr) — pure Rust,
4.88M downloads, updated 2026-09-02 (crates.io API, checked today);
[`bardecoder`](https://crates.io/crates/bardecoder) is the stale fallback
(2023). Feed the already-cropped region; `image` → gray → `rqrr`. Slots
straight into the queued after-capture pipeline as an action.

**Risks.** None material; license (rqrr is MIT) and done.

### 3.5 Backdrop: padding, background, rounded corners, drop shadow — S–M

**What.** One-step "pretty export": pad the shot on a solid/gradient
background, round corners, optional shadow — the default look of every
modern doc screenshot.

**Evidence.** Shottr "Beautiful Backgrounds … gradients backgrounds,
shadows and rounded corners" + v1.8 Backdrop Tool
([shottr.cc](https://shottr.cc)); CleanShot "Background Tool … beautiful
backgrounds, padding, shadows" ([cleanshot.com](https://cleanshot.com));
ShareX "image beautifier" ([README](https://github.com/ShareX/ShareX/blob/develop/README.md)).
Closest flameshot ask: [#313](https://github.com/flameshot-org/flameshot/issues/313)
"Add shadows" (23👍).

**Mapping.** Pure `image`-crate compositing on export (or as a live
non-destructive annotation kind — the `Annotation` enum is extensible by
design). Shadow = blurred black rounded rect; rounded corners = alpha mask.
No new deps.

**Risks.** Only if done live: export-size semantics (padding changes canvas
size — needs an explicit "backdrop is the canvas, not a shape" model).

### 3.6 Scrolling capture — L, defer with a decision

**What.** Capture a scrollable region taller than the viewport: synthesize
scroll, re-grab, stitch.

**Evidence.** The strongest unmet demand in the entire space: flameshot
[#1130](https://github.com/flameshot-org/flameshot/issues/1130) (137👍, open
since 2019). Shipped by CleanShot ("full pages, long chats, documents"),
Shottr (any window, up to 200,000px), ShareX
([docs](https://getsharex.com/docs/scrolling-screenshot)); Greenshot only
ever managed IE-era scrolling ([getgreenshot.org](https://getgreenshot.org)).

**Why defer.** It needs *input synthesis* (scroll events) + repeated
capture + overlap-stitching, and the input half is the killer:
- X11: XTest scroll — doable.
- macOS: CGEvent scroll — doable (Shottr/CleanShot prove it).
- Windows: SendInput — doable (ShareX proves it).
- **Wayland: no path.** Portals offer no scroll synthesis; the closest,
  `InputCapture`, exists for remote-desktop control flows, not
  screenshot helpers (interface list:
  [xdg-desktop-portal data/](https://github.com/flatpak/xdg-desktop-portal/tree/main/data)).

A feature that is Linux-X11-only on our primary platform inverts the
product's parity story. If pursued: post-1.0, macOS+X11 first, stitching
as a standalone pure-Rust piece (`image` + column-diff overlap search) that
ships value even before input synthesis is trusted.

### 3.7 Eyedropper + ruler off the magnifier — S

**Evidence.** Shottr's color picker ("press TAB to copy color under the
cursor") and ruler (arrow-key measuring, imprint on click) are advertised
features ([shottr.cc](https://shottr.cc)); ShareX ships "color picker",
"screen color picker", "ruler" ([README](https://github.com/ShareX/ShareX/blob/develop/README.md));
Snipaste lists a color picker in snipping mode ([snipaste.com](https://www.snipaste.com)).
flameshot-side demand is weak (top color-picker issue only 5👍 —
[#3669](https://github.com/flameshot-org/flameshot/issues/3669)) — this is
polish parity, not demand-driven.

**Mapping.** The magnifier already renders pixels under the cursor; add
copy-hex/rgb (clipboard write exists), and edge-to-edge measure lines with a
px readout in the overlay. S.

### 3.8 `--print-geometry` — S

**What.** `flintshot capture --print-geometry`: run the selection UI, print
`X,Y WxH` to stdout, grab nothing.

**Evidence.** slurp is *defined* by this contract ("Select a region in a
Wayland compositor and print it to the standard output",
[README](https://github.com/emersion/slurp)); grim consumes it (`grim -g
"$(slurp)"`, [README](https://github.com/emersion/grim)); Satty's own
integration docs assume the grim/slurp pipeline
([README](https://github.com/gabm/Satty)).

**Mapping.** The overlay already computes the selection rect; emit it and
exit. Makes flintshot composable with grim / wl-screenrecorder / any
`-g`-style tool instead of competing with them. VISION-aligned (honest
Wayland ecosystem citizen).

### 3.9 Re-slice, not new: history gallery before OCR index

The queued "searchable history" is L because it needs background OCR + an
index + search UI. CleanShot's Capture History ships as a plain gallery
first ([cleanshot.com](https://cleanshot.com)). flintshot already has the
`SessionStore` (ADR-0067) and the recents index — a persistent
capture-gallery page over them is M and de-risks the L. Worth folding into
the OCR effort's task breakdown.

## 4. Explicitly rejected

| Idea | Why rejected |
| --- | --- |
| **Screen recording / GIF capture** (flameshot [#172](https://github.com/flameshot-org/flameshot/issues/172), 126👍) | VISION non-goal ("use OBS"). ROADMAP already floats `gpu-screen-recorder` integration — the right shape is *integration*, never in-tree encoding. Re-evaluation trigger: xcap's video recording leaves WIP ([README](https://github.com/nashaofu/xcap)) and stays non-goal regardless. |
| **Pin-to-screen / floating image windows** (Snipaste's core; flameshot [#954](https://github.com/flameshot-org/flameshot/issues/954)) | Removed by [ADR-0049](../adrs/ADR-0049-remove-pin-to-desktop.md): no always-on-top for ordinary toplevels on Wayland; a real fix means layer-shell windows (per-compositor testing, a second windowing path beside eframe). Cost reconfirmation: Shottr/CleanShot ship it only because they're macOS-only. |
| **Self-hosted image hosting / CleanShot-style cloud** | VISION non-goal. The config-driven custom uploader (§3.2) is the sanctioned escape hatch. |
| **Mobile / WASM targets** | VISION non-goals, unchanged. |
| **Bundled OCR engine** (linking tesseract/leptonica or ONNX models) | VISION says OCR stays external integration; the queued CLI-wrapper approach is the compliant shape. Revisit only if the system `tesseract` dep demonstrably hurts packaging (per the 09-26 note §6). |
| **Emoji/sticker tool** (flameshot [#764](https://github.com/flameshot-org/flameshot/issues/764), 25👍) | Not a non-goal conflict — just weak fit: needs color-emoji font fallbacks everywhere (Satty needed GTK fallback chains for it). Reconsider if a text-tool font-fallback effort happens anyway. |

## 5. Sources

Issue reaction counts pulled 2026-10-05 via the GitHub search API
(`sort=reactions-+1-desc`, open issues only).

- **flameshot** issues (all open): [#5](https://github.com/flameshot-org/flameshot/issues/5) window-select 122👍 · [#240](https://github.com/flameshot-org/flameshot/issues/240) edit-existing 176👍 · [#172](https://github.com/flameshot-org/flameshot/issues/172) GIF 126👍 · [#334](https://github.com/flameshot-org/flameshot/issues/334) focused-window 34👍 · [#604](https://github.com/flameshot-org/flameshot/issues/604) cursor 119👍 · [#702](https://github.com/flameshot-org/flameshot/issues/702) OCR 61👍 · [#1130](https://github.com/flameshot-org/flameshot/issues/1130) scrolling 137👍 · [#50](https://github.com/flameshot-org/flameshot/issues/50) 40👍 / [#499](https://github.com/flameshot-org/flameshot/issues/499) 29👍 / [#27](https://github.com/flameshot-org/flameshot/issues/27) 21👍 upload breadth · [#511](https://github.com/flameshot-org/flameshot/issues/511) QR 22👍 · [#313](https://github.com/flameshot-org/flameshot/issues/313) shadows 23👍 · [#764](https://github.com/flameshot-org/flameshot/issues/764) emoji 25👍 · [#954](https://github.com/flameshot-org/flameshot/issues/954) pin 21👍 · Wayland breakage [#3700](https://github.com/flameshot-org/flameshot/issues/3700) 93👍, [#3365](https://github.com/flameshot-org/flameshot/issues/3365) 80👍 · this week's multi-monitor regressions [#4969](https://github.com/flameshot-org/flameshot/issues/4969), [#4928](https://github.com/flameshot-org/flameshot/issues/4928) · ShareX "No AI version" [#8348](https://github.com/ShareX/ShareX/issues/8348) (closed)
- **Satty**: [README (gabm/Satty)](https://github.com/gabm/Satty) — toolset, actions-on-enter, grim/slurp integration examples; MPL-2.0
- **Shottr**: [shottr.cc](https://shottr.cc) — feature list + release notes (v1.5.0 scrolling/active-window/OCR, v1.5.2 QR reader, v1.8 Backdrop, v1.9 S3)
- **CleanShot X**: [cleanshot.com](https://cleanshot.com) — scrolling capture, Capture History, pin, backgrounds, OCR/QR, recording
- **ShareX**: [README (ShareX/ShareX, develop)](https://github.com/ShareX/ShareX/blob/develop/README.md) + [scrolling screenshot](https://getsharex.com/docs/scrolling-screenshot), [OCR](https://getsharex.com/docs/ocr), [custom uploader](https://getsharex.com/docs/custom-uploader), [pin to screen](https://getsharex.com/docs/pin-to-screen)
- **Greenshot**: [getgreenshot.org](https://getgreenshot.org) — region/window/fullscreen, IE scrolling, annotate/obfuscate, export destinations
- **Spectacle**: [apps.kde.org/spectacle](https://apps.kde.org/spectacle/) — capture modes incl. active window and window under mouse; [invent.kde.org/plasma/spectacle](https://invent.kde.org/plasma/spectacle) README (KWin permission env vars)
- **Snipaste**: [snipaste.com](https://www.snipaste.com) — pin workflow (F1/F3), color picker, history playback
- **Wayland ecosystem**: [slurp README](https://github.com/emersion/slurp) (geometry-to-stdout contract) · [grim README](https://github.com/emersion/grim) (`-g` consumer, per-output/region grabs)
- **xdg-desktop-portal**: [README](https://github.com/flatpak/xdg-desktop-portal/blob/main/README.md) + [data/ interface XMLs](https://github.com/flatpak/xdg-desktop-portal/tree/main/data) (Screenshot, ScreenCast, GlobalShortcuts, FileChooser, InputCapture confirmed in-tree)
- **xcap**: [README (nashaofu/xcap)](https://github.com/nashaofu/xcap) — implementation-status table (window capture ⛔ Wayland; video recording WIP)
- **Crates** (crates.io API, checked 2026-10-05): [auto-launch 0.6.0](https://crates.io/crates/auto-launch) (2026-01-10) · [rqrr 0.11.0](https://crates.io/crates/rqrr) (2026-09-02) · [bardecoder 0.5.0](https://crates.io/crates/bardecoder) (2023-07-29)
- **Internal**: [VISION.md](../VISION.md) (non-goals), [ROADMAP.md](../ROADMAP.md) (queued work), [ADR-0049](../adrs/ADR-0049-remove-pin-to-desktop.md) (pin removal), [ADR-0050](../adrs/ADR-0050-capturer-trait-native-backends.md) (Capturer trait, portal spike), [ADR-0067](../adrs/ADR-0067-session-capture-store.md) (session store), `crates/flintshot-core/src/capture/portal.rs` (cursor/interactive options), `crates/flintshot-app/src/cli.rs` (current command surface)
