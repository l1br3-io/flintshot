# UX agent skills for flintshot — which to load, which to ignore

Research notes, 2026-09-26. Primary sources only: every claim about a
skill's content comes from the skill's own `SKILL.md` (local file or
upstream raw GitHub, fetched and re-verified today), and every claim
about the skills ecosystem comes from the registry
([skills.sh](https://skills.sh/)) or the repos themselves. Scope:
which **SKILL.md-based agent skill** best supports UX work on
flintshot — an egui/eframe immediate-mode desktop UI
(`crates/flintshot-editor-egui`) plus a libcosmic/iced COSMIC applet
(`crates/flintshot-cosmic`), with no DOM, CSS, or React anywhere.
Companion to
[`2026-09-26-ask-the-shot-ux-research.md`](2026-09-26-ask-the-shot-ux-research.md),
which settled AI-surface interaction patterns; this file settles the
*tooling* question: what the agent should load when doing that class
of work.

## TL;DR

**`frontend-design` (Anthropic) is the best single UX skill for this
project — and it is already installed.** It is the only candidate
whose instructions are almost entirely toolkit-agnostic (aesthetic
direction, typography, copy discipline, anti-template self-critique)
rather than web-mechanical. Use it as the primary; mine the installed
`frontend-ui-engineering` (Addy Osmani) for its transferable concept
rows (state-hierarchy discipline, empty/error/loading states,
anti-AI-aesthetic) while ignoring its React/Tailwind/ARIA mechanics;
and install `emil-design-eng` (Emil Kowalski) if motion/interaction
work intensifies — its animation *decision framework* is the most
egui-portable thing in the ecosystem. No skill anywhere teaches egui
or immediate-mode GUI; that gap stays with the
[official egui docs](https://docs.rs/egui) and
[wiki](https://github.com/emilk/egui/wiki).

## 1. What "UX" means in this repo (the transferability constraint)

From the shipped code and ADRs: the editor is a fixed five-zone
immediate-mode layout — topbar (44 px) → statusbar (30) → left tool
rail (48) → right properties panel (236) → canvas — reserved in that
order, with zone dimensions pinned by span-assertion tests
([ADR-0056](../adrs/ADR-0056-five-zone-layout.md)). Icons are painted
vector glyph fns (`fn(&Painter, Rect, Color32)`), never font-fallback
characters, enforced by a tofu-ban scan test
([ADR-0059](../adrs/ADR-0059-painted-glyphs-over-font-icons.md)).
Other UX surfaces: the three-button home page (`home.rs`), the AI
panel/dialog/config pages (`ai_panel.rs`, `ai_dialog.rs`,
`ai_config.rs`), the capture-loading route, worker editor windows,
tray, and the COSMIC popup (`crates/flintshot-cosmic`). Config
already exposes `[ui] animations` and statusbar toggles
([ARCHITECTURE.md](../ARCHITECTURE.md)).

This sets the evaluation rule: **design and interaction *principles*
transfer to egui; DOM/CSS/React *implementation* guidance does not.**
There is no hero section, no responsive breakpoints, no ARIA, no
Tailwind. What does transfer: palette/type/layout tokens, copy
discipline, motion budgets, state hierarchy, empty/error/loading
coverage, keyboard-first interaction.

## 2. Where skills come from in this setup

- The harness loads project-local skills from
  `.opencode/skills/` (28 `SKILL.md` files, including both frontend
  skills) plus user-level `~/.agents/skills/` (`find-skills`,
  `research` — verified by listing both directories).
- Provenance of the two installed frontend skills, traced to source:
  - `.opencode/skills/frontend-design/` carries an Apache-2.0
    `LICENSE.txt` and is **byte-identical** to the
    `claude-plugins-official` plugin cache copy whose
    `plugin.json` names the author "Anthropic" — i.e. Anthropic's
    skill from [anthropics/skills](https://github.com/anthropics/skills)
    (`skills/frontend-design/SKILL.md`, repo verified today; the
    local copy has drifted from upstream HEAD — §5.1).
  - `.opencode/skills/frontend-ui-engineering/` is a copy of
    [addyosmani/agent-skills](https://github.com/addyosmani/agent-skills)
    `skills/frontend-ui-engineering/SKILL.md` at an **older
    revision** (upstream has since inlined the container/presentational
    example and fixed its reference paths — §5.2).
- Discovery mechanics: `~/.agents/skills/find-skills/SKILL.md` says
  new skills come from the open agent-skills registry at
  [skills.sh](https://skills.sh/) (operated by Vercel; footer links
  [vercel-labs/skills](https://github.com/vercel-labs/skills)),
  installed via `npx skills add <owner/repo@skill>`, with explicit
  quality gates: prefer 1K+ installs, official sources
  (`anthropics`, `vercel-labs`, `microsoft`), skepticism below 100
  GitHub stars.

## 3. Candidate inventory

Installs are skills.sh leaderboard values fetched today (all-time).
"Transfer" = how much of the skill's actual instructions apply to an
egui immediate-mode desktop GUI.

| Skill | Primary source | Installs | Focus | Transfer | Verdict |
| --- | --- | --- | --- | --- | --- |
| `frontend-design` | [anthropics/skills](https://github.com/anthropics/skills/tree/main/skills/frontend-design) · local `.opencode/skills/` | 923.8K (#7) | Aesthetic direction, typography, copy, anti-template critique | **High** | **Primary (installed)** |
| `frontend-ui-engineering` | [addyosmani/agent-skills](https://github.com/addyosmani/agent-skills) · local `.opencode/skills/` | n/a (not on board) | Component/state/a11y engineering | Med (concepts only) | **Secondary — mine selectively** |
| `emil-design-eng` | [emilkowalski/skills](https://github.com/emilkowalski/skills) | 298.8K | Interaction & motion polish | High (frameworks) / Low (CSS) | **Optional add** |
| `impeccable` | [pbakaus/impeccable](https://github.com/pbakaus/impeccable) | 294.0K | Full design suite: shape/critique/audit/harden/onboard/animate | Med | Optional (heavy machinery) |
| `web-design-guidelines` | [vercel-labs/agent-skills](https://github.com/vercel-labs/agent-skills) | 667.7K | Audits web code against fetched web guidelines | Low | Reject |
| `ui-ux-pro-max` | [nextlevelbuilder/ui-ux-pro-max-skill](https://github.com/nextlevelbuilder/ui-ux-pro-max-skill) | 371.2K | Multi-stack UI decisions | Low (no Rust stack) | Reject |
| `design-mobile-apps` | [designed-by-ai/skills](https://github.com/designed-by-ai/skills) | 441.9K | Client for the Sleek mobile-design SaaS API | None | Reject |
| `canvas-design` | [anthropics/skills](https://github.com/anthropics/skills/tree/main/skills/canvas-design) | in-repo | Posters/art as .png/.pdf artifacts | Low | Reject for this use |
| `theme-factory` | [anthropics/skills](https://github.com/anthropics/skills/tree/main/skills/theme-factory) | in-repo | Slide-deck/report theming (10 preset themes) | Low | Reject |
| `rust-desktop-applications` | `bobmatnyc/claude-mpm-skills` (canonical path 404s) | unknown | Rust desktop engineering (Tauri/egui/iced/slint) | unverified | Reject (provenance) |
| obra/superpowers | [obra/superpowers](https://github.com/obra/superpowers) | 376.5K (top skill) | Process skills only — repo structure verified: no UX/design skill exists in it | n/a | Not a candidate |

The registry's entire design-skills cohort is web-first: beyond the
rows above, the leaderboard's design names are `design-taste-frontend`
(520.6K), `shadcn` (271.7K), `design-an-interface` (216.9K) — all
frontend-stack skills. **Nothing aimed at desktop native GUIs, Rust,
egui, or toolkit-agnostic UX appears anywhere in the top ~290.**

## 4. Evaluation criteria

1. **Transferability to egui/immediate-mode Rust GUI** — principles
   transfer, DOM/CSS/React mechanics do not. libcosmic/iced (applet)
   is a secondary consideration.
2. **Content depth/quality** of the actual `SKILL.md` instructions —
   not the marketing description around them.
3. **Provenance/maintenance** — first-party authorship, living
   upstream, license.

| Candidate | Transfer | Depth | Provenance |
| --- | --- | --- | --- |
| `frontend-design` | High | High (dense, opinionated, process + critique) | Anthropic, Apache-2.0, 178k-star repo |
| `frontend-ui-engineering` | Med | High (but ~70% of lines are web mechanics) | Addy Osmani; local copy stale vs HEAD |
| `emil-design-eng` | High (frameworks) | Very high on motion/interaction | Emil Kowalski (Vercel; Sonner/Base UI author) |
| `impeccable` | Med | Very high breadth | Paul Bakaus, Apache-2.0, v4.4.0; script-dependent |
| everything else | Low | — | — |

## 5. Per-candidate analysis (from the SKILL.md contents)

### 5.1 `frontend-design` — the recommendation

Local file: `.opencode/skills/frontend-design/SKILL.md`; upstream
[raw](https://raw.githubusercontent.com/anthropics/skills/main/skills/frontend-design/SKILL.md).
What it actually instructs: act as "the design lead at a small studio
known for giving every client a visual identity that could not be
mistaken for anyone else's"; **ground the design in the subject's own
world** ("its materials, instruments, artifacts, and vernacular");
build a compact **token system** before any code — palette as "4–6
named hex values," type roles ("a characterful display face that's
used with restraint, a complementary body face"), layout concept via
"one-sentence prose descriptions and ASCII wireframes," and a
**signature element** ("the single unique element this page will be
remembered by"); calibrate against three named AI-default looks
(warm-cream + serif + terracotta; near-black + acid-green;
broadsheet hairlines) and revise anything that reads as the default
answer; "**Spend your boldness in one place**… keep everything around
it quiet and disciplined"; a quality floor of "visible keyboard
focus, reduced motion respected"; and a full copy discipline —
"Name things by what people control and recognize, never by how the
system is built," buttons "say exactly what happens when it's used:
'Save changes,' not 'Submit'," "Errors don't apologize," "An empty
screen is an invitation to act."

Why it wins here: every one of those instructions is about *choices*,
not implementation. Nothing in the file requires a DOM — the only
web-specific passages are the "For web designs, the hero is a thesis"
framing, a closing note on CSS selector specificity, and
"responsive down to mobile" in the quality floor, each trivially
re-mapped (§6). It is also the most-installed design skill in the
registry and first-party maintained.

Maintenance note: the local (and plugin-cache) copy differs from
upstream HEAD in wording — e.g. local says "take one real aesthetic
risk you can justify" where HEAD says "take aesthetic risk if
justified," and the typography guidance has diverged (local: "Pair
the display and body faces deliberately"; HEAD: "You don't need a
different typeface for display… use one family or two"). Both diffs
verified today. Not load-bearing for the recommendation, but the
local copy is a snapshot, not a live track.

### 5.2 `frontend-ui-engineering` — web-centric; keep the concept rows

Local file: `.opencode/skills/frontend-ui-engineering/SKILL.md`. The
verdict is unambiguous from the content: file structures are
`TaskList.tsx` / `TaskList.stories.tsx`; component patterns are TSX
compositions; the state ladder runs `useState` → Context → URL
`searchParams` → React Query/SWR → Zustand/Redux; the design-system
section speaks Tailwind/CSS spacing scales and `text-primary` tokens;
accessibility is DOM/ARIA (`aria-label`, `htmlFor`, `tabIndex`, focus
traps via `useRef`/`useEffect`, `role="status"`); responsive is
mobile-first Tailwind grids tested at 320/768/1024/1440 px; loading
is skeleton components and React Query optimistic updates; the
verification checklist ends on "Screen reader," "axe-core," and
console errors. **None of that machinery exists in egui.**

What survives the toolkit translation:

- The **state-management ladder's** discipline — "choose the simplest
  approach that works," local before lifted before global — maps
  cleanly onto egui state ownership: per-frame widget state vs
  `EditorState` (core-owned) vs the app's `Shared` handle.
- "**Avoid prop drilling deeper than 3 levels**" → stop threading
  `&mut` through nested `ui` closures; restructure the panel tree.
- The **anti-AI-aesthetic table** (purple/indigo everything,
  rounded-everything, shadow-heavy, uniform card grids, oversized
  padding) is toolkit-blind — it reads as a checklist for our
  `Color32` token set and spacing constants.
- "**Don't rely solely on color to convey information**" — directly
  applicable to tool-rail selection state and AI error cards.
- The **empty/error/loading states** section and its "meaningful
  empty states" pattern (the AI transform store's invitation copy is
  exactly this class).
- "Components with more than 200 lines (split them)" — module
  discipline for `panel.rs`-scale files.

Caveats: the local copy is an older revision of the upstream
addyosmani skill (three diffs verified: description wording, the
container/presentational example, and the See-Also path), and its two
deep-dive pointers are **dangling in this repo** — `react/patterns.md`
and `@rules/accessibility-checklist.md` resolve only in the source
harness checkout (`opencode-agent-harness/.opencode/harness/tech/react/patterns.md`,
`.opencode/harness/rules/accessibility-checklist.md`), not in
flintshot's `.opencode/`.

### 5.3 `emil-design-eng` — the motion/interaction framework worth installing

Upstream [raw](https://raw.githubusercontent.com/emilkowalski/skills/main/skills/emil-design-eng/SKILL.md)
("encodes Emil Kowalski's philosophy on UI polish, component design,
animation decisions"). The CSS/Framer Motion/WAAPI mechanics don't
apply, but the decision frameworks are the most portable interaction
guidance found anywhere in this survey:

- **The frequency table**: "100+ times/day (keyboard shortcuts,
  command palette toggle) → **No animation. Ever.**" and "Never
  animate keyboard-initiated actions." flintshot's core flow *is*
  hotkey-initiated (capture → editor); the rule says the editor page
  must appear instantly, with zero entrance choreography.
- **Duration budgets**: press feedback 100–160 ms, tooltips
  125–200 ms, dropdowns 150–250 ms, "UI animations should stay under
  300 ms" — a direct spec for `[ui] animations` behavior and the
  properties-panel pivot.
- **Easing rule**: entering elements ease-out ("`ease-in` feels
  sluggish; `ease-out` gives instant feedback"); movement eases
  in-out.
- **Asymmetric press/release**: "slow where the user is deciding,
  fast where the system is responding."
- **Perceived performance**: "A faster-spinning spinner makes the
  app feel like it loads faster" — the capture-loading route's knob.
- **Interruptibility**: prefer retargetable transitions over
  restart-from-zero keyframes — immediate mode's continuous repaint
  gives this for free; the rule still disciplines *what* animates.
- `prefers-reduced-motion` ("fewer and gentler animations, not zero")
  maps onto honoring `[ui] animations` and the OS setting.
- Process discipline: "Review your work the next day," review output
  as a `| Before | After | Why |` table.

Provenance: Emil Kowalski is the author of Sonner ("13M+ weekly npm
downloads" per the skill's own text) and Base UI — a practicing
interaction-quality authority, 298.8K installs.

### 5.4 `impeccable` — strong ideas, heavy web-defaulted machinery

[SKILL.md](https://raw.githubusercontent.com/pbakaus/impeccable/main/.claude/skills/impeccable/SKILL.md)
(v4.4.0, Apache-2.0). A 20+-verb design suite (`shape`, `critique`,
`audit`, `polish`, `harden`, `onboard`, `animate`, `clarify`, `live`
…) with PRODUCT.md/DESIGN.md context files, a detector hook, and a
launcher binary. Two things earn it a look: its **mode model** —
"**Operate:** the visitor completes a task. App UI, dashboards,
editors, admin, settings, tools. Scanability, consistency, native
expectations… outrank expression" — is flintshot's exact category and
a useful tiebreaker rule; and it ships **native-platform variants**
(`audit.native.md`, `adapt.native.md`; `init` records `ios`,
`android`, or `adaptive`). But the native references are
mobile-only, verification defaults to browser iteration
("desktop and mobile together on the web"), and the setup (launcher
scripts, hooks) presumes a web toolchain. Concepts transfer; the
machinery does not. Keep on the radar, not in the kit.

### 5.5 Rejected, with reasons on the record

- **`web-design-guidelines`** (vercel-labs, 667.7K installs): the
  SKILL.md is a thin auditor — "Fetch the latest guidelines" from
  vercel-labs/web-interface-guidelines, then check files and emit
  `file:line` findings. Its rule set is DOM/CSS/web-a11y; reviewing
  egui Rust code against web interface guidelines is noise.
- **`ui-ux-pro-max`** (371.2K): multi-stack by intent — its stack
  list covers react/vue/swiftui/flutter/javafx/wpf/winui/avalonia
  and more — but no Rust-native GUI stack (egui/iced/slint) appears;
  the closest stacks are other languages' retained-mode toolkits.
- **`design-mobile-apps`** (441.9K): the SKILL.md is an API client
  for the paid [sleek.design](https://sleek.design) SaaS — requires
  `SLEEK_API_KEY`, network restricted to sleek.design, mobile-screen
  output. Wrong platform, paid service, network dependency.
- **`canvas-design` / `theme-factory`** (Anthropic): the former
  produces poster/art `.png`/`.pdf` artifacts from a generated
  "design philosophy"; the latter applies 10 preset themes to
  slides/decks. Neither addresses interactive application UI.
- **`rust-desktop-applications`** (`bobmatnyc/claude-mpm-skills`):
  the only "Rust + egui/iced/slint" skill that surfaced — via a
  third-tier mirror site; the canonical GitHub paths 404 on both
  `main` and `master` (verified today), and it targets app
  *engineering*, not UX. Fails the provenance bar
  (`find-skills`' own gates: official-ish source, reputation).
- **obra/superpowers**: repo structure verified — process skills
  only (brainstorming, TDD, debugging, worktrees); no UX skill.

## 6. How we'd use it on flintshot

| Surface | Skill | What it changes |
| --- | --- | --- |
| Home page (`home.rs`) | `frontend-design` | Ground-it-in-the-subject pass on the three-button job; token plan (palette/type/signature); copy rules for buttons and hints |
| Editor five zones (ADR-0056) | `frontend-design` + `frontend-ui-engineering` | "Spend boldness in one place" — the canvas paper-cutout stays the signature, chrome stays quiet; spacing-scale/token discipline for zone paddings (the `Color32` token scan is the enforcement hook) |
| Tool rail / topbar glyphs (ADR-0059) | `frontend-ui-engineering` | "Color not the sole indicator" for selected-tool state (geometry + color); spacing scale for the 48 px rail |
| Properties panel / AI panel / dialog / config | `frontend-design` (copy) + `frontend-ui-engineering` (states) | Error-card copy taxonomy ("Errors don't apologize," name-by-what-the-user-controls); empty/loading/error coverage on the transform store, key status, Test-connection flow |
| Motion anywhere (`[ui] animations`) | `emil-design-eng` | Frequency gate (never animate hotkey-initiated routes); <300 ms budgets; ease-out entries; asymmetric press/release; reduced-motion respect |
| Capture-loading route | `emil-design-eng` | Perceived-performance rules for spinner/progress; no entrance animation on the post-capture editor page |
| COSMIC applet (`flintshot-cosmic`) | same principles | Nothing iced/libcosmic-specific exists in any skill; apply the same principle rows by hand |

**What must be ignored or re-mapped for egui:**

- `frontend-design`: the "hero is a thesis" web framing → the home
  page's single job; "responsive down to mobile" → minimum window
  size plus the pinned zone spans (span-assertion tests are the
  floor); the CSS-selector-specificity warning → delete outright;
  "take screenshots" during self-critique → works as-is (viewport
  capture).
- `frontend-ui-engineering`: **all implementation content** — TSX,
  Tailwind, Storybook, React Query, ARIA/DOM focus, breakpoints,
  axe-core. The ARIA/focus intent re-maps to egui's keyboard-first
  `Response` handling and its AccessKit integration (screen-reader
  support), verified against the
  [egui docs](https://docs.rs/egui), not to `role=`/`tabIndex`.
- `emil-design-eng`: CSS transitions, `@starting-style`,
  `clip-path`, Framer Motion, WAAPI, GPU-compositing rules → egui's
  `Context::animate_*` helpers and painter-drawn states; the *rules*
  (what animates, how long, which easing) carry over unchanged.

## 7. Honest gaps

1. **No skill teaches egui.** Immediate mode, retained state via
   `Memory`/ids, panel reservation, painter drawing, viewport
   commands — the complementary primary sources are
   [docs.rs/egui](https://docs.rs/egui) and the
   [egui wiki](https://github.com/emilk/egui/wiki) (both fetched
   today), plus the bundled demo app as the canonical cookbook. Every
   skill above must be *supervised* by that documentation when it
   touches implementation.
2. **Nothing for iced/libcosmic** either — the COSMIC applet is
   uncovered by the entire skills ecosystem; libcosmic's own docs
   remain the only source there.
3. **No desktop-native UX skill exists in the registry's ranks.**
   The design-skills cohort is uniformly web-first (§3). This is an
   ecosystem gap, not a search failure — the honest answer to "is
   there a better, desktop/Rust-aware skill?" is *no, not today*.
4. **The local copies are stale snapshots** with dangling internal
   references (§5.1–5.2). Refresh path: re-copy from upstream or
   `npx skills add <owner/repo@skill>`; review diffs like any
   dependency bump.
5. **Supply-chain caution**: skills are executable-adjacent prompt
   content. Snyk's ToxicSkills survey found prompt injection in 36%
   of tested skills and recommends reviewing a skill's `SKILL.md`
   and bundled scripts before installing ([Snyk](https://snyk.io/articles/top-claude-skills-ui-ux-engineers)).
   Applies doubly to script-bearing skills like `impeccable`.
6. **Registry install counts measure popularity, not quality** —
   `design-mobile-apps` at 441.9K installs is a SaaS client, not UX
   guidance.

## 8. What could not be verified from primary sources

- **`rust-desktop-applications`**: only mirror-site descriptions were
  reachable; the canonical repo path 404s on both default branches.
  Its egui/iced/slint coverage claims are unverified against a
  first-party source.
- **UI.sh** (the Tailwind team's `/design`, `/ideas` skill set):
  paid, invite-only; its SKILL.md files are not public. Characterized
  only from a practitioner's blog post and video walkthrough —
  secondary sources, excluded from the ranking.
- **`ui-ux-pro-max`'s stack list** was taken from a search-engine
  snippet of its GitHub SKILL.md page, not a full read; the absence
  of egui/iced is consistent across snippets but was not verified
  against the complete file.
- **Per-skill install figures** differ between registries (e.g.
  aiuxplayground lists Anthropic skills at 45.5K where skills.sh
  shows 923.8K) — counting windows/methods differ; skills.sh (the
  registry `find-skills` itself points to) is used as canonical.
- **Anthropic's plugin-cache copy vs repo HEAD drift direction**:
  both local and cache copies match each other and differ from
  today's HEAD; which release the snapshot corresponds to was not
  established.

## What we adopt

| Skill | Action | What exactly we take |
| --- | --- | --- |
| `frontend-design` (installed) | **Primary** — invoke for every UX-facing change | Token-plan-first process; signature-element restraint; subject-grounded choices; copy discipline; self-critique pass before build |
| `frontend-ui-engineering` (installed) | Secondary — read for the concept rows only | State-hierarchy simplicity; empty/error/loading coverage; anti-AI-aesthetic table; color-not-sole-indicator; 200-line component rule |
| `emil-design-eng` | Optional install (`npx skills add emilkowalski/skills@emil-design-eng`) | Frequency gate (never animate hotkey-initiated flows); <300 ms budgets; ease-out entries; asymmetric press/release; reduced-motion respect; Before/After/Why review format |
| egui docs + wiki | Always alongside | The immediate-mode mechanics no skill covers |
