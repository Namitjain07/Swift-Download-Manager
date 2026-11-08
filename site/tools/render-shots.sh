#!/usr/bin/env bash
# Has the app draw its own window in every theme, for the website (no screen capture: nothing of the
# desktop or of anybody's files can end up in a picture).
#
#   bash site/tools/render-shots.sh <folder> [path-to-the-app]
#
# Writes <folder>/themes/<id>-<light|dark>.png, <folder>/sync/<name>.png and the frames of the two tours
# (<folder>/tour/NN.png, <folder>/onboarding/NN.png), each as sharp as the screen allows (see render-docs.sh:
# SDM_SCREEN=3840x2160 doubles it on a 4K screen). Then: bash site/tools/encode-media.sh <folder>
set -euo pipefail

OUT="${1:?folder to write the screenshots to}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
APP="${2:-$ROOT/source/target/release/swift_download_manager}"
[[ -x "$APP" || -x "$APP.exe" ]] || { echo "build the app first (cargo build --release) or pass its path" >&2; exit 1; }

mkdir -p "$OUT/themes" "$OUT/sync" "$OUT/tour" "$OUT/onboarding"
WALL="$ROOT/docs/images/wallpaper-bloom.png"
SCREEN="${SDM_SCREEN:-1920x1080}"
if command -v cygpath >/dev/null 2>&1; then WALL="$(cygpath -m "$WALL")"; OUT="$(cygpath -m "$OUT")"; fi

# draw <file> <WxH> <app options ...>: as large as the screen allows
draw() {
  local file="$1" size="$2"; shift 2
  local scale
  scale="$(awk -v s="$size" -v sc="$SCREEN" 'BEGIN { split(s, a, "x"); split(sc, b, "x"); x = b[1] / a[1]; y = b[2] / a[2]; r = (x < y ? x : y); r = int(r * 100) / 100; if (r < 1) r = 1; print r }')"
  "$APP" --screenshot "$file" --demo --size "$size" --scale "$scale" "$@"
}

for t in coral winui files carbon blue green black purple pink orange teal gold nord dracula mocha; do
  for m in light dark; do
    draw "$OUT/themes/$t-$m.png" 1180x720 --color-theme "$t" --theme "$m" --wallpaper "$WALL"
  done
done

# The WinUI 3 theme over different pictures (it takes its tint from the picture, like Mica does) and materials.
# Pass SDM_WALLPAPERS=<folder> with sunset.png, aurora.png, graphite.png and dawn.png (see make-wallpapers.ps1).
W="${SDM_WALLPAPERS:-}"
sync() { draw "$OUT/sync/$1.png" 1180x720 --color-theme winui --theme "$2" --backdrop "$3" ${4:+--wallpaper "$4"}; }
sync dark-bloom dark mica "$WALL"
sync dark-solid dark none
if [[ -n "$W" ]]; then
  sync dark-sunset dark mica "$W/sunset.png"
  sync dark-aurora dark mica "$W/aurora.png"
  sync dark-graphite dark mica "$W/graphite.png"
  sync dark-acrylic dark acrylic "$W/sunset.png"
  sync light-dawn light mica "$W/dawn.png"
fi

# A short tour of the app, and the welcome window step by step.
draw "$OUT/tour/01.png" 1180x760 --color-theme coral --theme light
draw "$OUT/tour/02.png" 1180x760 --color-theme winui --theme dark --backdrop mica --wallpaper "$WALL"
draw "$OUT/tour/03.png" 1180x760 --color-theme coral --theme light --show progress
draw "$OUT/tour/04.png" 1180x760 --color-theme coral --theme dark --show progress-mini
draw "$OUT/tour/05.png" 1180x760 --color-theme coral --theme light --show updates
draw "$OUT/tour/06.png" 1180x760 --color-theme carbon --theme dark --show networks
draw "$OUT/tour/07.png" 1180x760 --color-theme coral --theme light --show ai
draw "$OUT/tour/08.png" 1180x760 --color-theme nord --theme dark --show table
n=0
for step in onboarding onboarding-look onboarding-setup onboarding-browser onboarding-updates onboarding-tips; do
  n=$((n + 1))
  draw "$OUT/onboarding/0$n.png" 1180x800 --color-theme coral --theme light --show "$step"
done
echo "Done: $OUT"
