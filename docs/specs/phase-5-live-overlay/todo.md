# Todo — Phase 5: Live overlay

- [x] 001: `interaction_mode` config + `--overlay`/`--editor` CLI flags + mode-resolution helper
- [x] 002: `OverlayApp` fullscreen borderless viewport + frozen-capture render + dispatcher wiring + Esc/Enter
- [x] 003: Selection chrome — dim mask, 8 resize handles, move-drag, W×H badge, magnifier on hover
- [x] 004: Floating toolbar anchored to selection (tool rail + output actions + accept row)
- [x] 005: All ten annotation tools active inside the selection (rendering clipped to region)
- [ ] 006: Output flows — Enter=Copy, Save, Pin/Upload workers, `EditorOutcome` publication
- [ ] 007: Wayland detection + editor-window fallback (+ force-flag semantics); macOS/Windows pass
- [ ] 008: Idle-repaint discipline + performance check + polish
- [ ] 009: Docs — ADR-0028 (capture-then-overlay), CHANGELOG, TEST_MATRIX
