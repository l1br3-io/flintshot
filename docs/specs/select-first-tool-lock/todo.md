# select-first-tool-lock — status

- [x] 001 ToolGate + disabled-chip rendering + editor rail wiring (`toolbar.rs`, `app.rs`)
- [x] 002 editor input gates — phase-scoped hotkey + pointer (`app.rs`)
- [x] 003 overlay keyboard gate on usable selection (`overlay.rs`)
- [x] 004 closeout — ADR-0043, CHANGELOG, TEST_MATRIX, standing gate

Manual pass (spec exit criteria 1–5) — 2026-09-07, scripted XV1
(Xvfb :99 800×600 + XTest, release build at `983fd27`): 8/8 checks
green. Editor: R/V/M/P hotkeys leave the toolbar strip
byte-identical pre-crop; a pre-armed Rect drag commits nothing
while the same drag demonstrably creates the selection (blue
outline ring); post-crop R arms (strip changes) and R+drag commits
a rect. Overlay: an M press before any region is inert (a
subsequent inside-region drag leaves no tint — the region drag
itself verified), the same M after the selection paints marker
tint. Criterion 4 (fullscreen/open) unit-pinned per spec;
criterion 5 standing gate green. The visual greying + lock
tooltip (TEST_MATRIX row 1) still need an eyeball pass. Ready for
/review.
