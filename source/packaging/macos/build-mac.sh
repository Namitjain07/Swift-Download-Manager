#!/usr/bin/env bash
# Builds the Mac app (works on both Apple Silicon and Intel Macs) and a .dmg.
# Must be run on a Mac with Rust installed (https://rustup.rs) and Xcode's
# command line tools (xcode-select --install).
# Run from the app folder:  bash packaging/macos/build-mac.sh
# The result goes to dist/.
set -euo pipefail
cd "$(dirname "$0")/../.."

VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
APP="dist/mac/Swift Download Manager.app"
DMG="dist/SwiftDownloadManager-${VERSION}-mac.dmg"

rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin

rm -rf "dist/mac"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
# One app that runs on both kinds of Mac.
lipo -create -output "$APP/Contents/MacOS/SwiftDownloadManager" \
  target/aarch64-apple-darwin/release/swift_download_manager \
  target/x86_64-apple-darwin/release/swift_download_manager
sed "s/VERSION/${VERSION}/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"

# App icon (.icns) from the icon the app draws itself.
ICONSET="dist/mac/AppIcon.iconset"
mkdir -p "$ICONSET"
BIN="$APP/Contents/MacOS/SwiftDownloadManager"
for s in 16 32 128 256 512; do
  "$BIN" --write-icon "$ICONSET/icon_${s}x${s}.png" "$s"
  "$BIN" --write-icon "$ICONSET/icon_${s}x${s}@2x.png" "$((s * 2))"
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
rm -rf "$ICONSET"

# Not signed with an Apple developer account: sign locally so macOS will run it.
codesign --force --deep --sign - "$APP"

mkdir -p dist/mac/dmg
cp -R "$APP" dist/mac/dmg/
ln -s /Applications dist/mac/dmg/Applications
cp -R browser-extension "dist/mac/dmg/Browser add-on"
rm -f "$DMG"
hdiutil create -volname "Swift Download Manager" -srcfolder dist/mac/dmg -ov -format UDZO "$DMG"
echo "Made $DMG"
echo "First start: right-click the app and choose Open (it isn't signed by an Apple developer account)."
