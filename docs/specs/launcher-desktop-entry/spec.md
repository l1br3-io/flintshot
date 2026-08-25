---
status: active
started: 2026-08-25
---

# Spec — Launcher desktop entry: open Flintshot from the app menu

> Mini-effort spec. For product context, see
> [`../../VISION.md`](../../VISION.md); for the shipped system
> integration it builds on, see
> [Phase 2](../phase-2-daemon-tray-hotkeys/spec.md) (daemon, tray,
> single-instance) and
> [Phase 6](../phase-6-wayland-hotkeys/spec.md). Full packaging
> (Flatpak, MSI, notarized `.app`) stays a post-1.0 stretch goal —
> this effort is only about *discoverability* in desktop launchers.

## Effort status (revised 2026-08-26)

Tasks 001–004 shipped and are on `main` (icon, desktop entries,
validation test, install recipes, docs). Closeout surfaced one gap
that breaks the core promise: `just install` built the default
**no-tray** binary (`tray` is an opt-in cargo feature), so
launch-from-menu started a daemon with no tray icon. This revision
extends scope to close that gap. Remaining work:

1. Install recipes build `--features tray` by default (`FEATURES=`
   overrides) — not yet implemented.
2. Manual TEST_MATRIX pass (exit criteria 1–4) — pending a desktop
   session; the tray-build workaround in the matrix setup becomes
   obsolete once (1) lands.

## Objective

Today a user can only reach `flintshot` through a terminal
(`flintshot daemon` → tray icon) or the COSMIC panel applet. There is
no `.desktop` file, no icon, and no install target, so the app is
invisible to GNOME Activities, KDE/COSMIC launchers, and app menus.

Make Flintshot a first-class app-menu citizen, Flameshot-style: type
"Flintshot" in the launcher, hit Enter, the daemon starts and the
tray icon appears. Right-click the launcher entry for quick-capture
actions.

**Target users:** the non-terminal-inclined — or terminal-inclined
users on a fresh install who expect apps to live in the launcher,
not in `~/.cargo/bin`.

**Success looks like:**

- Fresh install → `just install` → "Flintshot" appears in the app
  launcher with an icon; the installed binary includes the tray
  (recipe-level default); launching it shows the tray icon; launching
  it again is a no-op (single-instance lock already guarantees
  join-or-replace semantics).
- Right-click / long-press the launcher entry offers *Capture
  region*, *Capture fullscreen*, *Capture window*¹ desktop actions.
- Opting in to autostart puts the tray on the desktop at login.

¹ *Capture window* lands with the
[snipping-modes](../snipping-modes/spec.md) effort; the action is
added there once the subcommand exists — this effort ships region +
fullscreen actions only.

## Assumptions

1. The primary entry launches `flintshot daemon` (the app *is* the
   resident tray), not a one-shot capture.
2. Desktop Actions (launcher right-click submenu) cover quick
   capture; no separate "Take Screenshot" top-level entry (avoids
   launcher clutter; revisit on demand).
3. Autostart is opt-in via the install recipe, not default.
4. XDG data dirs only (`~/.local/share` user install, `/usr/local`
   or `$PREFIX` system install). Flatpak/Snap/MSI remain out.
5. Install recipes build `--features tray` by default. The crate's
   cargo-level default features stay minimal (CI and bare checkouts
   stay hermetic); only the install path opts in. `FEATURES=`
   overrides (e.g. `FEATURES=""` for a minimal build). Consequence:
   GTK3 dev libs (libappindicator stack) become a build-time
   prerequisite for the default install; the recipe's failure path
   must say so and point at the escape hatch.

## Scope (in / out)

**In:**

- `assets/icons/hicolor/scalable/apps/flintshot.svg` — a simple
  flint/spark mark (geometric, monochrome-friendly, works at 16 px).
- `assets/applications/flintshot.desktop` — `Exec=flintshot daemon`,
  `Categories=Utility;Graphics`, `Actions=region;fullscreen`, icon,
  keywords, `StartupWMClass` (verified against the eframe window's
  WM_CLASS at implementation time).
- `assets/applications/flintshot-autostart.desktop` —
  `Hidden=false` autostart variant (no actions; `X-GNOME-Autostart-*`
  keys included for GNOME parity).
- `just install` / `just install-user` recipes (with `PREFIX=`
  argument): install the release binary + icon + desktop file
  (+ optional `AUTOSTART=1` for the autostart entry); `just
  uninstall` inverse. `desktop-file-install` / `update-desktop-database`
  used when present, skipped gracefully when absent.
- **Revision:** the same recipes build the binary with
  `--features "{{FEATURES:-tray}}"` — tray on by default at the
  recipe level, minimal (or custom) via `FEATURES=`. A build failure
  from missing GTK dev libs prints an actionable hint (install the
  deps or re-run with `FEATURES=""`).
- Validation: a unit/integration test asserting the `.desktop` files
  parse (spec 1.0 key/value grammar) and every `Exec=` resolves to a
  real subcommand — protects against drift when CLI flags change.
- CHANGELOG entry + TEST_MATRIX rows (launch-from-menu on X11 +
  Wayland sessions; autostart login flow).

**Out:**

- Packaging formats (Flatpak, AppImage, MSI, `.app`), binaries-only
  distribution channels (release artifacts with tarball layout can
  ride along later).
- Changing `flintshot-app`'s cargo-level default features — tray
  stays opt-in at the crate level; only install recipes default it
  on.
- Tray icon artwork changes (`tray-icon` currently uses a default
  icon; a branded tray icon is a follow-up once the mark exists).
- macOS (`.app` bundle) and Windows (Start-menu shortcut) — those
  belong to the packaging stretch goal.
- Any code changes in `flintshot-app` beyond what `StartupWMClass`
  verification demands.

## Deliverables

1. Icon + two `.desktop` files under `assets/`.
2. `just` install/uninstall recipes with PREFIX + AUTOSTART support.
3. Desktop-file validation test (parse + Exec-target checks).
4. CHANGELOG + TEST_MATRIX updates.
5. *(Revision)* Tray-default install builds: `FEATURES=` passthrough in
   `install` / `install-user`, actionable failure hint when GTK dev
   libs are missing.

## Exit criteria

1. On a Linux session (X11 and Wayland each), after `just
   install-user` (no extra env, tray included by default): the
   launcher lists "Flintshot" with the SVG icon; activation starts
   the daemon and shows the tray; a second activation joins/does
   nothing harmful (no second tray icon). `FEATURES="" just
   install-user` still succeeds as the minimal path.
2. Launcher context menu shows *Capture region* / *Capture
   fullscreen*; both trigger the standard capture flow without a
   terminal.
3. `AUTOSTART=1 just install-user` + re-login yields a running
   daemon + tray with no manual action.
4. `just uninstall` removes everything it installed; re-running
   install is idempotent.
5. Validation test fails if a `.desktop` `Exec=` names a subcommand
   that no longer exists (demonstrated by temporarily breaking one).
6. Standing gates green (`fmt`, `clippy -D warnings`, workspace
   tests, doc tests).

## Technologies

No new dependencies. `.desktop` per the freedesktop.org Desktop
Entry Specification (Actions per §"Desktop Entry Actions"); hicolor
icon theme layout; plain `install`/`cp` in `just` recipes. Optional
`desktop-file-validate` in CI when available (not a hard dep).

## Commands

```sh
just install              # system install (PREFIX=/usr/local), tray build
just install-user         # ~/.local (XDG data + bin), tray build
AUTOSTART=1 just install-user
FEATURES="" just install-user   # minimal build (no tray; no GTK build deps)
just uninstall            # inverse of install
cargo test --workspace --all-targets   # includes desktop-file validation test
```

## Project structure

| Change                | Location                            |
| --------------------- | ----------------------------------- |
| Icon                  | `assets/icons/hicolor/scalable/apps/flintshot.svg` |
| Desktop entries       | `assets/applications/*.desktop`     |
| Install recipes       | `justfile`                          |
| Validation test       | `crates/flintshot-app/tests/desktop_files.rs` |
| Docs                  | `CHANGELOG.md`, `docs/TEST_MATRIX.md` |

## Code style

No product code expected. Test follows standing conventions
(`#![forbid(unsafe_code)]` inherited, no `unwrap()` outside tests,
table-driven cases).

## Testing strategy

- **Unit/integration (app):** parse each `.desktop` (own tiny parser
  or `serde`-less line grammar — no new deps); assert required keys
  (`Type`, `Exec`, `Name`, `Icon`), `Actions` ↔ `[Desktop Action *]`
  section consistency, and `Exec=` first token + subcommand matches
  the clap command surface (share the subcommand list or hard-code
  with a pointer comment to `cli.rs`).
- **Manual:** TEST_MATRIX rows per session type; WM_CLASS check
  (`xprop` on X11) confirming `StartupWMClass` correctness. Once the
  tray-default revision lands, the matrix's `--features tray` build
  workaround goes away — plain `just install-user` is the test path;
  also verify the missing-GTK-dev-libs hint fires on a clean
  container.

## Boundaries

- **Always:** keep `.desktop` `Exec=` targets in sync with the CLI
  surface (test-enforced); follow hicolor/XDG layout conventions;
  install recipes must be idempotent; install recipes build with
  tray enabled unless `FEATURES=` overrides.
- **Ask first:** new dependencies (goal: zero); touching
  `flintshot-app` code beyond WM_CLASS verification; CI changes.
- **Never:** sudo inside recipes (document that system install needs
  elevation externally); secrets; vendor directories.

## Success criteria

- A user who never opens a terminal can install, find, launch, and
  quick-capture from the app menu.
- No regressions for CLI-first users (zero product-code changes
  expected).
- The maintenance cost is near-zero: the sync test catches drift.

## Decisions

- **Daemon is the app** — the launcher entry starts the resident
  process (Flameshot's model); one-shot captures ride desktop
  actions, not top-level entries.
- **Autostart opt-in** — respect user choice; install-time flag, not
  a default.
- **Validation via test, not linter dep** — `desktop-file-validate`
  is not guaranteed present; a small parse test keeps CI hermetic.
- **SVG-only icon at hicolor scalable/** — modern launchers scale;
  raster sizes can be generated later if a gap shows up (older tray
  implementations are out of this effort's scope).
- **Tray on by default at the recipe level** *(revision, 2026-08-26)*
  — the launcher promise is "start the daemon, see the tray"; a
  no-tray default install breaks the core UX, as closeout found.
  Cargo-level defaults stay minimal so CI and bare checkouts remain
  hermetic; `just install*` opts into `--features tray`, `FEATURES=`
  is the escape hatch, and the build-failure path must explain the
  GTK dev-lib prerequisite.

## Open questions

1. ~~Icon design: placeholder vs designed artwork?~~ **Resolved
   2026-08-25** — placeholder geometric mark shipped in task 001;
   refinement is post-1.0 branding.
2. Should `just install` also install the bash/zsh completions if
   `clap_complete` generation lands someday? Out of scope now —
   noted for the packaging effort.
3. ~~Should CI gain a `--features tray` build job?~~ **Resolved
   2026-08-26** — yes: task 006 adds a `tray-build` job (clippy with
   the feature, `-D warnings`); the stale `flintshot-app/Cargo.toml`
   comment gets fixed with it.
