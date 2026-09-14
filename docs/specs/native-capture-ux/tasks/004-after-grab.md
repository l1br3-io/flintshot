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
- [x] Config `after_grab` round-trips, unknown values warn +
      default, re-save verbatim (tests mirroring `backend`'s)
- [x] `--after` validates like `--backend` (unknown → hard CLI
      error naming valid actions; clap `value_parser`)
- [x] Pure dispatch fn unit tests: flag beats config; unknown →
      editor; fallback path ignores routing (editor always — GOTCHA
      comment at the dispatch site)
- [x] Clipboard: grab → image on clipboard, notification fires, no
      editor window; failure → error like any capture failure
      (implemented + unit-level; live check blocked by the wedged
      selection UI — Phase 2 checkpoint)
- [x] Upload: grab → PNG encode → upload flow → URL on clipboard +
      notification, `ReportUploaded` broadcast (implemented; live
      check at the Phase 2 checkpoint)
- [x] `--out` still short-circuits everything (headless path wins)

**Verification:**
- [x] `cargo test -p flintshot-app --bins --quiet` green
- [x] `cargo clippy --workspace --all-targets -- -D warnings` green
- [ ] Manual (COSMIC): `--after clipboard` and `--after upload`
      both end-to-end — Phase 2 checkpoint (post-reboot)

**Accidental live finding:** a mis-written unit test (later removed)
invoked the real native flow on the wedged session — `run_capture`
hung the full 300 s interactive budget, then **exited 0 quietly**
via the `"timed out"` classification, exactly as designed. The
quiet-timeout contract is verified end-to-end.

**Dependencies:** Task 3 (the flow it plugs into)

**Files likely touched:**
- `crates/flintshot-core/src/config.rs` (`after_grab`)
- `crates/flintshot-app/src/cli.rs` (flag + dispatch + actions)

**Docs:**
- GOTCHA: fallback-ignores-routing rationale → comment at the
  dispatch site.

**Estimated scope:** Medium (2 files)
