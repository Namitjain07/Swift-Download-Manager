#!/usr/bin/env bash
# Builds the Arch Linux package (.pkg.tar.zst) for the computer it runs on: x86_64 on Arch Linux, aarch64 on
# Arch Linux ARM. Run it as an ordinary user (makepkg refuses to run as root) on a system with
# `base-devel`, `git` and `cargo` installed, from the app folder:
#   bash packaging/arch/build-arch.sh
# It packages the last commit (what `git archive` sees), so commit first. The result is in dist/arch/.
set -euo pipefail
cd "$(dirname "$0")/../.."

VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
# A package version has no dashes: 1.9.0-beta.1 becomes 1.9.0_beta.1.
PKGVER=${VERSION//-/_}
NAME=swift-download-manager
OUT="$PWD/dist/arch"

rm -rf "$OUT/build"
mkdir -p "$OUT/build"
git -C .. archive --format=tar.gz --prefix="$NAME-$PKGVER/" -o "$OUT/build/$NAME-$PKGVER.tar.gz" HEAD:source
sed "s/^pkgver=.*/pkgver=$PKGVER/" packaging/arch/PKGBUILD > "$OUT/build/PKGBUILD"

# --syncdeps installs the build dependencies (it asks for sudo only when something is missing).
# Arch Linux ARM makes .pkg.tar.xz by default; the same .pkg.tar.zst as on Arch is wanted everywhere.
(cd "$OUT/build" && PKGEXT=.pkg.tar.zst makepkg --force --noconfirm --clean --syncdeps)
cp "$OUT"/build/*.pkg.tar.zst "$OUT/"
rm -rf "$OUT/build"
ls -1 "$OUT"/*.pkg.tar.zst
