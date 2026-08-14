# ADR-0025: Worker upload-result reporting (`Request::ReportUploaded`)

**Status.** Accepted.

**Context.** ADR-0023 gave workers a back-channel for saves because the
daemon needs the saved path to serve `PinLast` / `OpenLast`. Uploads
have the same information asymmetry plus one more layer: the daemon
emits `Event::UploadStarted` optimistically at dispatch, but the
resulting URL exists only inside the `flintshot upload` worker process.
Even the editor doesn't know it — the editor's Upload button spawns the
worker and closes immediately (mirroring the Pin handoff), so the
editor's `EditorOutcome::Uploaded` carries only a placeholder. IPC
subscribers (the COSMIC applet status line, scripts) want the URL and
delete-handle. The Phase 2 schema already declared
`Event::Uploaded { url, delete_handle }`; it had never been emitted.

**Decision.**

- **Add a worker-originated `Request::ReportUploaded { url,
  delete_handle }` IPC variant** to the existing `Request` enum (not a
  new transport). The worker reuses `ipc_client::delegate_to`'s
  connect+send path (length-prefixed postcard), exactly like
  `ReportSaved`.

- **`run_upload` sends the report on success, after the user-facing
  side effects** (stdout URL, clipboard copy, notification), via a
  `report_uploaded_to_daemon` helper with an
  `report_uploaded_to_daemon_at(override_path, …)` test seam.
  Best-effort semantics: `NoDaemon` (absent socket / refused connect)
  is a silent no-op so a standalone upload is unaffected, and a
  transport error mid-report is logged at `warn` and swallowed — a
  report failure must never break a successful upload.

- **The connection layer intercepts the report, not the worker
  dispatch table.** `ipc_server::handle_connection` broadcasts
  `Event::Uploaded { url, delete_handle }` to all subscribers and
  continues, mirroring the `ReportSaved` arm. A defensive no-op
  `Continue` arm in `worker::dispatch` keeps the variant from being
  misreported as "unsupported" if a future refactor routes it there.

- **Success-only.** A failed upload reports nothing: the worker already
  owns the failure UX (desktop notification "Upload failed: …", stderr
  diagnostics, non-zero exit). Subscribers see `UploadStarted` with no
  matching `Uploaded` — the same eventual consistency `CaptureStarted`
  / `EditorOpened` already have. (The spec draft sketched a
  daemon-broadcast `Event::Error { code: "upload-failed" }` on failure;
  the shipped behavior keeps failure handling worker-side. Recorded in
  the Phase 4 caveats of `docs/TEST_MATRIX.md`.)

- **No daemon state update.** `ReportSaved` feeds `LastSaved` because
  `PinLast` / `OpenLast` / `UploadLast` read it back; nothing reads a
  "last uploaded URL" yet, so the arm is pure fan-out with no state to
  track.

**Consequences.**

- Every successful upload — CLI, editor button, `UploadLast` — fans
  `Event::Uploaded` out to subscribers regardless of entry point; the
  COSMIC applet shows *"Uploaded \<url\>"*.
- `Event::Uploaded` goes from declared-never-emitted to actually
  emitted; the Phase 2 postcard round-trip test now has a live
  producer.
- Fire-and-forget, no ACK: a lost report (daemon down between upload
  and report) means subscribers miss the URL — but the worker already
  delivered it via clipboard + notification + stdout, so nothing
  user-visible depends on the report.
- `EditorOutcome::Uploaded(String)` carries an empty placeholder URL
  on the editor path; the outcome signals "upload initiated", and
  consumers must not parse the string for a real URL (documented on
  the variant).
