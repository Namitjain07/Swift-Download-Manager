#!/usr/bin/env bash
# Turns the screenshots the app itself draws into the pictures, WebP files and GIFs of the website.
#
#   bash site/tools/render-shots.sh <folder>      # 1. the app draws every theme (needs a build of the app)
#   bash site/tools/encode-media.sh <folder>      # 2. this: resize, compress, animate (needs Docker)
#
# <folder> holds themes/<id>-<light|dark>.png and sync/<name>.png from the first step.
# Nothing here runs in CI: the finished pictures are committed under site/assets/img.
set -euo pipefail

SHOTS="${1:?folder with the screenshots (see render-shots.sh)}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SITE="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$SITE/.." && pwd)"
OUT="$SITE/assets/img"
mkdir -p "$OUT"

# Docker on Windows wants Windows paths for -v; elsewhere the paths are fine as they are.
if command -v cygpath >/dev/null 2>&1; then
  w() { cygpath -w "$1"; }
  export MSYS_NO_PATHCONV=1
else
  w() { printf '%s' "$1"; }
fi

IMAGE="${FFMPEG_IMAGE:-linuxserver/ffmpeg:latest}"

docker run --rm -i --entrypoint sh \
  -v "$(w "$SHOTS"):/shots:ro" \
  -v "$(w "$ROOT/docs/images"):/docs:ro" \
  -v "$(w "$OUT"):/out" \
  "$IMAGE" -s <<'INNER'
set -eu
F="ffmpeg -hide_banner -loglevel error -y"
mkdir -p /out/themes /out/sync /out/product /tmp/frames

# One WebP per theme and mode, for the gallery.
for f in /shots/themes/*.png; do
  n=$(basename "$f" .png)
  $F -i "$f" -vf "scale=1180:-2:flags=lanczos" -c:v libwebp -quality 80 -compression_level 6 "/out/themes/$n.webp"
done
# ... and the WinUI 3 theme over each material and picture.
for f in /shots/sync/*.png; do
  n=$(basename "$f" .png)
  $F -i "$f" -vf "scale=1180:-2:flags=lanczos" -c:v libwebp -quality 82 -compression_level 6 "/out/sync/$n.webp"
done

# The pictures of the product that were already in the repository (no wider than a retina column needs).
for n in ai-assistants ai-assistants-controls add-download add-download-dark scheduler-dark networks-dark options-dark options-light \
         table-view responsive-compact responsive-medium web-light web-dark web-mobile web-bulk web-networks web-tokens extension-popup \
         winui-appearance winui-appearance-dark winui-options-dark winui-options themes-light themes-dark screenshot screenshot-dark download-flow; do
  [ -f "/docs/$n.png" ] || continue
  $F -i "/docs/$n.png" -vf "scale='min(1400,iw)':-2:flags=lanczos" -c:v libwebp -quality 82 -compression_level 6 "/out/product/$n.webp"
done

# The GIF that goes through the 15 themes, light and then dark.
i=0
for t in coral winui files carbon blue green black purple pink orange teal gold nord dracula mocha; do
  for m in light dark; do
    i=$((i + 1))
    cp "/shots/themes/$t-$m.png" "/tmp/frames/$(printf %02d $i).png"
  done
done
$F -framerate 1.25 -i /tmp/frames/%02d.png \
  -vf "scale=760:-2:flags=lanczos,split[a][b];[a]palettegen=max_colors=160:stats_mode=full[p];[b][p]paletteuse=dither=bayer:bayer_scale=5" \
  -loop 0 /out/theme-cycle.gif
$F -framerate 1.25 -i /tmp/frames/%02d.png -vf "scale=960:-2:flags=lanczos" \
  -c:v libwebp_anim -quality 72 -compression_level 6 -loop 0 /out/theme-cycle.webp

# The GIF of the WinUI 3 theme taking its colours from different pictures and materials.
rm -f /tmp/frames/*
i=0
for n in dark-bloom dark-sunset dark-aurora dark-graphite dark-acrylic light-dawn dark-solid; do
  i=$((i + 1))
  cp "/shots/sync/$n.png" "/tmp/frames/$(printf %02d $i).png"
done
$F -framerate 0.9 -i /tmp/frames/%02d.png \
  -vf "scale=760:-2:flags=lanczos,split[a][b];[a]palettegen=max_colors=192:stats_mode=full[p];[b][p]paletteuse=dither=bayer:bayer_scale=5" \
  -loop 0 /out/winui-sync.gif
$F -framerate 0.9 -i /tmp/frames/%02d.png -vf "scale=960:-2:flags=lanczos" \
  -c:v libwebp_anim -quality 74 -compression_level 6 -loop 0 /out/winui-sync.webp

ls -l /out | tail -n +2
INNER

echo "Done: $OUT"
