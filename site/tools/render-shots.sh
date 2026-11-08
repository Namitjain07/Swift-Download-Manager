#!/usr/bin/env bash
# Has the app draw its own window in every theme, for the website (no screen capture: nothing of the
# desktop or of anybody's files can end up in a picture).
#
#   bash site/tools/render-shots.sh <folder> [path-to-the-app]
#
# Writes <folder>/themes/<id>-<light|dark>.png and <folder>/sync/<name>.png.
set -euo pipefail

OUT="${1:?folder to write the screenshots to}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
APP="${2:-$ROOT/source/target/release/swift_download_manager}"
[ -x "$APP" ] || [ -x "$APP.exe" ] || { echo "build the app first (cargo build --release) or pass its path" >&2; exit 1; }

mkdir -p "$OUT/themes" "$OUT/sync"
WALL="$ROOT/docs/images/wallpaper-bloom.png"
SIZE="1180x720"

for t in coral winui files carbon blue green black purple pink orange teal gold nord dracula mocha; do
  for m in light dark; do
    "$APP" --screenshot "$OUT/themes/$t-$m.png" --demo --color-theme "$t" --theme "$m" --wallpaper "$WALL" --size "$SIZE"
  done
done

# The WinUI 3 theme over different pictures (it takes its tint from the picture, like Mica does) and materials.
# Pass SDM_WALLPAPERS=<folder> with sunset.png, aurora.png, graphite.png and dawn.png to use your own.
W="${SDM_WALLPAPERS:-}"
shot() { "$APP" --screenshot "$OUT/sync/$1.png" --demo --color-theme winui --theme "$2" --backdrop "$3" ${4:+--wallpaper "$4"} --size "$SIZE"; }
shot dark-bloom dark mica "$WALL"
shot dark-solid dark none
if [ -n "$W" ]; then
  shot dark-sunset dark mica "$W/sunset.png"
  shot dark-aurora dark mica "$W/aurora.png"
  shot dark-graphite dark mica "$W/graphite.png"
  shot dark-acrylic dark acrylic "$W/sunset.png"
  shot light-dawn light mica "$W/dawn.png"
fi
echo "Done: $OUT"
