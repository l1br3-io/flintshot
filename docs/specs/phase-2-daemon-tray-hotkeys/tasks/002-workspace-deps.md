# Task 002 — Workspace deps + MSRV gate

> Phase 2.A · size: S · depends on: none

## Description

Add the Phase 2 crate dependencies to the workspace root and to
`flintshot-app`, pin versions, and prove the whole workspace still
builds at MSRV 1.95 with a clean clippy. No behaviour yet — just get
the deps in, compiling, and lint-clean so later tasks import freely.
All choices are pre-approved by ARCHITECTURE.md §Tech stack and the
spec's Technologies table.

## Acceptance criteria

- [ ] `[workspace.dependencies]` gains: `interprocess`, `postcard`,
      `tray-icon`, `global-hotkey`, `single` (or `single-instance`,
      whichever is current — see Open Question 3), `notify`.
- [ ] `flintshot-app/Cargo.toml` pulls these in (plus `postcard` to
      match Task 001's core dep, and `tokio` already present).
- [ ] `cargo build --workspace` green at default toolchain.
- [ ] `cargo +1.95 build --workspace` green (MSRV gate; resolves Open
      Question around version pinning vs MSRV).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
      (deps that drag pedantic warnings are either feature-gated off
      or `allow`'d with a justifying comment on the crate, not per-line).

## Verification

- [ ] `cargo build --workspace && cargo +1.95 build --workspace` both
      exit 0.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` exit 0.
- [ ] `cargo fmt --all --check` exit 0.
- [ ] Note chosen versions in the CHANGELOG `[Unreleased]` Added
      section.

## Dependencies

None. Independent of Task 001 in principle (different Cargo.toml
sections), but commit 001 first so the core postcard dep and the app
deps land as one coherent "deps" checkpoint.

## Files likely touched

- `Cargo.toml` (workspace deps)
- `crates/flintshot-app/Cargo.toml`
- `CHANGELOG.md` (note new deps)

## Size

S
