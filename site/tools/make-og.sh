#!/usr/bin/env bash
# Renders tools/og.html into assets/img/og.png (the picture that shows when the link is shared).
# Needs Chrome, Chromium or Edge. Usage: bash site/tools/make-og.sh [browser]
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
B="${1:-$(command -v google-chrome chromium chromium-browser chrome 2>/dev/null | head -1 || true)}"
[[ -n "$B" ]] || { for p in "/c/Program Files/Google/Chrome/Application/chrome.exe" "/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"; do [[ -x "$p" ]] && B="$p" && break; done; }
[[ -n "$B" ]] || { echo "no browser found: pass its path" >&2; exit 1; }
PROFILE="$(mktemp -d)"
URL="file://$HERE/og.html"; case "$(uname -s)" in MINGW*|MSYS*) URL="file:///$(cygpath -m "$HERE")/og.html";; esac
"$B" --headless=new --disable-gpu --hide-scrollbars --no-first-run --user-data-dir="$PROFILE" --window-size=1200,630 \
  --allow-file-access-from-files --virtual-time-budget=4000 --screenshot="$HERE/../assets/img/og.png" "$URL" >/dev/null 2>&1
echo "Wrote $HERE/../assets/img/og.png"
