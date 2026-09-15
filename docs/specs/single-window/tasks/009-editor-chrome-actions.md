# Task 009: Editor chrome actions (capture / open / paste)

**Description:** The editor page gets the same three actions the home has
(spec: stay-in-editor + always-reachable actions). Extend
`EditorPageBridge` with `request_capture()`, `open_image(PathBuf)`,
`request_paste()`, and wire controls into the editor chrome — placement
follows the existing toolbar structure (a compact group in the rail/bay;
the current recapture control becomes/remaps to the Capture action since
`EditorOutcome::NewCapture` already models it). Capture routes through the
Task-006 close-then-grab flow (`EditorPageExit::Capture`); Open uses the
same in-window `rfd` dialog then replaces the page's image (window
recreate for geometry); Paste reads the clipboard (Task 005's helper,
hoisted if it lives in the app crate) and replaces the image. Replacing an
unsaved image needs no confirm — Replace semantics, consistent with
supersede (note this in the button's tooltip/doc).

**Acceptance criteria:**
- [ ] From an open editor page: Capture closes → grabs → fresh editor
      page; Open… swaps the image; Paste swaps or status-lines on empty
      clipboard
- [ ] The three actions are keyboard-reachable (accelerators matching
      home: C / O / P — note the editor's existing tool keys and resolve
      collisions; existing accelerators win, document the choice)
- [ ] Headless tests: each action reaches the bridge exactly once;
      `NewCapture` still carries its mode label for selection-first

**Verification:**
- [ ] Full gate green
- [ ] Manual: annotate → Capture → new page; annotate → Paste → swap;
      accelerator collision check on the dev session

**Dependencies:** Tasks 003, 005, 006

**Files likely touched:**
- `crates/flintshot-editor-egui/src/{toolbar,chrome,app,shell}.rs`
- `crates/flintshot-app/src/app_window.rs` (bridge impl)

**Docs:** One-line comment at the accelerator resolution noting which
single-letter keys the editor tools already claim.

**Estimated scope:** Medium: 4-5 files
