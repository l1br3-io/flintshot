# Next valuable additions — what to build after the current queue

Research notes, 2026-10-10. Primary sources only: upstream trackers via the
GitHub/GitLab APIs (reaction counts pulled today — see §5 for method),
crates.io API, official docs, first-party repo listings. Scope: additions
that are (a) not shipped, (b) not queued in [`../ROADMAP.md`](../ROADMAP.md),
(c) not already proposed in
[`2026-10-05-feature-gap-analysis.md`](2026-10-05-feature-gap-analysis.md) or
[`2026-10-07-ai-capability-features.md`](2026-10-07-ai-capability-features.md),
(d) not a [`../VISION.md`](../VISION.md) non-goal, (e) evidenced.

Headline findings outside the candidates: **zero releases have been cut**
(`git tag` empty, 0 GitHub Releases, both crates unpublished on crates.io —
all verified today) even though the release pipeline exists
([ADR-0068](../adrs/ADR-0068-release-pipeline.md)) and ROADMAP's earliest
`1.0.0` date (2026-09-14) is 26 days past. And a first MCP-screenshot
incumbent has appeared on crates.io
([`rmcp-screenshot` 0.1.0](https://crates.io/crates/rmcp-screenshot), 24
downloads, 2026-01-07 — tiny, but the queued MCP tool's "no incumbent" note
in REFERENCES.md now has a clock on it).

## 1. TL;DR — ranked candidates

Ranked by (evidence-backed demand × strategic fit) ÷ effort.

| # | Candidate | Evidence (primary, pulled 2026-10-10) | Effort | Verdict |
| --- | --- | --- | --- | --- |
| 1 | **Ship the first release + finish the install ring (winget, cargo-binstall, AUR)** | 0 tags/0 releases/0 crates.io today; winget-pkgs carries [ShareX ≤21.0.0](https://github.com/microsoft/winget-pkgs/tree/master/manifests/s/ShareX/ShareX), [Greenshot ≤1.3.323](https://github.com/microsoft/winget-pkgs/tree/master/manifests/g/Greenshot/Greenshot), [Flameshot ≤14.0.0](https://github.com/microsoft/winget-pkgs/tree/master/manifests/f/Flameshot/Flameshot); [Satty](https://github.com/Satty-org/Satty) documents `cargo binstall satty`; [cargo-dist docs](https://github.com/axodotdev/cargo-dist/blob/main/book/src/installers/other.md): binstall auto-detects its release URL schema | S per channel | The pipeline is built and unfired; everything else ranks against a tool nobody can install without building it |
| 2 | **Export depth: WebP (+ AVIF), format-at-save** | ShareX [#6090](https://github.com/ShareX/ShareX/issues/6090) "Save screenshots as WebP, AVIF, JPEG XL…" 41👍 + [#5250](https://github.com/ShareX/ShareX/issues/5250) static WebP 32👍 + flameshot [#506](https://github.com/flameshot-org/flameshot/issues/506) quality 36👍 = 109👍 combined | S–M | Strongest demand cluster not yet served anywhere in our stack; two crates of work |
| 3 | **Resume after crash / accidental dismiss** | flameshot [#2055](https://github.com/flameshot-org/flameshot/issues/2055) "Its very easy to lose your work" 19👍; our `SessionStore` already mirrors annotations to disk then *deletes them on restart* (`session.rs` header: "nothing resumes from a previous process's files") | S | The data is already crash-safe by construction; only the sweep policy and a restore row are missing |
| 4 | **Watermark on export** | flameshot [#158](https://github.com/flameshot-org/flameshot/issues/158) "[Feature request] Add watermark" 13👍 (open); no incumbent ships it on Linux | S | Cheap, uncited cluster; rides existing template + annotation infra |
| 5 | **Scripting surface: `--geometry` grab, `--json`, stdout pipe, completions + man** | grim's whole scripting contract is `-g "10,20 300x400"` / `grim -g "$(slurp)"` ([README](https://github.com/emersion/grim)); slurp prints geometry to stdout ([README](https://github.com/emersion/slurp)); [`clap_complete` 4.6.11](https://crates.io/crates/clap_complete) (123M downloads, updated 2026-09-15) | S | The input half of the 10-05 `--print-geometry` proposal (output half) — together flintshot becomes pipe-composable both directions |
| 6 | **PII redaction assist: AI-detect → suggest → user-applies blur** | [macshot](https://github.com/sw33tLie/macshot) (3,732★, launched 2026, macOS-only) advertises "auto-redact PII" in its repo description — first traction-backed AI-native capture feature; no Linux equivalent | M | The one AI-native feature a new entrant proved; fits our BYO-key + canvas-untouched posture if detection only *suggests* |
| 7 | **Annotation polish: aspect-ratio lock, curved arrows** | ShareX [#4381](https://github.com/ShareX/ShareX/issues/4381) aspect-ratio capture 7👍 **with sponsor bounty**; [#4227](https://github.com/ShareX/ShareX/issues/4227) more arrow shapes 8👍; Greenshot [#311](https://github.com/Greenshot/greenshot/issues/311) curved arrows 8👍 — 23👍 combined | S–M | Bounty = willingness to pay; aspect-lock first |
| 8 | **Enforce the VISION budgets in CI (binary size, cold-start)** | VISION success criteria (<25 MB binary, <200 ms cold-start) — no gate exists today (`ci.yml`/justfile: no size or bench step). External demand evidence: none found; this is honesty-about-goals hygiene, ranked accordingly | S | Size check is deterministic and cheap; cold-start = track, don't gate |

## 2. Candidates in detail

### 2.1 Ship v0.1.0 + finish the install ring — S per channel

**What.** Three moves: push the first tag through the existing cargo-dist
pipeline; submit a winget manifest (MSI already wired via `[package.metadata.wix]`
in `crates/flintshot-app/Cargo.toml`); publish `flintshot-app` to crates.io
with `[package.metadata.binstall]` so `cargo binstall flintshot` resolves to
the GitHub-Release archives, and add an AUR `-bin` package tracking releases.

**Evidence.** Verified today: `git ls-remote --tags origin` is empty, 0
GitHub Releases, `flintshot`/`flintshot-app` absent from crates.io. Every
comparable ships these channels: winget-pkgs contains ShareX (≤21.0.0),
Greenshot (≤1.3.323), Flameshot (≤14.0.0) (manifest directories checked via
the API today); Satty's README lists `cargo install satty`, `cargo binstall
satty`, distro packages; grim/slurp/flameshot live in Arch `[extra]` (Arch
package API today) with `normcap` (26 votes) and `satty-git` on AUR.
cargo-dist's own docs state binstall compatibility is automatic: "cargo-binstall
(the URL schema we use for Github Releases is auto-detected)"
([book/src/installers/other.md](https://github.com/axodotdev/cargo-dist/blob/main/book/src/installers/other.md)).

**Mapping.** Zero new build work — `dist-workspace.toml` already emits
shell/powershell/msi/homebrew installers and the deb post-announce job
(`.github/workflows/release.yml`, `packaging/`). The winget step is a YAML
manifest PR against `microsoft/winget-pkgs` referencing our MSI. The binstall
step is a crates.io `cargo publish` (resolution only; document that plain
`cargo install` needs the XCAP Linux system deps and point Linux users at the
installer script / deb). AUR: a `-bin` PKGBUILD over the release tarballs
(pure-Rust note: tarball, not source build, for the same system-deps reason).

**Risks.** winget wants stable upgrade behavior — our wix upgrade-guid
already pins that. First release resets the "no API stability guarantees"
posture: cut `0.1.x`, not `1.0.0`, until dogfooding gate is honest.

### 2.2 Export depth: WebP + AVIF, format-at-save — S–M

**What.** Extend `OutputFormat` with WebP (and optionally AVIF); let the
Save dialog override format per-save (today the rfd path prompts for a "PNG
save path", `app.rs` Save chip, and `capture --out` documents "Format is
always PNG regardless of the extension", `cli.rs`).

**Evidence.** ShareX [#6090](https://github.com/ShareX/ShareX/issues/6090)
(41👍) asks exactly for "WebP, AVIF, JPEG XL and other formats";
[#5250](https://github.com/ShareX/ShareX/issues/5250) (32👍) re-asks for
static WebP; flameshot [#506](https://github.com/flameshot-org/flameshot/issues/506)
(36👍) is the quality/slider version of the same want. 109👍 combined,
uncited by either prior note. flintshot today: PNG + JPEG-with-quality only
(`OutputFormat` in `crates/flintshot-core/src/config.rs:613`, encoder in
`crates/flintshot-core/src/output.rs` `save_capture_image`, two-entry picker
in `settings.rs`).

**Mapping.** The `image` crate already in-tree carries both encoders behind
features: `webp = dep:image-webp` and `avif = dep:ravif` (image
[Cargo.toml features](https://github.com/image-rs/image/blob/main/Cargo.toml)).
Verified on docs.rs (image 0.25.10): `WebPEncoder` is **lossless-only**
("Right now only lossless encoding is supported", sole constructor
`new_lossless`) — still the chat-app file-size story (lossless WebP runs
~25–30% under PNG); lossy would need libwebp bindings, rejected as non-pure-
Rust. AVIF via `ravif` encodes but slowly without the `nasm` feature.
Extend `OutputFormat` + `output_ext` + `save_capture_image`, add dialog
format presets beside the PNG path.

**Risks.** JPEG XL has **no** pure-Rust encoder ([`jxl-oxide` 0.12.6](https://crates.io/crates/jxl-oxide)
is decode-only) — reject JXL, say so in the dialog. AVIF encode time should
be measured before committing to ship it.

### 2.3 Resume after crash / accidental dismiss — S

**What.** When the app window starts and finds a previous process's session
dir, offer the leftover captures (image + committed annotations) on Home
instead of deleting them.

**Evidence.** flameshot [#2055](https://github.com/flameshot-org/flameshot/issues/2055)
(19👍, open): "I can dedicate 10 minutes adding numbers, text etc, just so
that one ESC extra … can close everything without an option to restore."
Our own `SessionStore` docs (`crates/flintshot-core/src/session.rs`) say
entries are mirrored to `<dir>/<id>.png` + `<id>.json` precisely so
"annotations are crash-safe by construction" — and then: "startup sweeps any
leftover dir from a crashed run … nothing resumes from a previous process's
files." The safety exists; the resume half was never built.

**Mapping.** `SessionStore::sweep_stale` (`crates/flintshot-app/src/app_window.rs:965`)
becomes a *quarantine* (move aside, keep TTL), `SessionSummary`/the Home
bridge (`crates/flintshot-editor-egui/src/home.rs`) gains a "Previous
session" row that re-enters the editor with the sidecar's `anns` —
`Annotation` is already serde round-trippable.

**Risks.** Privacy: unsaved screen content persisting indefinitely — keep
the sweep for entries older than N days and make Discard explicit on the
restore row. Worker-process editors (daemon-spawned) don't touch the store —
scope to the app-window path where the store lives.

### 2.4 Watermark on export — S

**What.** Optional text watermark (e.g. handle + `{date}`) burned in on
save/upload — corner or tiled, semi-transparent.

**Evidence.** flameshot [#158](https://github.com/flameshot-org/flameshot/issues/158)
(13👍, open) is a decade-old unmet ask; none of Satty/Spectacle/Greenshot
advertise it (checked their READMEs/pages — CleanShot/Shottr marketing was
re-checked by the 10-07 note). ShareX's equivalent is its generic
image-effects framework, not a one-click watermark — **unverified** as a
named ShareX feature, so flameshot #158 is the demand evidence.

**Mapping.** Either an export-time step beside `save_capture_image`
(composite text via the existing `cosmic-text` path in `crates/flintshot-core/src/text.rs`,
position/opacity from `[output] watermark` config reusing `render_filename_template`'s
placeholder substitution) or a non-destructive `Annotation::Watermark`
variant — the enum is extensible by design. The export-step shape avoids
polluting undo history with what is really an output policy.

**Risks.** Upload path must apply it too or users will trust it and leak
unwatermarked originals — make one code path serve save + upload.

### 2.5 Scripting surface: `--geometry`, `--json`, stdout, completions + man — S

**What.** `flintshot capture --geometry "10,20 300x40" --out -` (exact-rect
headless grab, PNG to stdout); `--json` on `backends`/`config`; an
`flintshot events` JSON-lines stream of daemon broadcasts; `clap_complete`
shell completions and a man page.

**Evidence.** grim's README defines the composability contract
(`grim -g "10,20 300x400"`, `grim -g "$(slurp)"`); the 10-05 note proposed
the *output* half (`--print-geometry`, slurp-style) — the *input* half (act
on a known geometry, no selection UI) is unproposed and is what makes the
pair a two-way pipe. Our `Event` enum (`crates/flintshot-core/src/ipc.rs`)
already serializes and `ipc_client::subscribe_to` already streams it — only
the CLI surface is missing. `clap_complete` 4.6.11: 123M downloads, updated
2026-09-15 (crates.io API today). Satty's CLI carries `--man` because
cargo-installed copies lack docs — same lesson.

**Mapping.** `CaptureArgs` grows `--geometry`; headless exact-rect capture
goes through the existing fallback backend's monitor-grab-then-crop
(`capture_prepared` + a crop against `image_ops`), bypassing the editor —
mirrors the `--out` headless path so no IPC changes (the `--after`
"cannot-carry-it" precedent in `cli.rs`). `--out -` special-cases
`write_png` to stdout. Completions: one `complete` subcommand on `Cli`.

**Risks.** Geometry is backend-tricky on Wayland portal captures (logical vs
physical px) — restrict v1 to the non-portal backends, document honestly.

### 2.6 PII redaction assist — M

**What.** A "Find sensitive regions" action: the configured BYO-key vision
model returns candidate rects (emails, tokens, faces), rendered as proposed
`Blur` annotations the user accepts/edits before anything is committed.

**Evidence.** [macshot](https://github.com/sw33tLie/macshot) — 3,732★ since
launch in 2026 (stars pulled today) — headlines "auto-redact PII"; it is
macOS/Swift-only, so the feature has no Linux provider. This is the only
AI-native capture feature with demonstrated traction that neither prior note
covered (10-07 §2 re-verified incumbents had nothing AI-shaped; macshot is
new since).

**Mapping.** Runs entirely through shipped machinery: an `AiTransform`
variant whose answer is parsed to rects (structured output was designed, not
yet shipped — 10-07 §4.1 gives the `response_format` plan), then rects →
`Annotation::Blur` proposals in the editor. ADR-0069's boundary holds
detection is a user-initiated network call, mutation is local and
user-confirmed.

**Risks.** A missed region reads as "checked, safe" — the UI must say
"suggestions", never "verified clean"; hallucinated boxes are merely
annoying. Screen content is untrusted input to the model (prompt-injection
via the screenshot itself); the transform must not act on instructions found
*in* the image. Auto-apply mode is rejected (§3).

### 2.7 Annotation polish: aspect-ratio lock, curved arrows — S–M

**What.** Shift/option-constrain the selection (and crop marquee) to a ratio
(free, 16:9, 1:1 presets); a curved-arrow variant.

**Evidence.** ShareX [#4381](https://github.com/ShareX/ShareX/issues/4381)
(7👍 + "I'll sponsor a bounty"), [#4227](https://github.com/ShareX/ShareX/issues/4227)
(8👍, more arrow shapes incl. curved), Greenshot [#311](https://github.com/Greenshot/greenshot/issues/311)
(8👍, curved arrows) — 23👍 combined, none cited before.

**Mapping.** Aspect lock is a constraint in the selection/crop in-flight
state (`editor.rs` marquee/crop fields) plus a modifier check in the shell's
pointer-drag pump — small and pure. Curved arrow = an `Annotation::Arrow`
sibling with a quadratic control point; the `Canvas` trait's draw path
renders the bezier.

**Risks.** Scope creep into an "annotation pack"; ship aspect-lock alone
first (it has the bounty).

### 2.8 Enforce the VISION budgets in CI — S, hygiene

**What.** A release-artifact size assertion (fail >25 MB stripped, per
VISION) in the release workflow, and a cold-start timing tracked (not
gated) per release via `hyperfine`.

**Evidence.** Intrinsic: VISION success criteria name both numbers; today
nothing measures or enforces either (`ci.yml`/justfile have no size/bench
step — verified by inspection). External demand evidence: none found — this
is ranked last *because* it is hygiene, per the honest-ranking rule.

**Mapping.** Size gate: a step in `release.yml`'s artifacts job summing
stripped-binary sizes (ADR-0068 owns that workflow). Cold-start: a
`just bench-start` hyperfine script over `flintshot capture --out` on the
release runner, results pasted into the release notes.

**Risks.** Shared-runner timings are noisy — hence track-not-gate; size
gates can pressure dropping debug symbols users may want — compare stripped
sizes only.

## 3. Explicitly rejected

| Idea | Why rejected |
| --- | --- |
| **Paste image from clipboard into the editor** | Already shipped: `request_paste` in `crates/flintshot-editor-egui/src/shell.rs` ("The editor chrome's Paste action: read the clipboard"). Demand corroboration only (flameshot [#2005](https://github.com/flameshot-org/flameshot/issues/2005) 7👍, Greenshot [#107](https://github.com/Greenshot/greenshot/issues/107) 10👍, Satty `--from-clipboard`). |
| **Drag-drop open / recent files** | `flintshot open` + `recents` + Home's rfd prompt (`home.rs`) cover it; 10-05 matrix already scored edit-existing as shipped. |
| **Crop tool, zoom & pan, per-annotation re-edit, fill opacity** | All shipped: `Tool::Crop`/`Tool::Hand` + wheel zoom (`app.rs` `zoom_at_cursor`), double-click re-edit (`dblclick.rs`) + inspector property mutators (`inspector.rs`), `fill: Option<Color>` with alpha (`editor.rs`). |
| **i18n groundwork now** | Evidence is supply-side only: flameshot ships 49 translation files (`data/translations`, counted today) — but zero demand signal from *our* constituency, and extraction across editor-egui's inline strings is L, not M. Crates are healthy when needed ([`rust-i18n` 4.2.4](https://crates.io/crates/rust-i18n) 2026-09-29, [`fluent` 0.17.0](https://crates.io/crates/fluent)). Revisit post-1.0 when a non-English user asks. |
| **Snap package** | Flathub already queued (ROADMAP stretch); channel redundancy. flameshot's snap pain ([#3119](https://github.com/flameshot-org/flameshot/issues/3119) 42👍) is evidence about snap's breakage class, not its value. |
| **Screen recording** (Spectacle shipped recording in 2025–26; its tracker's recording-settings MRs !542/!556 are live) | VISION non-goal regardless of who else ships it; ROADMAP already floats `gpu-screen-recorder` *integration* as the only sanctioned shape. |
| **`ext-foreign-toplevel-list-v1` window listing** | The 10-05 window-capture candidate's own "optional later" note — re-proposal by another name; nothing beyond it unlocks. |
| **Auto-apply redaction (macshot-style)** | Violates the AI posture (ADR-0069 + 10-07's "canvas never touched"): AI may *suggest* rects; only the user commits them (§2.6 keeps that line). |
| **Multi-monitor profiles / `--monitor`** | flameshot [#4894](https://github.com/flameshot-org/flameshot/issues/4894) is 2👍 — their multi-monitor pain is a correctness-regression class, not a feature gap; our single-Capture-action design deliberately omits `--monitor` (`cli.rs` CaptureArgs doc). |
| **"Use last region" memory** | flameshot [#4911](https://github.com/flameshot-org/flameshot/issues/4911) 0👍 — real nicety, no evidence; fold into a future capture-options pass, not a ranked candidate. |
| **JPEG XL export** | No pure-Rust encoder: `jxl-oxide` (0.12.6, 2026-05-29) is decode-only; `image` has no `jxl` feature. WebP/AVIF (§2.2) is the shippable subset of ShareX #6090's ask. |
| **Copy-as-HTML/Markdown embed, file-size budgets UI** | No demand evidence found in any tracker searched; would be invention, not evidence. |
| **Project website** | Marketing surface with no primary demand evidence; the README + release notes cover the discoverability that matters pre-1.0. |

## 4. Landscape deltas since the prior notes

- **macshot** ([sw33tLie/macshot](https://github.com/sw33tLie/macshot), 3,732★, 2026): macOS-native,
  AI-era entrant; its novel-vs-us feature is auto-redact PII (→ §2.6).
  Everything else it advertises (annotate, record, scrolling, OCR) we have
  queued, shipped, or rejected already.
- **Satty** moved to `Satty-org/Satty` (2,445★, pushed 2026-10-08); REFERENCES
  link should be updated opportunistically.
- **rmcp-screenshot** 0.1.0 on crates.io (2026-01-07, 24 downloads): the
  first Rust MCP screenshot server — negligible adoption, but the queued MCP
  capture tool now has a competitor; sooner beats first.
- **Spectacle** gained screen recording (tracker MRs !542/!556): non-goal for
  us, noted only to keep §3 honest.
- flintshot's AI niche remains uncontested: CleanShot/Shottr re-checked by
  the 10-07 note; macshot's AI surface is redaction, not ask-the-shot.

## 5. Sources

Reaction counts pulled 2026-10-10. Method note: the GitHub search API's
`sort=reactions-+1-desc` is currently **ignored server-side** (returns
newest-first, with and without auth) — worked around by pulling all open
issues per repo via the REST list-issues endpoint and sorting by 👍 locally.
Repo counts (stars, contents, translations) via the GitHub contents/search
APIs; winget presence via `microsoft/winget-pkgs` manifest directories;
Arch presence via archlinux.org package JSON + AUR RPC v5; crate facts via
the crates.io API — all today.

- **flameshot** (all open, 👍 as of 2026-10-10): [#240](https://github.com/flameshot-org/flameshot/issues/240) 176 · [#1130](https://github.com/flameshot-org/flameshot/issues/1130) 137 · [#172](https://github.com/flameshot-org/flameshot/issues/172) 126 · [#5](https://github.com/flameshot-org/flameshot/issues/5) 122 · [#604](https://github.com/flameshot-org/flameshot/issues/604) 119 · [#3700](https://github.com/flameshot-org/flameshot/issues/3700) 93 · [#3365](https://github.com/flameshot-org/flameshot/issues/3365) 80 · [#702](https://github.com/flameshot-org/flameshot/issues/702) 61 (queued OCR) · [#3119](https://github.com/flameshot-org/flameshot/issues/3119) 42 · [#50](https://github.com/flameshot-org/flameshot/issues/50) 40 · **[#506](https://github.com/flameshot-org/flameshot/issues/506) 36 (§2.2)** · [#334](https://github.com/flameshot-org/flameshot/issues/334) 34 · **[#2055](https://github.com/flameshot-org/flameshot/issues/2055) 19 (§2.3)** · **[#158](https://github.com/flameshot-org/flameshot/issues/158) 13 (§2.4)** · [#2005](https://github.com/flameshot-org/flameshot/issues/2005) 7 · [#4894](https://github.com/flameshot-org/flameshot/issues/4894) 2 · [#4911](https://github.com/flameshot-org/flameshot/issues/4911) 0 · 49 translation files in `data/translations`
- **ShareX** (open): [#6625](https://github.com/ShareX/ShareX/issues/6625) 68 · **[#6090](https://github.com/ShareX/ShareX/issues/6090) 41 (§2.2)** · **[#5250](https://github.com/ShareX/ShareX/issues/5250) 32 (§2.2)** · **[#4381](https://github.com/ShareX/ShareX/issues/4381) 7 + bounty (§2.7)** · **[#4227](https://github.com/ShareX/ShareX/issues/4227) 8 (§2.7)**
- **Greenshot** (open): **[#311](https://github.com/Greenshot/greenshot/issues/311) 8 (§2.7)** · [#107](https://github.com/Greenshot/greenshot/issues/107) 10 · [#635](https://github.com/Greenshot/greenshot/issues/635) 10 (choco channel context) · [#673](https://github.com/Greenshot/greenshot/issues/673)/[#842](https://github.com/Greenshot/greenshot/issues/842) MSI-for-enterprise (we ship MSI)
- **New entrants**: [sw33tLie/macshot](https://github.com/sw33tLie/macshot) 3,732★ (description: "annotate, auto-redact PII, record") · [Satty-org/Satty](https://github.com/Satty-org/Satty) 2,445★, pushed 2026-10-08, MPL-2.0, README documents `cargo install`/`cargo binstall` · [rmcp-screenshot](https://crates.io/crates/rmcp-screenshot) 0.1.0, 24 downloads · [screenpipe](https://github.com/screenpipe/screenpipe) 21,891★ (active)
- **Distribution channels** (checked 2026-10-10): winget-pkgs manifests — [s/ShareX/ShareX](https://github.com/microsoft/winget-pkgs/tree/master/manifests/s/ShareX/ShareX) ≤21.0.0, [g/Greenshot/Greenshot](https://github.com/microsoft/winget-pkgs/tree/master/manifests/g/Greenshot/Greenshot) ≤1.3.323, [f/Flameshot/Flameshot](https://github.com/microsoft/winget-pkgs/tree/master/manifests/f/Flameshot/Flameshot) ≤14.0.0 · Arch: grim/slurp/flameshot in `[extra]`; AUR `normcap` 26 votes, `satty-git` · flintshot itself: `git ls-remote --tags` empty, 0 Releases, crates.io 404 on `flintshot`/`flintshot-app` · cargo-dist on binstall: [book/src/installers/other.md](https://github.com/axodotdev/cargo-dist/blob/main/book/src/installers/other.md)
- **Crates** (crates.io API, 2026-10-10): [image 0.25.10](https://crates.io/crates/image) (features `webp=dep:image-webp`, `avif=dep:ravif` — [Cargo.toml](https://github.com/image-rs/image/blob/main/Cargo.toml)); [WebPEncoder docs](https://docs.rs/image/0.25.10/x86_64-unknown-linux-gnu/image/codecs/webp/struct.WebPEncoder.html) "Right now only lossless encoding is supported" · [jxl-oxide 0.12.6](https://crates.io/crates/jxl-oxide) (decode-only) · [clap_complete 4.6.11](https://crates.io/crates/clap_complete) (2026-09-15, 123M) · [rust-i18n 4.2.4](https://crates.io/crates/rust-i18n) · [fluent 0.17.0](https://crates.io/crates/fluent)
- **Ecosystem contracts**: [grim README](https://github.com/emersion/grim) (`-g` geometry input) · [slurp README](https://github.com/emersion/slurp) (geometry to stdout)
- **Internal**: [VISION.md](../VISION.md) (non-goals, success criteria) · [ROADMAP.md](../ROADMAP.md) (queued: MCP tool, OCR, pipeline, history, presets, Flatpak, MSI/dmg) · [ADR-0067](../adrs/ADR-0067-session-capture-store.md) (session store) · [ADR-0068](../adrs/ADR-0068-release-pipeline.md) (release pipeline) · [ADR-0069](../adrs/ADR-0069-ai-provider-client.md) (AI client posture) · `crates/flintshot-core/src/session.rs` (crash-safe mirror, no resume) · `crates/flintshot-app/src/app_window.rs` (`sweep_stale`) · `crates/flintshot-core/src/config.rs` (`OutputFormat`) · `crates/flintshot-core/src/output.rs` (`save_capture_image`) · `crates/flintshot-app/src/cli.rs` (`CaptureArgs`, `write_png`) · `crates/flintshot-core/src/ipc.rs` (`Event`) · `crates/flintshot-editor-egui/src/shell.rs` (`request_paste`) · `dist-workspace.toml` · prior notes [2026-10-05](2026-10-05-feature-gap-analysis.md), [2026-10-07](2026-10-07-ai-capability-features.md), [2026-09-26](2026-09-26-phase7-adaptation-research.md)
