#!/usr/bin/env bash
# Builds Swift Download Manager for the platforms in targets.txt. It runs inside the cross-compile image
# (packaging/cross/Dockerfile) and is started by cross.sh; it is not meant to be run by hand on a computer.
#
#   sdm-build-cross [--stdin] [name-or-pattern ...]
#
#   --stdin   first unpack the source (a tar stream: source/, LICENSE, CHANGELOG.md) from standard input into /work
#   names     platforms from targets.txt, or patterns such as "linux-*", "windows-*" or "linux-musl-*"; "all" (the default)
#
# Every platform ends up as one archive in /out (swift-download-manager-<version>-<name>.tar.gz, or .zip for Windows), with a
# log in /out/logs and a report in /out/report/<name>.txt (cross.sh puts the reports together into BUILD-REPORT.txt). A
# platform that does not build does not stop the others; the exit status says whether everything asked for was built and
# passed the checks.
set -uo pipefail

TARGETS=/usr/local/share/sdm/targets.txt
WORK=/work
OUT=/out
: "${SOURCE_DATE_EPOCH:=$(date +%s)}"

if [[ "${1:-}" == "--stdin" ]]; then
  shift
  mkdir -p "$WORK"
  tar -x -C "$WORK" || { echo "Could not unpack the source from standard input." >&2; exit 2; }
fi
[[ -f "$WORK/source/Cargo.toml" ]] || { echo "No source in $WORK/source (give it with --stdin)." >&2; exit 2; }
cd "$WORK/source" || exit 2

VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
patterns=("$@")
[[ ${#patterns[@]} -gt 0 ]] || patterns=(all)

# Does the name match one of the patterns asked for?
selected() {
  local pattern
  for pattern in "${patterns[@]}"; do
    # shellcheck disable=SC2053 # the pattern is meant to be a glob
    [[ "$pattern" == all || "$1" == $pattern ]] && return 0
  done
  return 1
}

# The platforms to build: the lines of targets.txt without comments, whose name was asked for.
rows=()
known=()
while read -r name target glibc kind says rest; do
  [[ -z "$name" || "$name" == \#* ]] && continue
  known+=("$name")
  if selected "$name"; then rows+=("$name $target $glibc $kind $says"); fi
done < "$TARGETS"
if [[ ${#rows[@]} -eq 0 ]]; then
  echo "Nothing to build: no platform matches \"${patterns[*]}\". The platforms are:" >&2
  printf '  %s\n' "${known[@]}" >&2
  exit 2
fi

rm -rf "$OUT"
mkdir -p "$OUT/logs" "$OUT/stage" "$OUT/report"
echo "Swift Download Manager $VERSION: ${#rows[@]} platform(s), zig $(zig version), $(rustc --version)"
# For cross.sh: what was built in this run, and with what.
for row in "${rows[@]}"; do echo "${row%% *}"; done > "$OUT/selected.txt"
echo "Swift Download Manager $VERSION, built with zig $(zig version) and $(rustc --version)" > "$OUT/report/_header.txt"

built=()
failed=()

# The notes that go next to a program that has no window.
server_notes() {
  cat <<'EOF'
This program has no window. It is a static build for servers and for computers without a desktop:

  ./swift-download-manager --server       the web interface and the download server (see README.md)
  ./swift-download-manager --networks     list the networks it can download over
  ./swift-download-manager --mcp          the server for AI assistants (over standard input and output)

For the program with the window, use the "linux-<cpu>" archive for your CPU (it needs glibc and the system's graphics libraries).
EOF
}

# A platform that did not work: say why on the screen and in its report.
fail() {
  local name=$1
  shift
  echo "    FAILED $*"
  printf '%s\n  FAILED  : %s\n  log     : logs/%s.log\n' "$name" "$*" "$name" > "$OUT/report/$name.txt"
  return 1
}

build_one() {
  local name=$1 target=$2 glibc=$3 kind=$4 says=${5//_/ }
  local zig_target=$target exe=swift_download_manager shipped=swift-download-manager started=$SECONDS
  [[ "$glibc" != "-" ]] && zig_target="$target.$glibc"
  if [[ "$target" == *-windows-* ]]; then
    exe+=.exe
    shipped=SwiftDownloadManager.exe
  fi
  local log="$OUT/logs/$name.log"
  echo "==> $name ($zig_target)"

  # Cargo keeps what build.rs said last time, including "could not embed the Windows resources", and does not notice that a tool
  # it used has changed since. For Windows the program itself is therefore built afresh (its dependencies stay in the cache).
  [[ "$target" == *-windows-* ]] && cargo clean --release --target "$target" -p swift_download_manager > /dev/null 2>&1
  # Windows on ARM needs its own windres (see windres-aarch64.sh); the others use the MinGW one that build.rs finds by itself.
  local -a build_env=()
  [[ "$target" == aarch64-pc-windows-* ]] && build_env=(WINDRES=/usr/local/bin/windres-aarch64)
  if ! env "${build_env[@]}" cargo zigbuild --release --locked --target "$zig_target" > "$log" 2>&1; then
    echo "    the end of the log:"
    tail -n 12 "$log" | sed 's/^/    | /'
    fail "$name" "the build did not finish"
    return 1
  fi
  local bin="$CARGO_TARGET_DIR/$target/release/$exe"
  [[ -f "$bin" ]] || { fail "$name" "the build said it was done but $bin is not there"; return 1; }

  # Is it a program for this platform, linked the way it should be?
  local what notes=""
  what=$(file -b "$bin")
  if [[ "$what" != *"$says"* ]]; then
    fail "$name" "wanted \"$says\" in: $what"
    return 1
  fi
  if [[ "$target" == *-linux-musl* && "$what" != *"static"* ]]; then
    fail "$name" "a musl build must be static, but: $what"
    return 1
  fi
  if [[ "$target" == *-windows-* ]] && grep -q 'could not embed the Windows resources' "$log"; then
    fail "$name" "built, but without the icon and the version information"
    return 1
  fi
  if [[ "$glibc" != "-" ]]; then
    local newest needs
    newest=$(readelf --dyn-syms -W "$bin" | grep -o 'GLIBC_[0-9][0-9.]*' | sed 's/^GLIBC_//' | sort -Vu | tail -n 1)
    needs=$(readelf -d "$bin" | grep -o 'Shared library: \[[^]]*\]' | sed 's/.*\[\(.*\)\]/\1/' | paste -sd' ' -)
    if [[ -n "$newest" && "$(printf '%s\n%s\n' "$newest" "$glibc" | sort -V | tail -n 1)" != "$glibc" ]]; then
      fail "$name" "it needs glibc $newest, more than the $glibc it was linked for"
      return 1
    fi
    notes="glibc ${newest:-?} (linked for $glibc), needs: $needs"
  elif [[ "$target" == *-linux-musl* ]]; then
    notes="static"
  fi

  # The archive: the program, the browser add-on, the licence, the readme, and the menu entry on Linux.
  local pkg="swift-download-manager-$VERSION-$name" stage archive
  stage="$OUT/stage/$pkg"
  mkdir -p "$stage"
  install -m 755 "$bin" "$stage/$shipped"
  cp -r browser-extension "$stage/"
  cp README.md ../LICENSE "$stage/"
  [[ "$target" == *-linux-gnu* ]] && cp packaging/linux/swift-download-manager.desktop "$stage/"
  [[ "$kind" == server ]] && server_notes > "$stage/SERVER-ONLY.txt"
  chmod -R u=rwX,go=rX "$stage"
  find "$stage" -exec touch -h -d "@$SOURCE_DATE_EPOCH" {} +
  if [[ "$target" == *-windows-* ]]; then
    archive="$pkg.zip"
    (cd "$OUT/stage" && zip -X -q -r "$OUT/$archive" "$pkg")
  else
    archive="$pkg.tar.gz"
    (cd "$OUT/stage" && tar --sort=name --owner=0 --group=0 --numeric-owner -cf - "$pkg" | gzip -9n > "$OUT/$archive")
  fi
  rm -rf "$stage"

  local size
  size=$(du -h "$bin" | cut -f1)
  {
    echo "$name"
    echo "  archive : $archive"
    echo "  target  : $target, program $size"
    echo "  file    : $what"
    [[ -n "$notes" ]] && echo "  notes   : $notes"
  } > "$OUT/report/$name.txt"
  echo "    ok: $archive, program $size, ${notes:-no extra notes} ($((SECONDS - started)) s)"
}

for row in "${rows[@]}"; do
  read -r name target glibc kind says <<< "$row"
  if build_one "$name" "$target" "$glibc" "$kind" "$says"; then built+=("$name"); else failed+=("$name"); fi
done
rm -rf "$OUT/stage"

echo
echo "Built ${#built[@]} of ${#rows[@]}.${failed[*]:+ Failed: ${failed[*]}}"
[[ ${#failed[@]} -eq 0 ]]
