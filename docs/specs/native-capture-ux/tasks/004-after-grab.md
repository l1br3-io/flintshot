# Task 004: `after_grab` routing — config + `--after` + clipboard/upload

**Description:** Wire the post-grab actions on the **native path
only**. Config: `[capture] after_grab = "editor" (default) |
"clipboard" | "upload"` (string-tolerant: unknown loads, warns,
resolves `editor`, re-saves verbatim — `backend`/`interaction-mode`
precedent). CLI: `--after <ACTION>` overrides config. Dispatch is a
pure function `(config_value, flag) -> PostGrabAction` (unit-tested)
applied after `capture_selection` succeeds: **editor** — today's
path; **clipboard** — `arboard` image copy + desktop notification,
no windows; **upload** — reuse the upload path (default backend,
URL → clipboard + notification, `ReportSaved` equivalent for
upload-last). Fallback (xcap) path always routes to the editor flow.

**Acceptance criteria:**
- [ ] Config `after_grab` round-trips, unknown values warn +
      default, re-save verbatim (tests mirroring `backend`'s)
- [ ] `--after` validates like `--backend` (unknown → hard CLI
      error naming valid actions)
- [ ] Pure dispatch fn unit tests: flag beats config; unknown →
      editor; fallback path ignores routing (editor always)
- [ ] Clipboard: grab → image on clipboard, notification fires, no
      editor window (manual); failure → error like any capture
      failure
- [ ] Upload: grab → PNG encode → upload flow → URL on clipboard +
      notification (manual with the mockito-backed tests where the
      upload suite has them)
- [ ] `--out` still short-circuits everything (headless path wins)

**Verification:**
- [ ] `cargo test -p flintshot-app --bins --quiet` green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` green
- [ ] Manual (COSMIC): `--after clipboard` and `--after upload`
      both end-to-end

**Dependencies:** Task 3 (the flow it plugs into)

**Files likely touched:**
- `crates/flintshot-core/src/config.rs` (`after_grab`)
- `crates/flintshot-app/src/cli.rs` (flag + dispatch + actions)

**Docs:**
- GOTCHA: fallback-ignores-routing rationale → comment at the
  dispatch site.

**Estimated scope:** Medium (2 files)
