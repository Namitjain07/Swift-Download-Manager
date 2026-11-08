#!/usr/bin/env bash
# Turns the screenshots the app itself draws into the WebP files and GIFs of the website and the README.
#
#   bash site/tools/render-shots.sh <folder>      # 1. the app draws every theme and tour frame (needs a build of the app)
#   bash site/tools/encode-media.sh <folder>      # 2. this: compress and animate (needs Docker)
#
# <folder> holds themes/, sync/, tour/ and onboarding/ from the first step. The stills keep the size they were
# drawn at (up to 1800 wide); the animations are 1100 wide as GIFs and 1400 wide as animated WebP.
# Nothing here runs in CI: the finished pictures are committed under site/assets/img (and docs/images for the README).
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
  -v "$(w "$ROOT/docs/images"):/docs" \
  -v "$(w "$OUT"):/out" \
  "$IMAGE" -s <<'INNER'
set -eu
F="ffmpeg -hide_banner -loglevel error -y"
mkdir -p /out/themes /out/sync /out/product /tmp/frames

still() { # input output max-width quality
  $F -i "$1" -vf "scale='min($3,iw)':-2:flags=lanczos" -c:v libwebp -quality "$4" -compression_level 6 "$2"
}

# One WebP per theme and mode, for the gallery; and the WinUI 3 theme over each material and picture.
for f in /shots/themes/*.png; do still "$f" "/out/themes/$(basename "$f" .png).webp" 1800 80; done
for f in /shots/sync/*.png;   do still "$f" "/out/sync/$(basename "$f" .png).webp" 1800 82; done

# The pictures of the product (the README's too), no wider than a big screen's column needs.
for n in screenshot screenshot-dark add-download add-download-dark table-view scheduler-dark options-dark options-light networks-dark \
         ai-assistants download-flow responsive-medium responsive-compact winui-light winui-dark winui-options winui-options-dark \
         winui-appearance winui-appearance-dark updates-window progress-mini progress-window onboarding-welcome onboarding-setup \
         extension-popup themes-light themes-dark web-light web-dark web-mobile web-bulk web-networks web-tokens ai-assistants-controls; do
  [ -f "/docs/$n.png" ] || continue
  still "/docs/$n.png" "/out/product/$n.webp" 1800 84
done

# A GIF (for the README) and an animated WebP (for the site) from numbered frames in a folder.
animate() { # name framerate folder gif-width webp-width
  $F -framerate "$2" -i "$3/%02d.png" \
    -vf "scale=$4:-2:flags=lanczos,split[a][b];[a]palettegen=max_colors=200:stats_mode=full[p];[b][p]paletteuse=dither=bayer:bayer_scale=4" \
    -loop 0 "/out/$1.gif"
  $F -framerate "$2" -i "$3/%02d.png" -vf "scale=$5:-2:flags=lanczos" -c:v libwebp_anim -quality 74 -compression_level 6 -loop 0 "/out/$1.webp"
}

# Through the 15 themes, light and then dark.
i=0
for t in coral winui files carbon blue green black purple pink orange teal gold nord dracula mocha; do
  for m in light dark; do
    i=$((i + 1)); cp "/shots/themes/$t-$m.png" "/tmp/frames/$(printf %02d $i).png"
  done
done
animate theme-cycle 1.25 /tmp/frames 1100 1400

# The WinUI 3 theme taking its colours from different pictures and materials.
rm -f /tmp/frames/*
i=0
for n in dark-bloom dark-sunset dark-aurora dark-graphite dark-acrylic light-dawn dark-solid; do
  i=$((i + 1)); cp "/shots/sync/$n.png" "/tmp/frames/$(printf %02d $i).png"
done
animate winui-sync 0.9 /tmp/frames 1100 1400

# A tour of the app, and the welcome window.
[ -d /shots/tour ] && animate tour 0.7 /shots/tour 1100 1400
[ -d /shots/onboarding ] && animate onboarding 0.55 /shots/onboarding 1000 1200

ls -l /out | tail -n +2
INNER

# The README's animations are the same files.
cp "$OUT/theme-cycle.gif" "$OUT/winui-sync.gif" "$ROOT/docs/images/"
[[ -f "$OUT/tour.gif" ]] && cp "$OUT/tour.gif" "$OUT/onboarding.gif" "$ROOT/docs/images/"
echo "Done: $OUT"
