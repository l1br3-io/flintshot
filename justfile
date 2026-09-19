# flintshot task runner. See https://github.com/casey/just
#
# Mirrors the CI gate (.github/workflows/ci.yml) so the same checks can
# be run locally with one word. All commands run from the repo root.
#
# Cargo invocations are quiet by default (`-q`, nextest:
# `--cargo-quiet`): silent on success, loud on failure — compile
# errors, test failures, and remaining warnings still print in full.
# Keeps output (and token cost, when an agent runs these) minimal.
# `watch` / `dev` stay verbose: watch loops want visible rebuild
# feedback.
#
# Optional cargo subcommands (install with `just bootstrap`):
#   cargo-watch   - rebuild on save (used by `dev`, `watch`)
#   cargo-nextest - faster test runner (used by `nextest`, preferred by `test`)
# Recipes that need them fall back gracefully when absent.

# Pass recipe arguments to recipe scripts as positional parameters
# ($1, $2, …) so `daemon` / `dev` can intercept their `--refresh` flag
# from "$@" before forwarding the rest. Recipes that only use {{ … }}
# interpolation are unaffected.
set positional-arguments

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
            cargo install -q "$name" --locked
            echo "ok: cargo-$name installed ($(cargo "$name" --version))"
        fi
    }
    install_if_missing watch
    install_if_missing nextest

# Auto-fix what CI checks: formatting and clippy.
fix:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "== fmt ==";    cargo fmt --all
    echo "== clippy =="; cargo clippy -q --fix --allow-dirty --all-targets -- -D warnings

# Run the same checks CI runs, locally. Fails fast on the first failing step.
# Mirrors the `lint` + `test` jobs in .github/workflows/ci.yml.
ci:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "== fmt --check =="; cargo fmt --all -- --check
    echo "== clippy ==";      cargo clippy -q --workspace --all-targets -- -D warnings
    echo "== test ==";        cargo test -q --workspace --all-targets
    echo "== test --doc ==";  cargo test -q --doc -p flintshot-core -p flintshot-editor-egui

# Run all tests. Prefers cargo-nextest for unit + integration (faster,
# nicer output); falls back to `cargo test` if nextest is absent.
# Doc tests always use `cargo test` — nextest doesn't run them.
# Quiet everywhere: `-q` / `--cargo-quiet` hide compile chatter and
# per-test lines; failures still print in full.
test:
    #!/usr/bin/env bash
    set -euo pipefail
    if cargo nextest --version >/dev/null 2>&1; then
        echo "== nextest (unit + integration) =="
        cargo nextest run --cargo-quiet --workspace --all-targets
    else
        echo "== test (unit + integration) =="
        cargo test -q --workspace --all-targets
    fi
    echo "== test --doc (lib crates) =="
    cargo test -q --doc -p flintshot-core -p flintshot-editor-egui

# Test only the core crate (the heavy-unit-coverage one).
test-core:
    cargo test -q -p flintshot-core --all-targets
    cargo test -q -p flintshot-core --doc

# Run tests with cargo-nextest explicitly. Requires cargo-nextest —
# `just bootstrap` to install. Doc tests run via `cargo test` afterwards
# since nextest doesn't collect them.
nextest:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo nextest run --cargo-quiet --workspace --all-targets
    cargo test -q --doc -p flintshot-core -p flintshot-editor-egui

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
    cargo build -q --all-targets

# Build the opt-in COSMIC applet. Needs system deps; see AGENTS.md.
# On Debian/Ubuntu: sudo apt-get install -y --no-install-recommends \
#   cmake libxkbcommon-dev libfontconfig-dev libfreetype6-dev \
#   libglib2.0-dev libgfxbuffer-dev pkg-config
cosmic:
    cargo build -q -p flintshot-cosmic

# --- Install / uninstall (desktop integration) ---

# Install system-wide: release binary, launcher entry, and icon under
# PREFIX (default /usr/local). Needs write access to PREFIX — elevate
# externally, there is no sudo inside the recipe. Builds first with
# the tray feature on (FEATURES= overrides; FEATURES='' for minimal).
#   sudo just install                      # -> /usr/local
#   sudo env PREFIX=/opt just install      # custom prefix, elevated
#   AUTOSTART=1 just install               # + per-user login autostart
#   sudo env FEATURES='' just install      # minimal build, no GTK deps
# Rustup users: cargo lives in ~/.cargo/bin, which sudo's secure_path
# hides from root — preserve your PATH with:
#   sudo env "PATH=$PATH" just install
# `--refresh` restarts the daemon from the installed binary — but a
# daemon spawned from an elevated recipe would sit on root's runtime
# dir (duplicate IPC socket) and the wrong session bus, so the restart
# is skipped with the exact command to run from your session instead.
# Un-elevated (`PREFIX=$HOME/.local just install --refresh`) restarts.
install *args:
    #!/usr/bin/env bash
    set -euo pipefail
    refresh=0
    for a in "$@"; do
        if [[ "$a" == "--refresh" ]]; then
            refresh=1
        else
            echo "error: unrecognized argument '$a' (only --refresh)" >&2
            exit 2
        fi
    done
    just _build
    just _deploy "${PREFIX:-/usr/local}"
    if [[ "$refresh" == 1 ]]; then
        just _restart-daemon "${PREFIX:-/usr/local}/bin/flintshot"
    fi

# Install per-user, no elevation: release binary, launcher entry, and
# icon under ~/.local. Builds first with the tray feature on
# (FEATURES= overrides; FEATURES='' for minimal).
#   just install-user                      # -> ~/.local
#   AUTOSTART=1 just install-user          # + ~/.config/autostart entry
#   FEATURES='' just install-user          # minimal build, no GTK deps
# `--refresh` is the single-command update: rebuild + redeploy +
# restart the daemon from the installed binary (`--replace` takes
# over from any running instance, dev-loop debug daemon included).
#   just install-user --refresh
install-user *args:
    #!/usr/bin/env bash
    set -euo pipefail
    refresh=0
    for a in "$@"; do
        if [[ "$a" == "--refresh" ]]; then
            refresh=1
        else
            echo "error: unrecognized argument '$a' (only --refresh)" >&2
            exit 2
        fi
    done
    just _build
    just _deploy "${HOME}/.local"
    if [[ "$refresh" == 1 ]]; then
        just _restart-daemon "${HOME}/.local/bin/flintshot"
    fi

# Build the release binary for the install recipes. FEATURES selects
# the feature set: unset -> tray (the launcher promise: daemon +
# tray), '' -> minimal (no tray; no GTK build deps), 'a,b' -> passed
# through verbatim. Private helper for install / install-user.
[private]
_build:
    #!/usr/bin/env bash
    set -euo pipefail
    # ${FEATURES-tray}, not ${FEATURES:-tray}: unset -> "tray",
    # set-but-empty -> "" (minimal). The :- form would make the
    # FEATURES='' escape hatch impossible.
    features="${FEATURES-tray}"
    if [[ -n "$features" ]]; then
        echo "== build (release, features: ${features}) =="
        if ! cargo build -q --release --features "$features"; then
            # Comma-boundary match so a feature like 'mytray' can't
            # trigger the hint.
            if [[ ",${features}," == *",tray,"* ]]; then
                echo "hint: the 'tray' feature needs the GTK dev libs at build time" >&2
                echo "      (Debian/Ubuntu: libgtk-3-dev libayatana-appindicator3-dev)" >&2
                echo "      Install them, or re-run with FEATURES='' for a minimal no-tray build" >&2
            fi
            exit 1
        fi
    else
        echo "== build (release, minimal: no features) =="
        cargo build -q --release
    fi

# Debug-profile build backing the `--refresh` flag of `daemon` / `dev`.
# FEATURES selects the feature set exactly as in _build: unset -> tray
# (the refresh promise: a tray icon at the end), '' -> minimal, 'a,b' ->
# passed through verbatim. Debug, not release, so the dev loop stays
# fast; the later `cargo run` reuses this build. Private helper.
[private]
_refresh-build:
    #!/usr/bin/env bash
    set -euo pipefail
    features="${FEATURES-tray}"
    if [[ -n "$features" ]]; then
        echo "== build (debug, features: ${features}) =="
        if ! cargo build -q --features "$features"; then
            # Comma-boundary match so a feature like 'mytray' can't
            # trigger the hint.
            if [[ ",${features}," == *",tray,"* ]]; then
                echo "hint: the 'tray' feature needs the GTK dev libs at build time" >&2
                echo "      (Debian/Ubuntu: libgtk-3-dev libayatana-appindicator3-dev)" >&2
                echo "      Install them, or re-run with FEATURES='' for a minimal no-tray build" >&2
            fi
            exit 1
        fi
    else
        echo "== build (debug, minimal: no features) =="
        cargo build -q
    fi

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

# Restart the daemon from a freshly installed binary, for the install
# recipes' `--refresh` flag. `--replace` asks any running daemon to
# quit over IPC and takes over its socket (a dev-loop debug daemon
# included) or starts fresh when none is running. Detached (setsid,
# stdio appended to a state-dir log) so just returns at once and the
# daemon outlives the terminal. Skips under elevation: a root-spawned
# daemon would resolve XDG_RUNTIME_DIR to root's — a second IPC socket
# next to the user session's — and miss the session bus entirely.
# Private helper.
[private]
_restart-daemon bin:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ -n "${SUDO_USER:-}" && "${SUDO_USER}" != "root" ]]; then
        echo "skip: elevated install — a root-spawned daemon would land on the wrong session" >&2
        echo "      restart from your session instead: {{ bin }} daemon --replace" >&2
        exit 0
    fi
    log_dir="${XDG_STATE_HOME:-$HOME/.local/state}/flintshot"
    mkdir -p "$log_dir"
    setsid "{{ bin }}" daemon --replace \
        </dev/null >>"$log_dir/daemon.log" 2>&1 &
    echo "ok: daemon restarting from {{ bin }} (log: $log_dir/daemon.log)"

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
    cargo run -q -- --help

# Start the long-running daemon (tray, hotkeys, IPC server). [Phase 2]
# Extra args pass through, e.g. `just daemon --replace`.
# `--refresh` = full refresh after code changes: rebuild first with
# the tray feature (FEATURES= overrides; FEATURES='' minimal), then
# restart any running daemon — `--replace` is appended for you when
# not already given. The pre-build makes the `cargo run` instant.
#   just daemon --refresh
daemon *args:
    #!/usr/bin/env bash
    set -euo pipefail
    refresh=0
    pass=()
    for a in "$@"; do
        if [[ "$a" == "--refresh" ]]; then refresh=1; else pass+=("$a"); fi
    done
    if [[ "$refresh" == 1 ]]; then
        just _refresh-build
        if [[ " ${pass[*]-} " != *" --replace "* ]]; then
            pass+=(--replace)
        fi
    fi
    exec cargo run -q -- daemon "${pass[@]}"

# Capture a region and open the editor. Extra args pass through, e.g.
#   just region --monitor 1 --delay-ms 500
region *args:
    cargo run -q -- region {{ args }}

# Capture a full monitor and open the editor. Extra args pass through, e.g.
#   just fullscreen --monitor 1
fullscreen *args:
    cargo run -q -- fullscreen {{ args }}

# Print the resolved configuration as TOML to stdout.
config *args:
    cargo run -q -- config {{ args }}

# Open an existing image in the editor (no capture). [Phase 3]
open path *args:
    cargo run -q -- open {{ path }} {{ args }}

# Upload an image to a backend and print the resulting URL. [Phase 4]
#   just upload foo.png --backend imgur
upload path *args:
    cargo run -q -- upload {{ path }} {{ args }}

# Set or clear credentials for an upload backend (OS keychain). [Phase 4]
#   just auth imgur            # set (hidden prompt; or IMGUR_CLIENT_ID env)
#   just auth imgur --clear    # remove the stored credential
auth backend *args:
    cargo run -q -- auth {{ backend }} {{ args }}

# --- Dev loop ---

# Dev mode: auto-rebuild on save. Requires `cargo-watch`; falls back to a
# plain `cargo run` if it's not installed. Extra args pass through.
# `--refresh` keeps the tray in the loop: every (re)build uses the tray
# feature (FEATURES= overrides; FEATURES='' minimal), and — when the
# target is the daemon — `--replace` is appended so each restart takes
# over from the previous instance instead of trying to join it.
# Ctrl-C kills the process.
#   just dev daemon --refresh     # watch-rebuild the daemon, tray on
#   just dev app                  # plain dev loop, no tray
dev *args:
    #!/usr/bin/env bash
    set -euo pipefail
    refresh=0
    pass=()
    for a in "$@"; do
        if [[ "$a" == "--refresh" ]]; then refresh=1; else pass+=("$a"); fi
    done
    run_args=("${pass[@]}")
    feat=()
    if [[ "$refresh" == 1 ]]; then
        just _refresh-build
        features="${FEATURES-tray}"
        if [[ -n "$features" ]]; then
            feat=(--features "$features")
        fi
        if [[ "${pass[0]-}" == "daemon" && " ${pass[*]-} " != *" --replace "* ]]; then
            run_args+=(--replace)
        fi
    fi
    if cargo watch --version >/dev/null 2>&1; then
        cargo watch -x "run ${feat[*]-} -- ${run_args[*]-}"
    else
        echo "note: cargo-watch not installed; backend won't auto-reload. Run 'just bootstrap' or 'cargo install cargo-watch' to enable." >&2
        cargo run "${feat[@]}" -- "${run_args[@]}"
    fi
