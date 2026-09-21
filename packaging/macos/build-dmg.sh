#!/usr/bin/env bash
# Build flintshot's macOS universal .dmg: both arches, lipo'd into
# one binary inside a proper .app bundle, wrapped with hdiutil.
# cargo-dist has no universal-apple-darwin target (#77) and no native
# dmg — this custom job is the one artifact every macOS user downloads
# regardless of their chip.
#
# One script, two callers: CI (.github/workflows/package-macos-dmg.yml,
# via workflow_call from the cargo-dist release workflow) and local
# smoke (`just dist-build` on a Mac).
#
# Env:
#   FLINTSHOT_PACKAGE_NO_UPLOAD=1  build + checksum only, no upload
#                                  (local smoke / packaging-test.yml)
#   GH_TOKEN                       upload credentials (CI only)
#   GITHUB_REF_NAME                the v* tag being released (CI only)
set -euo pipefail
cd "$(dirname "$0")/../.."

fail() {
    echo "error: $*" >&2
    exit 1
}

[[ "$(uname -s)" == "Darwin" ]] || fail "this script must run on macOS (hdiutil/lipo)"

version="$(cargo pkgid --package flintshot-app | cut -d'#' -f2)"
[[ -n "$version" ]] || fail "could not read flintshot-app version"
out="target/packages"
app="flintshot.app"
dmg="flintshot_${version}_universal.dmg"
mkdir -p "$out"

# --- build both arches (tray on; macOS needs no GTK for it) ------------
rustup target add x86_64-apple-darwin aarch64-apple-darwin
for target in x86_64-apple-darwin aarch64-apple-darwin; do
    echo "== build (release, tray, ${target}) =="
    cargo build --release --features tray --target "$target" -q
done

echo "== lipo: universal flintshot =="
mkdir -p "${app}/Contents/MacOS"
lipo -create \
    "target/x86_64-apple-darwin/release/flintshot" \
    "target/aarch64-apple-darwin/release/flintshot" \
    -output "${app}/Contents/MacOS/flintshot"
lipo -info "${app}/Contents/MacOS/flintshot"

# --- icon: SVG -> png set -> icns (runner has brew; keep local runs
# working when librsvg is absent by skipping the icon gracefully) ------
if command -v rsvg-convert >/dev/null 2>&1; then
    iconset="${app}/Contents/Resources/flintshot.iconset"
    mkdir -p "$iconset"
    for size in 16 32 128 256 512; do
        rsvg-convert -w "$size" -h "$size" \
            assets/icons/hicolor/scalable/apps/flintshot.svg \
            -o "${iconset}/icon_${size}x${size}.png"
        rsvg-convert -w $((size * 2)) -h $((size * 2)) \
            assets/icons/hicolor/scalable/apps/flintshot.svg \
            -o "${iconset}/icon_${size}x${size}@2x.png"
    done
    iconutil -c icns "$iconset" -o "${app}/Contents/Resources/flintshot.icns"
    rm -rf "$iconset"
else
    echo "note: rsvg-convert not found; shipping .app without icns" >&2
fi

# --- Info.plist ----------------------------------------------------------
# LSUIElement: background tool + tray icon, no Dock presence; the
# editor windows are regular windows and appear regardless.
cat > "${app}/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key><string>flintshot</string>
    <key>CFBundleIdentifier</key><string>io.github.l1br3-io.flintshot</string>
    <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
    <key>CFBundleName</key><string>flintshot</string>
    <key>CFBundleDisplayName</key><string>flintshot</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>${version}</string>
    <key>CFBundleVersion</key><string>${version}</string>
    <key>CFBundleIconFile</key><string>flintshot</string>
    <key>LSMinimumSystemVersion</key><string>12.0</string>
    <key>LSUIElement</key><true/>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

# --- dmg -------------------------------------------------------------------
echo "== dmg: ${dmg} =="
rm -f "${out}/${dmg}"
hdiutil create -volname "flintshot" \
    -srcdir "$app" \
    -ov -format UDZO \
    -o "${out}/${dmg}" >/dev/null

(cd "$out" && { sha256sum "${dmg}" 2>/dev/null || shasum -a 256 "${dmg}"; } > "${dmg}.sha256")

# --- upload (release CI only) ------------------------------------------
if [[ "${FLINTSHOT_PACKAGE_NO_UPLOAD:-0}" != "1" ]]; then
    [[ -n "${GH_TOKEN:-}" ]] || fail "upload requested but GH_TOKEN is unset"
    [[ "${GITHUB_REF_NAME:-}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+ ]] \
        || fail "upload requested but GITHUB_REF_NAME '${GITHUB_REF_NAME:-}' is not a v* tag"
    echo "== upload to release ${GITHUB_REF_NAME} =="
    gh release upload "$GITHUB_REF_NAME" \
        "${out}/${dmg}" "${out}/${dmg}.sha256" \
        --repo l1br3-io/flintshot --clobber
fi

echo "ok: ${out}/${dmg}"
