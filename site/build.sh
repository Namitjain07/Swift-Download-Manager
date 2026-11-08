#!/usr/bin/env bash
# Assembles the website into one folder that GitHub Pages can serve.
#
#   bash site/build.sh [out-dir]        # default: _site in the repository root
#
# - copies the page, its styles, script and pictures
# - makes the live previews: the page of the web interface (source/src/web/ui/index.html) of the current code and
#   of every release tag that has one, each with the pretend server (site/demo/mock-api.js) added before its script
# - writes data/releases.json from the GitHub API when `gh` is available (otherwise the committed copy is used),
#   so the download and version lists show at once, even when the API is busy
# - writes sitemap.xml and robots.txt
#
# CI runs this (see .github/workflows/pages.yml). To look at the result: serve the folder with any web server,
# for example `docker run --rm -p 8080:80 -v "$PWD/_site:/usr/share/nginx/html:ro" nginx:alpine`.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
OUT="${1:-$ROOT/_site}"
REPO="${GITHUB_REPOSITORY:-Namitjain07/Swift-Download-Manager}"
BASE="${SITE_URL:-https://namitjain07.github.io/Swift-Download-Manager}"
UI="source/src/web/ui/index.html"

rm -rf "$OUT"
mkdir -p "$OUT/data" "$OUT/demo"
cp "$HERE/index.html" "$HERE/404.html" "$OUT/"
cp -r "$HERE/assets" "$HERE/report-bug" "$OUT/"
cp "$HERE/demo/mock-api.js" "$OUT/demo/mock-api.js"
touch "$OUT/.nojekyll"

# ---------------------------------------------------------------- the live previews
# $1: the page (a file), $2: the folder to put it in, $3: the way to the pretend server from there, $4: the label.
make_demo() {
  local src="$1" dest="$2" rel="$3" label="$4"
  mkdir -p "$dest"
  awk -v inj="<script>window.SDM_PREVIEW=\"$label\";</script><script src=\"${rel}mock-api.js\"></script>
" '!done && /<script>/ { sub(/<script>/, inj "<script>"); done = 1 } { print }' "$src" > "$dest/index.html"
  grep -q 'mock-api.js' "$dest/index.html" || { echo "no <script> in $src" >&2; return 1; }
}

make_demo "$ROOT/$UI" "$OUT/demo" "" "latest build"
TAGS=()
if git -C "$ROOT" rev-parse --git-dir >/dev/null 2>&1; then
  while IFS= read -r tag; do
    tmp="$(mktemp)"
    if git -C "$ROOT" show "$tag:$UI" > "$tmp" 2>/dev/null && [ -s "$tmp" ]; then
      make_demo "$tmp" "$OUT/demo/$tag" "../" "$tag" && TAGS+=("$tag")
    fi
    rm -f "$tmp"
  done < <(git -C "$ROOT" tag -l 'v*' | sort -V)
fi
{
  printf '{"tags":['
  sep=""; for t in "${TAGS[@]+"${TAGS[@]}"}"; do printf '%s"%s"' "$sep" "$t"; sep=","; done
  printf ']}\n'
} > "$OUT/demo/versions.json"
echo "previews: latest build + ${#TAGS[@]} release(s) ${TAGS[*]+"(${TAGS[*]})"}"

# ---------------------------------------------------------------- the list of releases
FRESH=""
if command -v gh >/dev/null 2>&1; then
  if gh api "repos/$REPO/releases?per_page=100" --jq '[.[] | select(.draft | not) | {tag_name, name, published_at, prerelease, body, html_url,
      assets: [.assets[] | {name, size, download_count, browser_download_url}]}]' > "$OUT/data/releases.json.new" 2>/dev/null \
     && [ -s "$OUT/data/releases.json.new" ] && grep -q '^\[' "$OUT/data/releases.json.new"; then
    mv "$OUT/data/releases.json.new" "$OUT/data/releases.json"; FRESH=1
  fi
fi
rm -f "$OUT/data/releases.json.new"
if [ -z "$FRESH" ]; then
  cp "$HERE/data/releases.json" "$OUT/data/releases.json"
  echo "releases: the committed copy (no GitHub API)"
else
  echo "releases: $(grep -o '"tag_name"' "$OUT/data/releases.json" | wc -l) from the GitHub API"
fi

# ---------------------------------------------------------------- for search engines
cat > "$OUT/robots.txt" <<EOF
User-agent: *
Allow: /
Sitemap: $BASE/sitemap.xml
EOF
cat > "$OUT/sitemap.xml" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>$BASE/</loc></url>
  <url><loc>$BASE/report-bug/</loc></url>
</urlset>
EOF

echo "Built $OUT ($(find "$OUT" -type f | wc -l) files, $(du -sk "$OUT" | cut -f1) KB)"
