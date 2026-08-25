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

# --- Install / uninstall (desktop integration) ---

# Install system-wide: release binary, launcher entry, and icon under
# PREFIX (default /usr/local). Needs write access to PREFIX — elevate
# externally, there is no sudo inside the recipe. Builds first.
#   sudo just install                      # -> /usr/local
#   sudo env PREFIX=/opt just install      # custom prefix, elevated
#   AUTOSTART=1 just install               # + per-user login autostart
# Rustup users: cargo lives in ~/.cargo/bin, which sudo's secure_path
# hides from root — preserve your PATH with:
#   sudo env "PATH=$PATH" just install
install:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "== build (release) =="
    cargo build --release
    just _deploy "${PREFIX:-/usr/local}"

# Install per-user, no elevation: release binary, launcher entry, and
# icon under ~/.local. Builds first.
#   just install-user                      # -> ~/.local
#   AUTOSTART=1 just install-user          # + ~/.config/autostart entry
install-user:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "== build (release) =="
    cargo build --release
    just _deploy "${HOME}/.local"

# Copy the built artifacts under a scope root, then refresh desktop /
# icon caches (graceful skip when those tools are missing). AUTOSTART=1
# adds the login-autostart entry — always per-user
# (~/.config/autostart), even for a system install. Private helper.
[private]
_deploy root:
    #!/usr/bin/env bash
    set -euo pipefail
    apps="{{ root }}/share/applications"
    icons="{{ root }}/share/icons/hicolor/scalable/apps"
    bins="{{ root }}/bin"

    mkdir -p "$bins" "$apps" "$icons"
    # --remove-destination: a pre-existing symlink (dev setups often
    # link ~/.local/bin/flintshot at target/release) makes cp refuse
    # with "same file"; replace it with a real copy instead.
    cp --remove-destination target/release/flintshot "$bins/flintshot"
    cp assets/applications/flintshot.desktop "$apps/flintshot.desktop"
    cp assets/icons/hicolor/scalable/apps/flintshot.svg "$icons/flintshot.svg"
    chmod 755 "$bins/flintshot"
    chmod 644 "$apps/flintshot.desktop" "$icons/flintshot.svg"
    echo "ok: binary + desktop entry + icon installed under {{ root }}"

    if [[ "${AUTOSTART:-0}" == "1" ]]; then
        # Autostart is per-user even for system installs; when
        # elevated, resolve the invoking user's home, not root's.
        home="$HOME"
        if [[ -n "${SUDO_USER:-}" && "${SUDO_USER}" != "root" ]]; then
            home="$(getent passwd "$SUDO_USER" | cut -d: -f6)"
        fi
        autostart_dir="${XDG_CONFIG_HOME:-$home/.config}/autostart"
        mkdir -p "$autostart_dir"
        cp assets/applications/flintshot-autostart.desktop "$autostart_dir/"
        chmod 644 "$autostart_dir/flintshot-autostart.desktop"
        echo "ok: autostart entry installed to $autostart_dir"
    fi

    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "$apps"
    else
        echo "ok: update-desktop-database not on PATH; skipping"
    fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache --quiet --force --ignore-theme-index \
            "${icons%/scalable/apps}" \
            || echo "ok: gtk-update-icon-cache failed; skipping (caches rebuild on demand)"
    else
        echo "ok: gtk-update-icon-cache not on PATH; skipping"
    fi

# Remove everything install / install-user put down — both scopes
# (PREFIX and ~/.local) plus the autostart entry. Safe to re-run when
# nothing is installed. System paths need the same elevation as
# install; PREFIX applies exactly as there:
#   just uninstall                         # user scope + /usr/local
#   sudo env PREFIX=/opt just uninstall    # elevated, custom prefix
uninstall:
    #!/usr/bin/env bash
    set -euo pipefail
    prefix="${PREFIX:-/usr/local}"
    # User-scope files belong to the invoking user, not root.
    home="$HOME"
    if [[ -n "${SUDO_USER:-}" && "${SUDO_USER}" != "root" ]]; then
        home="$(getent passwd "$SUDO_USER" | cut -d: -f6)"
    fi
    local_root="$home/.local"

    rm -f -- \
        "$prefix/bin/flintshot" \
        "$prefix/share/applications/flintshot.desktop" \
        "$prefix/share/icons/hicolor/scalable/apps/flintshot.svg" \
        "$local_root/bin/flintshot" \
        "$local_root/share/applications/flintshot.desktop" \
        "$local_root/share/icons/hicolor/scalable/apps/flintshot.svg" \
        "${XDG_CONFIG_HOME:-$home/.config}/autostart/flintshot-autostart.desktop"
    echo "ok: removed installed files from $prefix and $local_root (incl. autostart)"

    if command -v update-desktop-database >/dev/null 2>&1; then
        for d in "$prefix/share/applications" "$local_root/share/applications"; do
            if [[ -d "$d" ]]; then
                update-desktop-database "$d"
            fi
        done
    else
        echo "ok: update-desktop-database not on PATH; skipping"
    fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        for d in "$prefix/share/icons/hicolor" "$local_root/share/icons/hicolor"; do
            if [[ -d "$d" ]]; then
                gtk-update-icon-cache --quiet --force --ignore-theme-index "$d" \
                    || echo "ok: gtk-update-icon-cache failed; skipping"
            fi
        done
    else
        echo "ok: gtk-update-icon-cache not on PATH; skipping"
    fi

# --- Run subcommands (shortcuts for `cargo run -- <cmd>`) ---

# Show the CLI help (full subcommand surface).
help:
    cargo run -- --help

# Start the long-running daemon (tray, hotkeys, IPC server). [Phase 2]
# Extra args pass through, e.g. `just daemon --replace`.
daemon *args:
    cargo run -- daemon {{ args }}

# Capture a region and open the editor. Extra args pass through, e.g.
#   just region --monitor 1 --delay-ms 500
region *args:
    cargo run -- region {{ args }}

# Capture a full monitor and open the editor. Extra args pass through, e.g.
#   just fullscreen --monitor 1
fullscreen *args:
    cargo run -- fullscreen {{ args }}

# Print the resolved configuration as TOML to stdout.
config *args:
    cargo run -- config {{ args }}

# Open a pin-to-desktop window for an existing image. [Phase 3]
#   just pin foo.png --at 100,200 --size 800x600
pin path *args:
    cargo run -- pin {{ path }} {{ args }}

# Open an existing image in the editor (no capture). [Phase 3]
open path *args:
    cargo run -- open {{ path }} {{ args }}

# Upload an image to a backend and print the resulting URL. [Phase 4]
#   just upload foo.png --backend imgur
upload path *args:
    cargo run -- upload {{ path }} {{ args }}

# Set or clear credentials for an upload backend (OS keychain). [Phase 4]
#   just auth imgur            # set (hidden prompt; or IMGUR_CLIENT_ID env)
#   just auth imgur --clear    # remove the stored credential
auth backend *args:
    cargo run -- auth {{ backend }} {{ args }}

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
