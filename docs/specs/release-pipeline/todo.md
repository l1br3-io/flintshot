# release-pipeline todo

- [x] 001 Author dist.toml (pinned cargo-dist config, plan green)
- [x] 002 Generate release.yml + CI plan job + just recipes
- [x] 003 cargo-release wiring (release.toml, just release / release-dry)
- [x] 004 Homebrew tap channel (tap repo + dist.toml)
- [ ] 005 .deb packaging + custom Linux job + packaging-test smoke
- [ ] 006 .rpm packaging (cargo-generate-rpm)
- [ ] 007 macOS universal .dmg (lipo + .app + hdiutil)
- [ ] 008 .AppImage (timeboxed, fallback: drop)
- [ ] 009 Docs: ADR, AGENTS.md commands, CHANGELOG
- [ ] 010 First release v0.1.0 + post-release checklist
