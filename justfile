# flintshot task runner. See https://github.com/casey/just
#
# Mirrors the CI gate (.github/workflows/ci.yml) so the same checks can
# be run locally with one word. All commands run from the repo root.

# Default: list available recipes.
default:
    @just --list

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
    echo "== clippy ==";      cargo clippy --all-targets -- -D warnings
    echo "== test ==";        cargo test --all-targets
    echo "== test --doc ==";  cargo test --doc

# Run all tests: unit + integration + doc.
test:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "== test ==";      cargo test --all-targets
    echo "== test --doc =="; cargo test --doc

# Test only the core crate (the heavy-unit-coverage one).
test-core:
    cargo test -p flintshot-core --all-targets
    cargo test -p flintshot-core --doc

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
        echo "note: cargo-watch not installed; backend won't auto-reload. 'cargo install cargo-watch' to enable." >&2
        cargo run -- {{ args }}
    fi
