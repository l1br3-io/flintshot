# Task 006: PortalScreenshotCapturer

**Description:** Implement the cross-desktop portal backend per
Spike B's verdict: raw `org.freedesktop.portal.Screenshot` over zbus
blocking (handle token → match rule on `org.freedesktop.portal.Request::Response`
→ method call → await one response → decode URI → crop to requested
monitor per spike coverage findings). Registered per verdict: `auto`
slot or pin-only (which sets `select_backend`'s precedence data, not
code shape). Timeout budget: ~5s request/response, then fall through
as an availability failure at selection time — never hang the worker
inside the CaptureGuard window.

**Acceptance criteria:**
- [ ] Portal dance implemented with zbus blocking: handle token
      derived from unique name, match rule installed **before** the
      method call, single Response consumed, timeouts on both phases
- [ ] Response `uri` read, file decoded, cropped to the requested
      monitor per Spike B coverage findings (whole-desktop crop vs
      single-output passthrough)
- [ ] `available()` = `org.freedesktop.portal.Desktop` name probe +
      Screenshot interface presence; `monitors()` delegates to xcap
- [ ] `Cancelled` portal response (user dismissed) maps to a distinct
      `Capture { detail }` message containing `"cancelled"` so callers
      can distinguish it (macOS `"permission"` substring precedent)
- [ ] Fixture-based unit tests: response parsing, URI decode, crop
- [ ] `#[ignore]`'d live-portal test; auto-slot/pin-only registration
      matches Spike B verdict and the (possibly amended) spec
- [ ] Cursor inclusion best-effort per spike findings; gap documented
      in code where the portal ignores it

**Verification:**
- [ ] `cargo test -p flintshot-core --quiet` green
- [ ] `cargo test --workspace --all-targets --quiet` green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` green
- [ ] Manual (each reachable session): `cargo run -- region --backend portal`
      — correct monitor, no hang, prompt behavior matches spike record

**Dependencies:** Task 2 (spike verdict), Task 3 (registry); Task 5's
Cargo.toml/zbus scaffolding if Task 5 executed first (else this task
adds them)

**Files likely touched:**
- `crates/flintshot-core/src/capture/mod.rs` (registration)
- `crates/flintshot-core/src/capture/portal.rs` (new)
- `crates/flintshot-core/Cargo.toml` (only if Task 5 hasn't landed zbus)

**Docs:**
- GOTCHA: match-rule-before-call ordering (lost responses) → comment
  at the subscription site.

**Estimated scope:** Medium (2-3 files)
