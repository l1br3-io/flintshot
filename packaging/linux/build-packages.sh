#!/usr/bin/env bash
# Build flintshot's Linux packages (.deb today; .rpm/.AppImage are
# added by later tasks in this directory). One script, two callers:
# CI (.github/workflows/package-linux.yml, via workflow_call from the
# cargo-dist release workflow) and local smoke (`just dist-build`).
#
# Env:
#   FLINTSHOT_PACKAGE_NO_UPLOAD=1  build + checksum only, no upload
#                                  (local smoke / packaging-test.yml)
#   GH_TOKEN                       upload credentials (CI only)
#   GITHUB_REF_NAME                the v* tag being released (CI only)
#
# Outputs land in target/packages/ — NOT target/dist/, which
# cargo-dist 0.32 claims as its cargo build target-dir, and not
# target/distrib/, where it stages release artifacts.
#
# NOTE: CI builds inside a debian:12 container (glibc 2.36 + PipeWire
# 0.3.65 headers — the oldest set xcap 0.9's pipewire stack compiles
# against; ubuntu-22.04's 0.3.48 fails). A locally built deb links the
# local glibc — fine for smoke tests, not for shipping.
set -euo pipefail
cd "$(dirname "$0")/../.."

fail() {
    echo "error: $*" >&2
    exit 1
}

version="$(cargo pkgid --package flintshot-app | cut -d'#' -f2)"
[[ -n "$version" ]] || fail "could not read flintshot-app version"
out="target/packages"
out_abs="$(pwd)/${out}"
mkdir -p "$out"

# --- toolchain: pinned packagers (idempotent) ---------------------------
if ! command -v cargo-deb >/dev/null 2>&1; then
    echo "== installing cargo-deb (pinned 3.8.0) =="
    cargo install cargo-deb@3.8.0 --locked
fi
if ! command -v cargo-generate-rpm >/dev/null 2>&1; then
    echo "== installing cargo-generate-rpm (pinned 0.21.0) =="
    cargo install cargo-generate-rpm@0.21.0 --locked
fi

# --- .deb --------------------------------------------------------------
# Build first with the tray feature so the packaged binary is exactly
# what we smoke-tested; --no-build keeps cargo-deb from rebuilding.
echo "== build (release, tray) =="
cargo build --release --features tray -q

echo "== deb: flintshot_${version}_amd64.deb =="
cargo deb --no-build --package flintshot-app --output "${out}/flintshot_${version}_amd64.deb"

(cd "$out" && sha256sum "flintshot_${version}_amd64.deb" > "flintshot_${version}_amd64.deb.sha256")

# --- .rpm ----------------------------------------------------------------
# Fedora-side names for the same runtime set as the deb's Depends
# (see [package.metadata.generate-rpm] for the mapping rationale).
# Run from the package dir: cargo-generate-rpm resolves asset paths
# relative to the manifest it runs against (unlike cargo-deb, which
# resolves against the workspace target dir).
echo "== rpm: flintshot-${version}-1.x86_64.rpm =="
(
    cd crates/flintshot-app
    cargo generate-rpm --output "${out_abs}/flintshot-${version}-1.x86_64.rpm"
)

(cd "$out" && sha256sum "flintshot-${version}-1.x86_64.rpm" > "flintshot-${version}-1.x86_64.rpm.sha256")

# --- .AppImage ------------------------------------------------------------
# linuxdeploy traces linked deps (ldd) into an AppDir, then wraps it.
# The GTK/appindicator tray stack is the risky part: libgtk-3 is
# ldd-visible (traced), but libayatana-appindicator3 is dlopen'd by
# tray-icon and invisible to tracing — deployed explicitly with
# --deploy-deps-only. libpipewire is likewise dlopen'd (xcap capture)
# but deliberately NOT bundled: it talks to the session's pipewire
# daemon, and a bundled client lib newer than the host daemon breaks
# the protocol — rely on the host's (standard AppImage practice).
# --appimage-extract-and-run: no FUSE in containers/CI.
appimage="flintshot_${version}_x86_64.AppImage"
linuxdeploy_tag="1-alpha-20251107-1"
if [[ ! -x "${out}/.tools/linuxdeploy" ]]; then
    echo "== downloading linuxdeploy (${linuxdeploy_tag}) =="
    mkdir -p "${out}/.tools"
    curl -fsSL "https://github.com/linuxdeploy/linuxdeploy/releases/download/${linuxdeploy_tag}/linuxdeploy-x86_64.AppImage" \
        -o "${out}/.tools/linuxdeploy"
    chmod +x "${out}/.tools/linuxdeploy"
fi

appdir="${out}/.appdir"
rm -rf "$appdir"
mkdir -p \
    "${appdir}/usr/bin" \
    "${appdir}/usr/lib" \
    "${appdir}/usr/share/applications" \
    "${appdir}/usr/share/icons/hicolor/scalable/apps"
cp target/release/flintshot "${appdir}/usr/bin/"
cp assets/applications/flintshot.desktop "${appdir}/usr/share/applications/"
cp assets/icons/hicolor/scalable/apps/flintshot.svg \
    "${appdir}/usr/share/icons/hicolor/scalable/apps/"
# dlopen'd appindicator: copy it in, then deploy *its* deps (the arg
# must name a file already inside the AppDir).
appindicator_lib="$(ldconfig -p | awk '/libayatana-appindicator3.so\.1 /{print $NF; exit}')"
[[ -n "$appindicator_lib" ]] || fail "libayatana-appindicator3.so.1 not found via ldconfig"
cp -L "$appindicator_lib" "${appdir}/usr/lib/"

echo "== appimage: ${appimage} =="
OUTPUT="$(pwd)/${out}/${appimage}" \
"${out}/.tools/linuxdeploy" --appimage-extract-and-run \
    --appdir "$appdir" \
    --desktop-file "${appdir}/usr/share/applications/flintshot.desktop" \
    --icon-file "${appdir}/usr/share/icons/hicolor/scalable/apps/flintshot.svg" \
    --deploy-deps-only "${appdir}/usr/lib/$(basename "$appindicator_lib")" \
    --output appimage

(cd "$out" && sha256sum "${appimage}" > "${appimage}.sha256")

# --- upload (release CI only) ------------------------------------------
if [[ "${FLINTSHOT_PACKAGE_NO_UPLOAD:-0}" != "1" ]]; then
    [[ -n "${GH_TOKEN:-}" ]] || fail "upload requested but GH_TOKEN is unset"
    [[ "${GITHUB_REF_NAME:-}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+ ]] \
        || fail "upload requested but GITHUB_REF_NAME '${GITHUB_REF_NAME:-}' is not a v* tag"
    # The debian:12 container has no gh; install the pinned static
    # binary when missing (the ubuntu runner has its own — leave it).
    if ! command -v gh >/dev/null 2>&1; then
        gh_ver="2.101.0"
        echo "== installing gh ${gh_ver} (container) =="
        curl -fsSL "https://github.com/cli/cli/releases/download/v${gh_ver}/gh_${gh_ver}_linux_amd64.tar.gz" \
            | tar -xz -C /usr/local --strip-components=1 "gh_${gh_ver}_linux_amd64/bin/gh"
    fi
    echo "== upload to release ${GITHUB_REF_NAME} =="
    gh release upload "$GITHUB_REF_NAME" \
        "${out}/flintshot_${version}_amd64.deb" \
        "${out}/flintshot_${version}_amd64.deb.sha256" \
        "${out}/flintshot-${version}-1.x86_64.rpm" \
        "${out}/flintshot-${version}-1.x86_64.rpm.sha256" \
        "${out}/flintshot_${version}_x86_64.AppImage" \
        "${out}/flintshot_${version}_x86_64.AppImage.sha256" \
        --repo l1br3-io/flintshot --clobber
fi

echo "ok: linux packages in ${out}/"
