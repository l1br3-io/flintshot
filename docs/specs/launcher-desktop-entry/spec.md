---
status: draft
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
  launcher with an icon; launching it shows the tray icon; launching
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
- Validation: a unit/integration test asserting the `.desktop` files
  parse (spec 1.0 key/value grammar) and every `Exec=` resolves to a
  real subcommand — protects against drift when CLI flags change.
- CHANGELOG entry + TEST_MATRIX rows (launch-from-menu on X11 +
  Wayland sessions; autostart login flow).

**Out:**

- Packaging formats (Flatpak, AppImage, MSI, `.app`), binaries-only
  distribution channels (release artifacts with tarball layout can
  ride along later).
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

## Exit criteria

1. On a Linux session (X11 and Wayland each), after `just
   install-user`: the launcher lists "Flintshot" with the SVG icon;
   activation starts the daemon and shows the tray; a second
   activation joins/does nothing harmful (no second tray icon).
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
just install              # system install (PREFIX=/usr/local)
just install-user         # ~/.local (XDG data + bin)
AUTOSTART=1 just install-user
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
  (`xprop` on X11) confirming `StartupWMClass` correctness.

## Boundaries

- **Always:** keep `.desktop` `Exec=` targets in sync with the CLI
  surface (test-enforced); follow hicolor/XDG layout conventions;
  install recipes must be idempotent.
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

## Open questions

1. Icon design: accept a placeholder geometric flint/spark now and
   refine post-1.0 branding, or block on designed artwork?
   (Recommendation: placeholder now.)
2. Should `just install` also install the bash/zsh completions if
   `clap_complete` generation lands someday? Out of scope now —
   noted for the packaging effort.
