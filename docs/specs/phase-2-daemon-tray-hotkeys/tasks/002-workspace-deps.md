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

- [x] `[workspace.dependencies]` gains: `interprocess`, `postcard`,
      `tray-icon`, `global-hotkey`, `single` (or `single-instance`,
      whichever is current — see Open Question 3), `notify`.
      _Resolved: `single-instance` (0.3.3) — the `single` crate is an
      unrelated Itertools helper. Versions: `interprocess` 2.4.2
      (`tokio` feature), `global-hotkey` 0.8, `notify` 8,
      `single-instance` 0.3, `tray-icon` 0.24._
- [x] `flintshot-app/Cargo.toml` pulls these in (plus `postcard` to
      match Task 001's core dep, and `tokio` already present).
      _`tray-icon` is gated behind a new `tray` feature (default OFF):
      its Linux backend (`libappindicator`) pulls the GTK3 stack the
      default CI build doesn't install. The daemon runs without it; a
      dedicated daemon-build CI job lands in Task 012. Approved by
      maintainer during Task 002._
- [x] `cargo build --workspace` green at default toolchain.
- [x] `cargo +1.95 build --workspace` green (MSRV gate; resolves Open
      Question around version pinning vs MSRV).
      _`rust-toolchain.toml` pins the active toolchain to 1.95, so the
      MSRV build is the default build; all deps resolve on 1.95._
- [x] `cargo clippy --workspace --all-targets -- -D warnings` clean
      (deps that drag pedantic warnings are either feature-gated off
      or `allow`'d with a justifying comment on the crate, not per-line).
      _Also verified `cargo clippy -p flintshot-app --features tray
      --all-targets -- -D warnings` clean._

## Verification

- [x] `cargo build --workspace && cargo +1.95 build --workspace` both
      exit 0. _(Same invocation — toolchain pinned to 1.95.)_
- [x] `cargo clippy --workspace --all-targets -- -D warnings` exit 0.
- [x] `cargo fmt --all --check` exit 0.
- [x] Note chosen versions in the CHANGELOG `[Unreleased]` Added
      section. _(New "Phase 2 (in progress)" subsection.)_

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
