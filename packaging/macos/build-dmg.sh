#!/usr/bin/env bash
# Cantus for macOS: a universal Cantus.app in a .dmg (ADR 0066).
#
#   packaging/macos/build-dmg.sh <version> <out-dir>
#
# Run on a Mac (GitHub's macos-latest) from the repository's root, with Xcode's command-line
# tools and the two Rust targets (`rustup target add aarch64-apple-darwin
# x86_64-apple-darwin`). It builds Cantus, cantus-scan and cantus-sandbox for Apple silicon
# and Intel, joins each pair with `lipo`, makes Cantus.app (Info.plist, icon, the
# microphone's reason in English and Spanish, the manual and the licences in Resources),
# signs it with the hardened runtime and writes <out-dir>/Cantus-<version>-macos-universal.dmg.
#
# Signing and notarisation happen only when all five secrets are in the environment:
#   APPLE_CERT_P12_BASE64  the "Developer ID Application" certificate and key (.p12), base64
#   APPLE_CERT_PASSWORD    the .p12's password
#   APPLE_TEAM_ID          the team's ten-character id
#   APPLE_ID               the Apple account notarytool signs in with
#   APPLE_APP_PASSWORD     an app-specific password of that account
# Without them everything is signed ad hoc (`codesign -s -`): the .dmg opens on the Mac that
# built it, and elsewhere after right-click > Open (Gatekeeper's warning for unsigned apps).
#
# CANTUS_DMG_NO_BUILD=1 skips the cargo builds (the binaries already in target/).
set -euo pipefail

version="${1:?usage: build-dmg.sh <version> <out-dir>}"
out="${2:?usage: build-dmg.sh <version> <out-dir>}"
here="$(cd "$(dirname "$0")" && pwd)"
root="$(pwd)"
[ -f "$root/Cargo.toml" ] || { echo "run from the repository's root" >&2; exit 2; }
mkdir -p "$out"
out="$(cd "$out" && pwd)"

targets=(aarch64-apple-darwin x86_64-apple-darwin)
bins=(cantus cantus-scan cantus-sandbox)
# The oldest macOS Cantus runs on (Info.plist's LSMinimumSystemVersion says the same).
export MACOSX_DEPLOYMENT_TARGET=11.0

work="$root/target/macos-dmg"
rm -rf "$work"
mkdir -p "$work"

# ---- build ---------------------------------------------------------------------------
icon_dir=""
for t in "${targets[@]}"; do
  if [ -z "${CANTUS_DMG_NO_BUILD:-}" ]; then
    cargo build --release --locked --target "$t" -p cantus-host --bin cantus-scan
    cargo build --release --locked --target "$t" -p cantus-sandbox
  fi
  # The app last, reading where its build script wrote the icon (cantus-app/build.rs).
  dir="$(cargo build --release --locked --target "$t" -p cantus --message-format=json-render-diagnostics |
    python3 -c 'import json, sys
for line in sys.stdin:
    m = json.loads(line)
    if m.get("reason") == "build-script-executed" and "cantus-app" in m.get("package_id", ""):
        print(m["out_dir"])')"
  [ -n "$icon_dir" ] || icon_dir="$dir"
done
[ -f "$icon_dir/Cantus.icns" ] || { echo "no Cantus.icns in $icon_dir" >&2; exit 1; }

# ---- the bundle ----------------------------------------------------------------------
app="$work/Cantus.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
for b in "${bins[@]}"; do
  lipo -create \
    "$root/target/aarch64-apple-darwin/release/$b" \
    "$root/target/x86_64-apple-darwin/release/$b" \
    -output "$app/Contents/MacOS/$b"
  lipo -info "$app/Contents/MacOS/$b"
done
short="${version%%-*}"
sed -e "s/@SHORT_VERSION@/$short/g" -e "s/@VERSION@/$version/g" \
  "$here/Info.plist" > "$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist"
printf 'APPL????' > "$app/Contents/PkgInfo"
cp "$icon_dir/Cantus.icns" "$app/Contents/Resources/Cantus.icns"
for l in en es; do
  mkdir -p "$app/Contents/Resources/$l.lproj"
  cp "$here/$l.lproj/InfoPlist.strings" "$app/Contents/Resources/$l.lproj/"
done
# What the Windows installer puts beside the program: the licences, the notices, the
# privacy note and the manual (the app reads it from Resources: cantus-ui/src/docs.rs).
res="$app/Contents/Resources"
cp "$root/LICENSE-GPL" "$res/LICENSE.txt"
cp "$root/LICENSE" "$res/COPYRIGHT.txt"
cp "$root/THIRD-PARTY-NOTICES.md" "$res/THIRD-PARTY-NOTICES.txt"
cp "$root/PRIVACY.md" "$res/PRIVACY.txt"
cargo run --locked -p xtask -- manual
mkdir -p "$res/manual"
cp -R "$root/target/manual/." "$res/manual/"

# ---- signing -------------------------------------------------------------------------
entitlements="$here/Cantus.entitlements"
signed=""
if [ -n "${APPLE_CERT_P12_BASE64:-}" ] && [ -n "${APPLE_CERT_PASSWORD:-}" ] &&
  [ -n "${APPLE_TEAM_ID:-}" ] && [ -n "${APPLE_ID:-}" ] && [ -n "${APPLE_APP_PASSWORD:-}" ]; then
  signed=1
fi

keychain=""
cleanup() {
  if [ -n "$keychain" ]; then
    security delete-keychain "$keychain" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

if [ -n "$signed" ]; then
  keychain="$work/signing.keychain-db"
  kc_pass="$(uuidgen)"
  echo "$APPLE_CERT_P12_BASE64" | base64 --decode > "$work/cert.p12"
  security create-keychain -p "$kc_pass" "$keychain"
  security set-keychain-settings -lut 3600 "$keychain"
  security unlock-keychain -p "$kc_pass" "$keychain"
  security import "$work/cert.p12" -k "$keychain" -P "$APPLE_CERT_PASSWORD" \
    -T /usr/bin/codesign -T /usr/bin/security
  rm -f "$work/cert.p12"
  security set-key-partition-list -S apple-tool:,apple: -s -k "$kc_pass" "$keychain" >/dev/null
  security list-keychains -d user -s "$keychain" $(security list-keychains -d user | tr -d '"')
  identity="$(security find-identity -v -p codesigning "$keychain" |
    sed -n 's/.*"\(Developer ID Application: .*('"$APPLE_TEAM_ID"')\)".*/\1/p' | head -n 1)"
  [ -n "$identity" ] || { echo "no Developer ID Application identity for $APPLE_TEAM_ID" >&2; exit 1; }
  sign=(codesign --force --options runtime --timestamp --entitlements "$entitlements"
    --keychain "$keychain" --sign "$identity")
else
  echo "No Apple signing secrets: signing ad hoc."
  sign=(codesign --force --options runtime --entitlements "$entitlements" --sign -)
fi

# Inside out: the helpers, then the bundle (which signs its main executable).
"${sign[@]}" "$app/Contents/MacOS/cantus-scan"
"${sign[@]}" "$app/Contents/MacOS/cantus-sandbox"
"${sign[@]}" "$app"
codesign --verify --strict --verbose=2 "$app"

notarise() {
  xcrun notarytool submit "$1" --apple-id "$APPLE_ID" --password "$APPLE_APP_PASSWORD" \
    --team-id "$APPLE_TEAM_ID" --wait --timeout 30m
}

if [ -n "$signed" ]; then
  # The app first, so its ticket is stapled to it and it opens offline once copied out.
  ditto -c -k --keepParent "$app" "$work/Cantus.zip"
  notarise "$work/Cantus.zip"
  xcrun stapler staple "$app"
fi

# ---- the disk image ------------------------------------------------------------------
stage="$work/dmg"
mkdir -p "$stage"
cp -R "$app" "$stage/"
ln -s /Applications "$stage/Applications"
dmg="$out/Cantus-$version-macos-universal.dmg"
rm -f "$dmg"
hdiutil create -volname "Cantus $version" -srcfolder "$stage" -fs HFS+ -format UDZO \
  -ov "$dmg"

if [ -n "$signed" ]; then
  codesign --force --timestamp --keychain "$keychain" --sign "$identity" "$dmg"
  notarise "$dmg"
  xcrun stapler staple "$dmg"
  spctl --assess --type open --context context:primary-signature --verbose=2 "$dmg"
fi

echo "$dmg"
