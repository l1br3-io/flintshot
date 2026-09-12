# Task 003: Closeout — narrow-width matrix, CHANGELOG, review kit

**Description:** Final verification sweep across both surfaces,
including the narrow-capture stress the spec calls out (a ~500 px
region capture is wider than nothing but far narrower than the full
pod rail), plus the CHANGELOG entry and the PR review kit. No new
product code unless the matrix finds a defect (fix-forward, then
re-run the gates).

**Acceptance criteria:**
- [x] All four gate commands green in one final run
- [x] Manual matrix: `cargo run -- region` with a ~500 px-wide
      selection — editor-rail fallback (horizontal scroll) reaches
      every control; overlay toolbar stays on-panel with no clipped
      or overlapping chips
      *(editor rail: verified live — the 500 px window lays out
      every pod with the redact island clipped at the window edge,
      i.e. the scroll fallback engaged; every-control-reachability
      is pinned headlessly by
      `editor_rail_narrow_viewport_lays_out_every_control`. Overlay:
      N/A on this Wayland machine — see findings.)*
- [x] Manual matrix: default `cargo run -- app` editor window at
      normal width — pods perceivable without tooltips on both
      surfaces
      *(editor rail: verified live at 1200 px — human approved
      2026-09-13. Overlay: N/A on Wayland; pod geometry pinned by
      the task-002 island tests.)*
- [x] `CHANGELOG.md` updated under `## [Unreleased]` (user-visible
      toolbar change)
- [x] PR screenshots attached (editor + overlay, normal and narrow)
      *(editor normal + narrow captured in
      `docs/specs/toolbar-groups/screenshots/` for PR attachment;
      overlay shots N/A on Wayland — see findings.)*
- [x] Spec success criteria 1-4 checked against the built app; any
      miss is either fixed or recorded in the spec's Open Questions
      with the human's call *(no misses; findings below are
      platform N/A, not toolbar defects)*

**Verification:**
- [x] `cargo fmt --all --check` && `cargo clippy --workspace
      --all-targets -- -D warnings` && `cargo test --workspace
      --all-targets` && `cargo test --doc -p flintshot-core -p
      flintshot-editor-egui`
- [x] Manual checks above, screenshots saved

**Findings (closeout):**

- **Wayland overlay is worse than best-effort on COSMIC.** An
  explicit `region --overlay --standalone` run died ~80 s in with a
  winit event-loop error and broken Wayland pipe; a later manual
  attempt wedged the user's session so hard it needed a logout.
  Pre-existing Phase 5 platform behavior (ADR-0028 demotes the
  config path; TEST_MATRIX L2–L5 rows are N/A) — recorded in
  TEST_MATRIX's Phase 5 caveats, not fixed here. The overlay's pod
  visuals stay pinned by the task-002 synthetic-context tests, and
  `toolbar_anchor_keeps_wide_pod_rail_on_panel_beside_narrow_selection`
  pins the widened rail's on-panel clamping headlessly.
- **Live shift-scroll of the narrow rail was not driven** (synthetic
  input focus on the live desktop proved unreliable). Compensating
  coverage: the narrow-viewport unit test (every control laid out,
  non-overlapping, overflow engaged) plus the live 500 px screenshot
  showing the fallback state engaged with visible pods intact.

**Tests added (the matrix, pinned headlessly):**

- `toolbar::tests::editor_rail_groups_pods_as_islands` — 8
  GRAPHITE_800 islands, one row, pod gaps (24 px) dominating chip
  gaps (8 px) at a uniform rhythm.
- `toolbar::tests::editor_rail_narrow_viewport_lays_out_every_control`
  — the 500 px stress: all 19 controls registered, non-degenerate,
  non-overlapping; rail content overflows the viewport (scroll
  fallback engaged).
- `overlay::tests::toolbar_anchor_keeps_wide_pod_rail_on_panel_beside_narrow_selection`
  — a measured-width pod rail stays fully on-panel beside a 500 px
  selection on a 1280 px panel; a panel narrower than the rail pins
  left (spec OQ2's no-wrap answer, now executable documentation).

**Dependencies:** Task 002

**Files likely touched:**
- `CHANGELOG.md`
- (Contingency fixes only where the manual matrix finds a defect)

**Docs:**
- If the narrow-width scroll behavior surprises, a why-comment at
  the ScrollArea site — it already documents the fallback, so
  likely nothing.

**Estimated scope:** Small: 1 file + manual checks
