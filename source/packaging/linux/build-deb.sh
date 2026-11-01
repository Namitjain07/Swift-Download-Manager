#!/usr/bin/env bash
# Builds the Linux installer (.deb) for Ubuntu, Debian, Mint and similar.
# Run from the app folder:  bash packaging/linux/build-deb.sh
# The result is written to dist/.
set -euo pipefail
cd "$(dirname "$0")/../.."

VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
ARCH=$(dpkg --print-architecture 2>/dev/null || echo amd64)
NAME=swift-download-manager
PKG="dist/deb/${NAME}_${VERSION}_${ARCH}"

cargo build --release

rm -rf "$PKG"
mkdir -p "$PKG/DEBIAN" "$PKG/usr/bin" "$PKG/usr/share/applications" \
         "$PKG/usr/share/$NAME/browser-extension" "$PKG/usr/share/doc/$NAME"
install -m 755 target/release/swift_download_manager "$PKG/usr/bin/$NAME"
install -m 644 packaging/linux/$NAME.desktop "$PKG/usr/share/applications/$NAME.desktop"
cp -r browser-extension/. "$PKG/usr/share/$NAME/browser-extension/"
chmod -R u=rwX,go=rX "$PKG/usr/share/$NAME/browser-extension"
install -m 644 README.md "$PKG/usr/share/doc/$NAME/README.md"
for size in 32 48 64 128 256 512; do
  dir="$PKG/usr/share/icons/hicolor/${size}x${size}/apps"
  mkdir -p "$dir"
  target/release/swift_download_manager --write-icon "$dir/$NAME.png" "$size"
done

SIZE_KB=$(du -sk "$PKG/usr" | cut -f1)
# The oldest system library version this build works with.
GLIBC=$(objdump -T target/release/swift_download_manager 2>/dev/null | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1 | cut -d_ -f2)
GLIBC=${GLIBC:-2.35}
cat > "$PKG/DEBIAN/control" <<EOF
Package: $NAME
Version: $VERSION
Section: net
Priority: optional
Architecture: $ARCH
Installed-Size: $SIZE_KB
Depends: libc6 (>= $GLIBC), libgcc-s1
Recommends: libgl1, libegl1, libxkbcommon0, libwayland-client0, libx11-6, libxcursor1, libxrandr2, libxi6, xdg-utils, xdg-desktop-portal
Maintainer: Swift Download Manager <noreply@example.com>
Description: Fast download manager with split downloads and resume
 Splits each download into several connections, pauses and resumes,
 keeps queues with a scheduler, grabs links from web pages, checks
 SHA-256 checksums and works with a browser add-on (included in
 /usr/share/$NAME/browser-extension).
EOF
cat > "$PKG/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q /usr/share/applications || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t /usr/share/icons/hicolor || true
EOF
chmod 755 "$PKG/DEBIAN/postinst"

dpkg-deb --root-owner-group --build "$PKG" "dist/${NAME}_${VERSION}_${ARCH}.deb"
echo "Made dist/${NAME}_${VERSION}_${ARCH}.deb"
