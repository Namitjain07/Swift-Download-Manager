#!/usr/bin/env bash
# Builds the Linux installer (.deb) for Ubuntu, Debian, Mint and similar, for the computer it runs on:
# on an Intel/AMD machine it makes the amd64 package, on a 64-bit ARM machine (Raspberry Pi 4/5, an ARM server
# or laptop) the arm64 package.
# Run from the app folder:  bash packaging/linux/build-deb.sh
# The result is written to dist/. Set SKIP_BUILD=1 to package a program that is already built
# (target/release/swift_download_manager).
set -euo pipefail
cd "$(dirname "$0")/../.."

VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
# A pre-release such as 1.5.0-beta.1 is written 1.5.0~beta.1 for dpkg, so it sorts before 1.5.0.
VERSION=${VERSION/-/\~}
# GitHub turns a tilde in a file name into a dot, so the file name keeps the dash.
FILE_VERSION=${VERSION/\~/-}
ARCH=${DEB_ARCH:-$(dpkg --print-architecture 2>/dev/null || echo amd64)}
NAME=swift-download-manager
PKG="dist/deb/${NAME}_${FILE_VERSION}_${ARCH}"

if [[ "${SKIP_BUILD:-}" != "1" ]]; then
  cargo build --release
fi

rm -rf "$PKG"
mkdir -p "$PKG/DEBIAN" "$PKG/usr/bin" "$PKG/usr/share/applications" \
         "$PKG/usr/share/$NAME/browser-extension" "$PKG/usr/share/doc/$NAME"
install -m 755 target/release/swift_download_manager "$PKG/usr/bin/$NAME"
install -m 644 packaging/linux/$NAME.desktop "$PKG/usr/share/applications/$NAME.desktop"
cp -r browser-extension/. "$PKG/usr/share/$NAME/browser-extension/"
chmod -R u=rwX,go=rX "$PKG/usr/share/$NAME/browser-extension"
install -m 644 README.md "$PKG/usr/share/doc/$NAME/README.md"
# The licence, in the form Debian looks for (the full text of the Apache licence is in base-files).
cat > "$PKG/usr/share/doc/$NAME/copyright" <<'EOF'
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: Swift Download Manager
Upstream-Contact: https://github.com/Namitjain07/Swift-Download-Manager/issues
Source: https://github.com/Namitjain07/Swift-Download-Manager

Files: *
Copyright: Swift Download Manager contributors
License: Apache-2.0

Files: usr/share/*/browser-extension/*
Copyright: Swift Download Manager contributors
License: Apache-2.0

License: Apache-2.0
 Licensed under the Apache License, Version 2.0 (the "License"); you may not use
 this software except in compliance with the License. You may obtain a copy of
 the License at http://www.apache.org/licenses/LICENSE-2.0
 .
 On Debian systems, the full text of the Apache License, Version 2.0 can be
 found in the file /usr/share/common-licenses/Apache-2.0.
EOF
chmod 644 "$PKG/usr/share/doc/$NAME/copyright"
if [[ -f ../CHANGELOG.md ]]; then
  gzip -9n -c ../CHANGELOG.md > "$PKG/usr/share/doc/$NAME/changelog.gz"
  chmod 644 "$PKG/usr/share/doc/$NAME/changelog.gz"
fi
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
Maintainer: Namitjain07 <119175433+Namitjain07@users.noreply.github.com>
Homepage: https://github.com/Namitjain07/Swift-Download-Manager
Description: Fast download manager with split downloads and resume
 Splits each download into several connections, pauses and resumes,
 keeps queues with a scheduler, grabs links from web pages, checks
 SHA-256 checksums and works with a browser add-on (included in
 /usr/share/$NAME/browser-extension). Can download over several
 networks at once (Wi-Fi, Ethernet, a phone with USB tethering) and
 run as a web-managed download server.
EOF
# A checksum for every installed file, so that "debsums" can check the installation.
(cd "$PKG" && find usr -type f -print0 | sort -z | xargs -0 md5sum > DEBIAN/md5sums)
cat > "$PKG/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q /usr/share/applications || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t /usr/share/icons/hicolor || true
EOF
chmod 755 "$PKG/DEBIAN/postinst"
cat > "$PKG/DEBIAN/postrm" <<'EOF'
#!/bin/sh
set -e
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q /usr/share/applications || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t /usr/share/icons/hicolor || true
EOF
chmod 755 "$PKG/DEBIAN/postrm"

dpkg-deb --root-owner-group --build "$PKG" "dist/${NAME}_${FILE_VERSION}_${ARCH}.deb"
echo "Made dist/${NAME}_${FILE_VERSION}_${ARCH}.deb"
