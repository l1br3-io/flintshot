# Task 001: Assets — SVG icon + desktop entries

**Description:** Create the launcher-facing artifacts: a simple
geometric flint/spark SVG mark (placeholder per spec OQ-1), the main
`flintshot.desktop` entry launching the daemon with region +
fullscreen Desktop Actions, and the opt-in autostart variant.

**Acceptance criteria:**
- [ ] `assets/icons/hicolor/scalable/apps/flintshot.svg` — valid
      SVG, readable at 16 px, works against both light and dark
      panels (no photographic detail, simple silhouette).
- [ ] `assets/applications/flintshot.desktop`: `Type=Application`,
      `Name=Flintshot`, `Comment=`, `Exec=flintshot daemon`,
      `Icon=flintshot`, `Terminal=false`,
      `Categories=Utility;Graphics`, `Keywords=screenshot;capture;`,
      `Actions=region;fullscreen`, matching
      `[Desktop Action region]` / `[Desktop Action fullscreen]`
      sections with `Exec=flintshot region` / `flintshot fullscreen`,
      `StartupWMClass=` set to the eframe window class (verify via
      `xprop` on a running editor window; record the value found).
- [ ] `assets/applications/flintshot-autostart.desktop`: same
      launch target, `Hidden=false`, no Actions,
      `X-GNOME-Autostart-enabled=true` + delay keys for GNOME
      parity.
- [ ] `desktop-file-validate` (if present locally) passes on both
      files; absence of the tool is non-blocking.

**Verification:**
- [ ] `desktop-file-validate assets/applications/*.desktop` (if
      available)
- [ ] Manual: copy the main file + icon to `~/.local/share/...`,
      confirm the launcher lists Flintshot with the icon and the
      context-menu actions appear

**Dependencies:** None

**Files likely touched:**
- `assets/icons/hicolor/scalable/apps/flintshot.svg` (new)
- `assets/applications/flintshot.desktop` (new)
- `assets/applications/flintshot-autostart.desktop` (new)

**Docs:** The verified `StartupWMClass` value gets a comment in the
desktop file (upstream-of-value rationale) — no external docs.

**Estimated scope:** Small (3 new asset files)
