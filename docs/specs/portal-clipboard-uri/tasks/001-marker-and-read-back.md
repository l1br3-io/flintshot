# Task 001: Portal marks `clipboard:///`; app reads the grab back

**Description:** (1) In `crates/flintshot-core/src/capture/portal.rs`:
export `pub const PORTAL_GRAB_ON_CLIPBOARD: &str = "portal grab is on the system clipboard"`
(doc comment — `missing_docs` is on). In `parse_portal_uri`, before the
non-file-uri error, special-case `uri == "clipboard:///""` to return
`FlintshotError::Capture { detail: PORTAL_GRAB_ON_CLIPBOARD }` (marker
alone; no interpolation so the consumer's `contains` is exact).
(2) In `crates/flintshot-app/src/cli.rs`: add
`portal_grab_on_clipboard(err) -> bool` next to `quiet_capture_exit`,
matching `detail.contains(PORTAL_GRAB_ON_CLIPBOARD)`. In
`native_grab_prepared`'s error match, add a guard arm before the
generic error arm: on marker, read the image back via a new
`read_image_from_clipboard() -> Result<image::RgbaImage>` (arboard
`get_image`, `RgbaImage::from_raw`, dimension-mismatch →
`Capture`-style contextual error) and return `Ok(Some(img))`.
Update `parse_portal_uri`'s doc comment to name the clipboard
form. Keep `native_capture_and_dispatch` untouched.

**Acceptance criteria:**
- [ ] `parse_portal_uri("clipboard:///")` errors with a detail
      containing `PORTAL_GRAB_ON_CLIPBOARD`; `file:///` and
      `file:////` forms, empty-path, and foreign schemes behave
      exactly as before (existing tests still pass)
- [ ] `portal_grab_on_clipboard` classifies the marker detail
      true; near-miss details ("non-file uri", "cancelled",
      decode-failure) false
- [ ] `native_grab_prepared` marker arm returns the read-back
      image as the grab; read-back failure is a contextual hard
      error (mentions clipboard read-back)
- [ ] No `Capturer` trait change; xcap backend, fallback path,
      and all `PostGrabAction` arms untouched
- [ ] `flintshot-core` gains no new dependencies

**Verification:**
- [ ] `cargo test -p flintshot-core portal` (new + existing URI tests)
- [ ] `cargo test -p flintshot-app clipboard`
- [ ] `cargo build`
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo test --doc -p flintshot-core -p flintshot-editor-egui`
- [ ] Manual (COSMIC session, human): `flintshot capture` confirmed
      with **Copy** → post-grab action runs, daemon.log clean;
      **Save/Enter** unchanged

**Dependencies:** None

**Files likely touched:**
- `crates/flintshot-core/src/capture/portal.rs`
- `crates/flintshot-app/src/cli.rs`
- `CHANGELOG.md` (with Task 002)

**Docs:**
- CHANGELOG Fixed entry lands with Task 002; in-code doc comments
  updated here (`parse_portal_uri`, `native_grab_prepared` arm).

**Estimated scope:** Small (2 files)
