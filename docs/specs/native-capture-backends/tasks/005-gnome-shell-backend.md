# Task 005: GnomeShellCapturer

**Description:** Implement the GNOME Shell Screenshot backend per
Spike A's verdict. Adds `zbus` (linux-gated) + `tempfile` to core,
lands `capture/gnome_shell.rs` with the blocking facade, and amends
`no_gui_types_in_core.rs` to allow `use zbus` **only** under
`src/capture/` (the ashpd ban stays global and its regression test
keeps asserting it). Registers the backend in the linux registry slot.
If Spike A said `drop-to-portal`, this task shrinks to just the
no-GUI-test amendment + dep scaffolding, and the registry slot is
removed — do not implement a dead backend.

**Acceptance criteria:**
- [ ] `zbus` + `tempfile` under
      `[target.'cfg(target_os = "linux")'.dependencies]`; macOS/Windows
      builds compile no D-Bus code
- [ ] `no_gui_types_in_core.rs`: path-scoped zbus allowance with its
      own regression test (synthetic `use zbus` line in capture/ →
      pass; elsewhere → fail); `use ashpd` still fails everywhere
- [ ] `available()` = bus-name probe (+ method presence per spike);
      `monitors()` delegates to xcap; `capture_monitor` grabs by
      monitor **geometry/rect** per spike findings (never by service
      index), writes to a temp file, decodes, cleans up on all paths
- [ ] Every D-Bus call carries a timeout; service errors map to
      `FlintshotError::Capture { detail }` verbatim; service
      returning `success = false` is an error, not an empty image
- [ ] Fixture-based unit tests: PNG-file decode path, error mapping
- [ ] `#[ignore]`'d live-service test mirroring `tests/capture.rs`
- [ ] zbus version aligned with ashpd's zbus in `flintshot-app`
      (same major; `cargo tree -d` shows no duplicate)

**Verification:**
- [ ] `cargo test -p flintshot-core --quiet` green
- [ ] `cargo test --workspace --all-targets --quiet` green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` green
- [ ] Manual (GNOME session): `cargo run -- region --backend gnome-shell`
      captures correct monitor, cursor honored, timing sane vs spike

**Dependencies:** Task 1 (spike verdict), Task 3 (registry)

**Files likely touched:**
- `crates/flintshot-core/Cargo.toml`
- `crates/flintshot-core/src/capture/mod.rs` (registration)
- `crates/flintshot-core/src/capture/gnome_shell.rs` (new)
- `crates/flintshot-core/tests/no_gui_types_in_core.rs`

**Docs:**
- GOTCHA: coordinate-space mismatch (shell logical vs xcap physical
  px on scaled displays) → comment at the rect-conversion site.

**Estimated scope:** Medium (4 files)

**Outcome (executed 2026-09-13, per Spike A `drop-to-portal`):**
GnomeShellCapturer never implemented — GNOME 41+ blocks third-party
callers of `org.gnome.Shell.Screenshot` (DBusSenderChecker allowlist),
so it would be dead code. No registry slot existed to remove (Task 3
shipped portal + xcap only). Landed: linux-gated `zbus` workspace dep
in `flintshot-core` (unifies with ashpd's 5.18, `cargo tree -d` shows
one zbus); `no_gui_types_in_core.rs` gained a path-scoped allowance —
`use zbus` passes only under `src/capture/`, fails everywhere else
(probe-verified both directions), `use ashpd` still banned everywhere.
`tempfile` was **not** added: the portal writes the screenshot file
itself; the client only reads + decodes it (the tempfile consumer was
the gnome backend). Config round-trip fixture retargeted off the
`"gnome-shell"` id. GNOME-manual verification criteria are moot.
