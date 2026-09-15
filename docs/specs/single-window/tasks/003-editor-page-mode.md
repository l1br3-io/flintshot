# Task 003: Editor page mode in `flintshot-editor-egui`

**Description:** Make `EditorApp` embeddable as a page of the single window.
New `EguiShell::run_editor_page(state, bridge) -> EditorPageExit` mirrors
`run_home`: same image-sized viewport (`flintshot — annotate`), but the
outcome lifecycle changes per the spec — Save/Copy/Upload apply their side
effect, report it through the bridge, and the page **stays open**; `Esc`
returns `EditorPageExit::Home`; the existing `EditorOutcome::NewCapture`
returns `EditorPageExit::Capture(mode)`; a render error closes with
`EditorPageExit::Home` plus a status through the bridge. The seam is a new
`EditorPageBridge` trait (app crate implements): `report_outcome(
&EditorOutcome)`, `set_status(&str)`, `poll_directive() -> Option<
HomeDirective>` (reuse — Hide means "external capture, close now").
Implementation: a page-mode flag consulted only at the outcome-publication
points (`publish_outcome`, and the Save/Upload arms that currently send
`ViewportCommand::Close` at `app.rs:953`/`961`); Copy already stays open
(keep reporting it). Do not touch `run_editor`/`run_overlay` (worker paths).

**Acceptance criteria:**
- [ ] Headless (`Context::default()`) tests pin every outcome: Save and
      Upload do not close and reach `report_outcome`; Copy reports and stays;
      Esc closes with `EditorPageExit::Home`; NewCapture closes with
      `Capture(mode)` and its selection-first mode label survives
- [ ] `run_editor` (worker path) behavior is byte-identical — existing
      editor tests untouched and green
- [ ] A Hide directive polled mid-frame issues the close command (the
      external-capture seam)

**Verification:**
- [ ] `cargo test -p flintshot-editor-egui` (unit + doc)
- [ ] Full gate green

**Dependencies:** None (compiles standalone; wired up in Task 004)

**Files likely touched:**
- `crates/flintshot-editor-egui/src/shell.rs` (`run_editor_page`,
  `EditorPageBridge`, `EditorPageExit`)
- `crates/flintshot-editor-egui/src/app.rs` (page-mode flag)
- `crates/flintshot-editor-egui/src/lib.rs` (re-exports)

**Docs:** GOTCHA comment at the Save/Upload close-skip explaining the
worker-vs-page divergence (the same file serves both lifecycles).

**Estimated scope:** Medium: 3 files
