#!/usr/bin/env bash
# Builds the Windows version from Linux (needs: rustup target add x86_64-pc-windows-gnu
# and the mingw-w64 package). On Windows itself, just run:  cargo build --release
# Run from the app folder:  bash packaging/windows/build-windows.sh
# The result (a zip with the .exe, the browser add-on and the read-me) goes to dist/.
set -euo pipefail
cd "$(dirname "$0")/../.."

VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
NAME=SwiftDownloadManager
OUT="dist/windows/${NAME}-${VERSION}-windows-x64"

cargo build --release --target x86_64-pc-windows-gnu

rm -rf "$OUT"
mkdir -p "$OUT/browser-extension"
cp target/x86_64-pc-windows-gnu/release/swift_download_manager.exe "$OUT/${NAME}.exe"
cp -r browser-extension/. "$OUT/browser-extension/"
cp README.md "$OUT/README.md"
cat > "$OUT/START HERE.txt" <<'EOF'
Swift Download Manager for Windows

1. Double-click SwiftDownloadManager.exe to start it (no installing needed).
   Windows may say it doesn't know this app: click "More info", then "Run anyway".
2. To catch downloads from your browser, see browser-extension\README.txt.
EOF
(cd dist/windows && rm -f "${NAME}-${VERSION}-windows-x64.zip" && python3 -c "
import os, sys, zipfile
root = sys.argv[1]
with zipfile.ZipFile(root + '.zip', 'w', zipfile.ZIP_DEFLATED) as z:
    for d, _, files in os.walk(root):
        for f in files:
            p = os.path.join(d, f)
            z.write(p, p)
" "${NAME}-${VERSION}-windows-x64")
echo "Made dist/windows/${NAME}-${VERSION}-windows-x64.zip"
