# Task 003: Install / uninstall recipes

**Description:** `just` recipes putting the artifacts (and release
binary) where desktop environments look: `just install` (system,
`PREFIX=/usr/local` overridable, expects the user to have write
access or elevate externally — no sudo inside), `just install-user`
(`~/.local`: `share/applications`, `share/icons`, `bin`), both with
optional `AUTOSTART=1` adding the autostart variant to
`~/.config/autostart`, and `just uninstall` removing exactly what
install put down.

**Acceptance criteria:**
- [ ] `install` and `install-user` copy: binary (from
      `cargo build --release` — recipe builds first),
      `flintshot.desktop`, icon (autostart variant only with
      `AUTOSTART=1`); dirs created as needed; re-runs idempotent.
- [ ] `update-desktop-database` / `gtk-update-icon-cache` invoked
      when present on PATH, skipped with an `echo ok:` line when
      absent (house fallback style).
- [ ] `uninstall` removes installed files (both scopes; safe if
      already absent).
- [ ] Recipe headers document: system install may need elevation
      (`sudo just install`), `PREFIX=`/`AUTOSTART=` usage examples.
- [ ] No recipe captures or moves anything outside PREFIX /
      `~/.local` / `~/.config/autostart`.

**Verification:**
- [ ] `just install-user && just install-user` (idempotency)
- [ ] Manual (X11 + Wayland sessions): launcher lists Flintshot;
      launch → tray appears; relaunch → still one tray icon;
      context actions capture; `AUTOSTART=1` + re-login → tray at
      login; `just uninstall` → all gone
- [ ] `just --list` shows the new recipes with their doc comments

**Dependencies:** Tasks 001, 002

**Files likely touched:**
- `justfile`

**Docs:** None beyond recipe comments.

**Estimated scope:** Small (1 file)
