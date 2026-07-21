# flintshot task runner. See https://github.com/casey/just
#
# Mirrors the CI gate (.github/workflows/ci.yml) so the same checks can
# be run locally with one word. All commands run from the repo root.
#
# Optional cargo subcommands (install with `just bootstrap`):
#   cargo-watch   - rebuild on save (used by `dev`, `watch`)
#   cargo-nextest - faster test runner (used by `nextest`, preferred by `test`)
# Recipes that need them fall back gracefully when absent.

# Default: list available recipes.
default:
    @just --list

# Install recommended cargo subcommands for local dev. Idempotent —
# re-runs are safe; already-installed binaries are skipped. Run once on
# a fresh checkout so `dev`, `watch`, and `nextest` work at full power.
bootstrap:
    #!/usr/bin/env bash
    set -euo pipefail
    install_if_missing() {
        local name="$1"
        if cargo "$name" --version >/dev/null 2>&1; then
            echo "ok: cargo-$name already installed ($(cargo "$name" --version))"
        else
            echo "installing: cargo-$name"
            cargo install "$name" --locked
        fi
    }
    install_if_missing watch
    install_if_missing nextest

# Auto-fix what CI checks: formatting and clippy.
fix:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "== fmt ==";    cargo fmt --all
    echo "== clippy =="; cargo clippy --fix --allow-dirty --all-targets -- -D warnings

# Run the same checks CI runs, locally. Fails fast on the first failing step.
# Mirrors the `lint` + `test` jobs in .github/workflows/ci.yml.
ci:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "== fmt --check =="; cargo fmt --all -- --check
    echo "== clippy ==";      cargo clippy --workspace --all-targets -- -D warnings
    echo "== test ==";        cargo test --workspace --all-targets
    echo "== test --doc ==";  cargo test --doc -p flintshot-core -p flintshot-editor-egui

# Run all tests. Prefers cargo-nextest for unit + integration (faster,
# nicer output); falls back to `cargo test` if nextest is absent.
# Doc tests always use `cargo test` — nextest doesn't run them.
test:
    #!/usr/bin/env bash
    set -euo pipefail
    if cargo nextest --version >/dev/null 2>&1; then
        echo "== nextest (unit + integration) =="
        cargo nextest run --workspace --all-targets
    else
        echo "== test (unit + integration) =="
        cargo test --workspace --all-targets
    fi
    echo "== test --doc (lib crates) =="
    cargo test --doc -p flintshot-core -p flintshot-editor-egui

# Test only the core crate (the heavy-unit-coverage one).
test-core:
    cargo test -p flintshot-core --all-targets
    cargo test -p flintshot-core --doc

# Run tests with cargo-nextest explicitly. Requires cargo-nextest —
# `just bootstrap` to install. Doc tests run via `cargo test` afterwards
# since nextest doesn't collect them.
nextest:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo nextest run --workspace --all-targets
    cargo test --doc -p flintshot-core -p flintshot-editor-egui

# Rebuild on save. Defaults to `cargo check`; pass a cargo subcommand
# (or several) to customize. Requires cargo-watch — `just bootstrap` to install.
#   just watch                              # cargo check on every save
#   just watch "test -p flintshot-core"     # run a single crate's tests
#   watch "fmt -- --check && clippy -- -D warnings"
watch args="check":
    cargo watch -x "{{ args }}"

# Verify the project builds on the pinned MSRV (1.95).
# Toolchain is selected automatically by rust-toolchain.toml.
msrv:
    cargo build --all-targets

# Build the opt-in COSMIC applet. Needs system deps; see AGENTS.md.
# On Debian/Ubuntu: sudo apt-get install -y --no-install-recommends \
#   cmake libxkbcommon-dev libfontconfig-dev libfreetype6-dev \
#   libglib2.0-dev libgfxbuffer-dev pkg-config
cosmic:
    cargo build -p flintshot-cosmic

# --- Run subcommands (shortcuts for `cargo run -- <cmd>`) ---

# Show the CLI help (full subcommand surface).
help:
    cargo run -- --help

# Start the long-running daemon (tray, hotkeys, IPC server). [Phase 2]
daemon:
    cargo run -- daemon

# Capture a region and open the editor. Extra args pass through, e.g.
#   just region --monitor 1 --delay-ms 500
region *args:
    cargo run -- region {{ args }}

# Capture a full monitor and open the editor. Extra args pass through, e.g.
#   just fullscreen --monitor 1
fullscreen *args:
    cargo run -- fullscreen {{ args }}

# Open a pin-to-desktop window for an existing image. [Phase 3]
pin path *args:
    cargo run -- pin {{ path }} {{ args }}

# Upload an image to a backend and print the resulting URL. [Phase 4]
#   just upload foo.png --backend imgur
upload path *args:
    cargo run -- upload {{ path }} {{ args }}

# --- Dev loop ---

# Dev mode: auto-rebuild on save. Requires `cargo-watch`; falls back to a
# plain `cargo run` if it's not installed. Extra args pass through.
# Ctrl-C kills the process.
dev *args:
    #!/usr/bin/env bash
    set -euo pipefail
    if cargo watch --version >/dev/null 2>&1; then
        cargo watch -x "run -- {{ args }}"
    else
        echo "note: cargo-watch not installed; backend won't auto-reload. Run 'just bootstrap' or 'cargo install cargo-watch' to enable." >&2
        cargo run -- {{ args }}
    fi
