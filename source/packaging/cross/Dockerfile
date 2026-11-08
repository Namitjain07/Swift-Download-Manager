# syntax=docker/dockerfile:1
#
# A cross-compile toolchain for Swift Download Manager: one image that builds the program for Linux (glibc and static
# musl; Intel/AMD, ARM, RISC-V, POWER, IBM Z, LoongArch), Windows (Intel/AMD, 32-bit and ARM) and the BSDs.
#
# Zig is the C compiler and the linker for every target (`cargo zigbuild`), so no separate gcc is needed for each CPU, and a
# Linux program can be linked against an old glibc so that it runs on old distributions as well as new ones.
#
# Use it through packaging/cross/cross.sh (or `make cross` in the repository); nothing here is meant to be run by hand.
# macOS is not in the image: building for it needs Apple's SDK, which cannot be put in an image (see README.md).

ARG RUST_VERSION=1.99.1
FROM rust:${RUST_VERSION}-bookworm

ARG ZIG_VERSION=0.17.1
ARG ZIG_SHA256_X86_64=3581863e3cbcfd0fce0efc682ce7e9e93da96668a977791d356a26d2a7e9b17a
ARG ZIG_SHA256_AARCH64=a1546d5646eb184437918d6635ca6b762a0e889eac09cef720ee7a1ed3fe7795
ARG ZIGBUILD_VERSION=0.24.1
# Where Zig is fetched from after ziglang.org (see https://ziglang.org/download/community-mirrors.txt); change with --build-arg.
ARG ZIG_MIRRORS="https://zig.linus.dev/zig https://zig.squirl.dev"

# The MinGW binutils are for Windows: windres puts the icon and the version information into the .exe (build.rs), dlltool
# makes the import libraries. The rest is for packaging and for checking what was built.
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl xz-utils zip file binutils \
      binutils-mingw-w64-x86-64 binutils-mingw-w64-i686 \
 && rm -rf /var/lib/apt/lists/*

# Zig, from ziglang.org or, when that is slow or unreachable, from one of the community mirrors the Zig project lists. Every
# download is checked against the checksum published on ziglang.org, so a mirror cannot change what is installed. The image
# builds on Intel/AMD and on ARM machines.
RUN set -eux; \
    case "$(uname -m)" in \
      x86_64)  zig_arch=x86_64;  zig_sha="${ZIG_SHA256_X86_64}" ;; \
      aarch64) zig_arch=aarch64; zig_sha="${ZIG_SHA256_AARCH64}" ;; \
      *) echo "This image cannot be built on a $(uname -m) computer."; exit 1 ;; \
    esac; \
    zig_file="zig-${zig_arch}-linux-${ZIG_VERSION}.tar.xz"; \
    got=no; \
    for base in "https://ziglang.org/download/${ZIG_VERSION}" ${ZIG_MIRRORS}; do \
      echo "Trying ${base}"; \
      if curl -fsSL --connect-timeout 15 --speed-limit 50000 --speed-time 20 -o /tmp/zig.tar.xz "${base}/${zig_file}?source=swift-download-manager-cross" \
         && echo "${zig_sha}  /tmp/zig.tar.xz" | sha256sum -c -; then got=yes; break; fi; \
      echo "Not from ${base}"; \
    done; \
    [ "${got}" = yes ] || { echo "Could not get Zig ${ZIG_VERSION}."; exit 1; }; \
    mkdir -p /opt/zig; \
    tar -xJf /tmp/zig.tar.xz -C /opt/zig --strip-components=1; \
    rm /tmp/zig.tar.xz; \
    ln -s /opt/zig/zig /usr/local/bin/zig; \
    zig version

RUN cargo install --locked cargo-zigbuild --version "${ZIGBUILD_VERSION}" \
 && rm -rf /usr/local/cargo/registry

# The icon, the manifest and the version information of the Windows programs (build.rs) are compiled by windres, which runs the
# MinGW gcc as its preprocessor. For Windows on ARM the LLVM tools do the same job (see windres-aarch64.sh).
RUN apt-get update \
 && apt-get install -y --no-install-recommends gcc-mingw-w64-x86-64 gcc-mingw-w64-i686 llvm \
 && rm -rf /var/lib/apt/lists/*

# The standard library of every platform in targets.txt.
COPY targets.txt /usr/local/share/sdm/targets.txt
RUN grep -v '^[[:space:]]*#' /usr/local/share/sdm/targets.txt | awk 'NF { print $2 }' | xargs rustup target add

COPY build-cross.sh /usr/local/bin/sdm-build-cross
COPY windres-aarch64.sh /usr/local/bin/windres-aarch64
RUN chmod 755 /usr/local/bin/sdm-build-cross /usr/local/bin/windres-aarch64

# Everything that is worth keeping between runs (the downloaded crates, what was compiled, Zig's cache) is in /cache, a volume.
ENV CARGO_HOME=/cache/cargo \
    CARGO_TARGET_DIR=/cache/target \
    ZIG_GLOBAL_CACHE_DIR=/cache/zig \
    CARGO_TERM_COLOR=never \
    PATH=/usr/local/cargo/bin:/usr/local/bin:/usr/bin:/bin
WORKDIR /work
ENTRYPOINT ["/usr/local/bin/sdm-build-cross"]
