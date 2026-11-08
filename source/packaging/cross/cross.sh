#!/usr/bin/env bash
# Builds Swift Download Manager for many platforms in a Docker container (Linux, Windows, BSD; Intel/AMD, ARM, RISC-V, ...).
# Works from Linux, macOS, and Git Bash on Windows; all it needs is Docker. Run it from the repository (or `make cross`):
#
#   bash packaging/cross/cross.sh list                 the platforms
#   bash packaging/cross/cross.sh build                build all of them into out/ (in the repository)
#   bash packaging/cross/cross.sh build linux-aarch64 'windows-*'     only some (names or patterns, see "list")
#   bash packaging/cross/cross.sh test                 try the Linux programs on their own CPU (emulated), see below
#   bash packaging/cross/cross.sh clean                remove the cache volume (crates, compiled code) of the builds
#
# "build" makes the image the first time (a few minutes; the Rust standard library of every platform is in it), unpacks the
# source into the container (nothing on your disk is mounted, so it is quick on OneDrive and network drives as well), builds,
# and copies the archives and BUILD-REPORT.txt to out/. A cache volume keeps the compiled code for the next run.
# "test" starts every Linux program in a container of its own CPU (Docker emulates it with QEMU): the network list, and the
# server with its health check.
set -euo pipefail
# Git Bash on Windows would turn /paths given to docker.exe into Windows paths.
export MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*'

cd "$(dirname "$0")/../.."
TARGETS=packaging/cross/targets.txt
IMAGE=${SDM_CROSS_IMAGE:-swift-download-manager-cross}
CACHE=${SDM_CROSS_CACHE:-swift-download-manager-cross-cache}
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
# Where the results go: out/ in the repository (next to source/), so that they are easy to find and to try.
OUT_DIR=${SDM_OUT_DIR:-../out}

die() { echo "cross.sh: $*" >&2; exit 1; }

need_docker() {
  command -v docker > /dev/null 2>&1 || die "Docker is needed (https://docs.docker.com/get-docker/)."
  docker info > /dev/null 2>&1 || die "Docker is installed but not running. Start Docker Desktop (or the docker service) and try again."
}

# The platforms whose name matches one of the patterns given (all when there are none), as the lines of targets.txt.
platform_rows() {
  local name rest pattern
  while read -r name rest; do
    [[ -z "$name" || "$name" == \#* ]] && continue
    if [[ $# -eq 0 ]]; then echo "$name $rest"; continue; fi
    for pattern in "$@"; do
      # shellcheck disable=SC2053 # the pattern is meant to be a glob
      if [[ "$pattern" == all || "$name" == $pattern ]]; then echo "$name $rest"; break; fi
    done
  done < "$TARGETS"
}

cmd_list() {
  printf '%-24s %-34s %-7s %s\n' PLATFORM "RUST TARGET" KIND "TESTED WITH (cross.sh test)"
  local name target glibc kind says platform image
  platform_rows "$@" | while read -r name target glibc kind says platform image; do
    printf '%-24s %-34s %-7s %s\n' "$name" "$target" "$kind" "$([[ "$platform" == "-" ]] && echo "-" || echo "$platform $image")"
  done
}

cmd_build() {
  need_docker
  docker build -t "$IMAGE" -f packaging/cross/Dockerfile packaging/cross
  docker volume create "$CACHE" > /dev/null
  local container="sdm-cross-$$" epoch status=0
  epoch=$(git -C .. log -1 --format=%ct 2> /dev/null || date +%s)
  # The container is kept until its /out has been copied, and removed in any case.
  trap "docker rm -f '$container' > /dev/null 2>&1 || true" EXIT
  # The working copy (without what was built) goes in as a tar stream, so no folder of yours is mounted.
  tar -c --exclude=source/target --exclude=source/dist -C .. source LICENSE CHANGELOG.md \
    | docker run -i --name "$container" -v "$CACHE:/cache" -e "SOURCE_DATE_EPOCH=$epoch" "$IMAGE" --stdin "$@" || status=$?
  # What this run built replaces the earlier files of those platforms; the files of the other platforms stay.
  local fresh="$OUT_DIR/.new" name
  rm -rf "$fresh"
  mkdir -p "$fresh" "$OUT_DIR"
  docker cp "$container:/out/." "$fresh" > /dev/null 2>&1 || die "The build produced nothing to copy (see the messages above)."
  while read -r name; do
    rm -f "$OUT_DIR/swift-download-manager-$VERSION-$name.tar.gz" "$OUT_DIR/swift-download-manager-$VERSION-$name.zip" \
      "$OUT_DIR/report/$name.txt" "$OUT_DIR/logs/$name.log"
  done < "$fresh/selected.txt"
  rm -f "$fresh/selected.txt"
  cp -R "$fresh/." "$OUT_DIR/"
  rm -rf "$fresh"
  write_report
  echo
  ls -lh "$OUT_DIR" | sed 's/^/  /'
  echo
  echo "In $OUT_DIR. The details of every platform are in BUILD-REPORT.txt there, the build logs in the logs folder."
  return "$status"
}

# BUILD-REPORT.txt: what is known about every platform that was ever built into the output folder (one file each in its report folder).
write_report() {
  local file name built=() failed=()
  for file in "$OUT_DIR"/report/[!_]*.txt; do
    [[ -e "$file" ]] || continue
    name=$(basename "$file" .txt)
    if grep -q '^  FAILED' "$file"; then failed+=("$name"); else built+=("$name"); fi
  done
  {
    cat "$OUT_DIR/report/_header.txt"
    echo
    echo "built (${#built[@]}): ${built[*]:-none}"
    echo "failed (${#failed[@]}): ${failed[*]:-none}"
    echo
    for file in "$OUT_DIR"/report/[!_]*.txt; do
      [[ -e "$file" ]] && cat "$file"
    done
  } > "$OUT_DIR/BUILD-REPORT.txt"
}

# Starts one Linux program in a container of its own CPU and checks the network list and the server.
test_one() {
  local name=$1 platform=$2 image=$3 archive=$OUT_DIR/swift-download-manager-$VERSION-$1.tar.gz
  [[ -f "$archive" ]] || { echo "-- $name: not built, skipped"; return 2; }
  echo "==> $name on $platform ($image)"
  # The program is unpacked inside the container from standard input, so no folder is mounted.
  # shellcheck disable=SC2016 # the script is for the container's shell
  docker run --rm -i --platform "$platform" "$image" sh -c '
    set -eu
    mkdir /t && tar -xzf - -C /t && cd /t/*/
    echo "machine: $(uname -m)"
    ./swift-download-manager --networks | grep -Eq "^Name +Kind" && echo "networks: ok"
    ./swift-download-manager --write-icon /tmp/icon.png 64 && test -s /tmp/icon.png && echo "icon: ok"
    export SDM_DATA=/tmp/sdm SDM_DOWNLOADS=/tmp/sdm/dl SDM_BIND=127.0.0.1 SDM_PORT=18080 SDM_ADMIN_USER=admin SDM_ADMIN_PASSWORD=a-password-for-this-check-only
    ./swift-download-manager --server > /tmp/server.log 2>&1 &
    server=$!
    up=no
    for _ in $(seq 1 90); do
      if ./swift-download-manager --healthcheck > /dev/null 2>&1; then up=yes; break; fi
      kill -0 "$server" 2> /dev/null || break
      sleep 1
    done
    kill "$server" 2> /dev/null || true
    if [ "$up" != yes ]; then echo "server: NOT answering"; cat /tmp/server.log; exit 1; fi
    echo "server: answers"
  ' < "$archive"
}

cmd_test() {
  need_docker
  [[ -d "$OUT_DIR" ]] || die "Nothing is built yet: bash packaging/cross/cross.sh build"
  local name target glibc kind says platform image ok=() bad=() skipped=() status
  while read -r name target glibc kind says platform image; do
    if [[ "$platform" == "-" ]]; then skipped+=("$name"); continue; fi
    if test_one "$name" "$platform" "$image"; then ok+=("$name"); else
      status=$?
      if [[ "$status" -eq 2 ]]; then skipped+=("$name"); else bad+=("$name"); fi
    fi
  done < <(platform_rows "$@")
  echo
  echo "tried and working (${#ok[@]}): ${ok[*]:-none}"
  echo "failed (${#bad[@]}): ${bad[*]:-none}"
  echo "not tried here (${#skipped[@]}): ${skipped[*]:-none}"
  [[ ${#bad[@]} -eq 0 ]]
}

cmd_clean() {
  need_docker
  docker volume rm "$CACHE" 2> /dev/null && echo "Removed the cache volume $CACHE." || echo "There is no cache volume $CACHE."
}

command=${1:-build}
case "$command" in
  list | build | test | clean) shift ;;
  -h | --help | help) sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
  *) command=build ;; # "cross.sh linux-aarch64" builds that platform
esac
"cmd_$command" "$@"
