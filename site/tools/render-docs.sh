#!/usr/bin/env bash
# Retakes the pictures of the app for the README and the website at the sharpest size the screen allows.
#
#   bash site/tools/render-docs.sh [path-to-the-app]
#
# The app draws its own window with sample data (--demo) and saves it. A window cannot be bigger than the screen, so
# each picture is drawn with `--scale S`: everything S times larger in a window S times bigger, with S the largest
# that still fits the screen (1920x1080 unless SDM_SCREEN says otherwise, e.g. SDM_SCREEN=3840x2160 on a 4K screen,
# where the pictures come out twice as sharp). The layout is always the one of the plain size given below.
#
# Writes docs/images/*.png. The other pictures (the web interface, the browser pop-up, the grid of all themes) come
# from a browser and are made by other means.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
APP="${1:-$ROOT/source/target/release/swift_download_manager}"
[[ -x "$APP" || -x "$APP.exe" ]] || { echo "build the app first (cargo build --release) or pass its path" >&2; exit 1; }
OUT="$ROOT/docs/images"
SCREEN="${SDM_SCREEN:-1920x1080}"
SW="${SCREEN%x*}"
SH="${SCREEN#*x}"
WALL="$ROOT/docs/images/wallpaper-bloom.png"
if command -v cygpath >/dev/null 2>&1; then WALL="$(cygpath -m "$WALL")"; OUT="$(cygpath -m "$OUT")"; fi

# shot <file> <WxH> <app options ...>
shot() {
  local file="$1" size="$2"; shift 2
  local w="${size%x*}" h="${size#*x}" scale
  scale="$(awk -v w="$w" -v h="$h" -v sw="$SW" -v sh="$SH" 'BEGIN { a = sw / w; b = sh / h; s = (a < b ? a : b); s = int(s * 100) / 100; if (s < 1) s = 1; print s }')"
  "$APP" --screenshot "$OUT/$file.png" --demo --size "$size" --scale "$scale" "$@"
  echo "$file.png  ${size}  x$scale"
}

# the main window
shot screenshot       1180x680 --theme light
shot screenshot-dark  1180x680 --theme dark
shot table-view       1180x680 --theme light --show table
shot responsive-medium 900x640 --theme light
shot responsive-compact 600x760 --theme light
shot theme-files-light 1180x680 --theme light --color-theme files
shot theme-carbon-dark 1180x680 --theme dark --color-theme carbon

# windows over it
shot add-download        1180x680 --theme light --show add
shot add-download-dark   1180x680 --theme dark --show add
shot scheduler-dark      1180x800 --theme dark --show scheduler
shot options-light       1180x900 --theme light --show options
shot options-dark        1180x900 --theme dark --show options
shot networks-dark       1180x900 --theme dark --show networks
shot appearance-dark     1180x900 --theme dark --show appearance
shot download-flow       1180x800 --theme dark --show progress
shot ai-assistants       1180x800 --theme light --show ai
shot progress-window     1180x760 --theme light --show progress
shot progress-mini       1180x760 --theme dark --show progress-mini
shot updates-window      1180x760 --theme light --show updates
shot onboarding-welcome  1180x800 --theme light --show onboarding
shot onboarding-setup    1180x800 --theme light --show onboarding-setup

# WinUI 3, over the sample wallpaper
shot winui-light            1180x800 --theme light --color-theme winui --backdrop mica --wallpaper "$WALL"
shot winui-dark             1180x800 --theme dark --color-theme winui --backdrop mica --wallpaper "$WALL"
shot winui-options          1180x800 --theme light --color-theme winui --backdrop mica --wallpaper "$WALL" --show winui
shot winui-options-dark     1180x800 --theme dark --color-theme winui --backdrop mica --wallpaper "$WALL" --show winui
shot winui-appearance       1180x800 --theme light --color-theme winui --backdrop mica --wallpaper "$WALL" --show appearance
shot winui-appearance-dark  1180x800 --theme dark --color-theme winui --backdrop mica --wallpaper "$WALL" --show appearance
echo "Done: $OUT"
