# Building for many platforms with Docker

One Docker image builds Swift Download Manager for Linux, Windows and the BSDs, for Intel/AMD, ARM, RISC-V, POWER, IBM Z and
LoongArch, from any computer that runs Docker (Windows, macOS or Linux). Nothing else has to be installed: not Rust, not a C
compiler for each CPU, not MinGW.

```bash
bash packaging/cross/cross.sh list                        # the platforms
bash packaging/cross/cross.sh build                       # all of them, into out/ in the repository
bash packaging/cross/cross.sh build linux-aarch64 'windows-*'    # some of them (names or patterns)
bash packaging/cross/cross.sh test                        # start the Linux programs on their own CPU, see "Checks"
bash packaging/cross/cross.sh clean                       # throw away the cache of the builds
```

Run these in the `source` folder (on Windows in Git Bash). `make cross`, `make cross-list` and `make cross-test` in the main
folder do the same; `make cross CROSS='linux-musl-*'` builds a selection.

The first `build` makes the image, which takes a few minutes and about 8 GB: the Rust standard library of every platform
and the cross-compiler are in it. The compiled code and the downloaded crates are kept in a Docker volume, so the next run
only builds what changed. The working copy is sent into the container as a tar stream (no folder of yours is mounted), and
`out/` (in the main folder of the repository, next to `source/`) gets one archive per platform, a log for each, and `BUILD-REPORT.txt`. A build of some platforms replaces only those; the others stay. `SDM_OUT_DIR` puts the results somewhere else.

## What it builds

`cross.sh list` shows the current list; it is [`targets.txt`](targets.txt), one line per platform. The names are used in the
file names: `swift-download-manager-<version>-<name>.tar.gz`, and `.zip` for Windows.

| Platforms | Kind | What it is |
| --- | --- | --- |
| `linux-x86_64`, `linux-aarch64`, `linux-armv7`, `linux-armv6`, `linux-i686`, `linux-riscv64`, `linux-ppc64le`, `linux-s390x`, `linux-loongarch64` | glibc, with a window | Runs on any distribution with a glibc at least as new as the one it was linked against (2.17 for most, that is CentOS 7 and newer). The window needs the usual OpenGL and X11 or Wayland libraries of the system, which are loaded when the window opens |
| `linux-musl-x86_64`, `-aarch64`, `-armv7`, `-i686`, `-riscv64`, `-ppc64le`, `-loongarch64` | static, no window | One file that runs on any Linux of that CPU, Alpine and containers without a libc included. It has the web interface (`--server`), `--networks` and `--mcp`; a static program cannot load the graphics libraries, so there is no window |
| `windows-x86_64`, `windows-i686`, `windows-aarch64` | with a window | Built with MinGW, so there is no Visual C++ runtime to install. The icon, manifest and version information are in the program. **Not signed** (see below) |
| `freebsd-x86_64`, `freebsd-aarch64`, `netbsd-x86_64` | with a window | |

## How it works

* **Zig is the linker and the C compiler of every platform** (`cargo zigbuild`), so one compiler serves all CPUs and
  operating systems, and a Linux program is linked against an old glibc on purpose: the build checks that the program needs
  nothing newer than the version in `targets.txt`.
* The program loads OpenGL, X11, Wayland and fontconfig when it needs them, so none of those libraries has to exist for
  the target at build time. That is why a window can be cross-compiled at all.
* `build-cross.sh` runs inside the image. For every platform it builds with `--locked`, then checks the result (`file`
  names the right CPU and system, musl programs are static, glibc programs need nothing newer than planned, Windows programs
  have their icon and version information), packs the program with the browser add-on, the readme and the licence, and
  records what it found in `BUILD-REPORT.txt`. A platform that fails does not stop the others; the exit status tells.
* Archives are packed in a fixed order with fixed owners and time stamps (the time of the last commit), so building twice
  gives the same files.

## Checks

`cross.sh test` starts each Linux program in a container of its own CPU (Docker emulates ARM, RISC-V, POWER and IBM Z with
QEMU) and checks that it lists the networks, writes its icon, and that `--server` starts and answers its health check.
The Windows programs for Intel/AMD can be tried on Windows (`SwiftDownloadManager.exe --networks`). The BSD and the
LoongArch builds, and the Windows ARM build, are checked only for being the right kind of file, because there is nothing
to start them on here.

## What is not in here

* **macOS.** Linking for macOS needs Apple's SDK, which Apple's licence allows only on Apple hardware, so it cannot be put
  in an image. The macOS build is made on a Mac (`packaging/macos/build-mac.sh`) and by the `macos-latest` job on GitHub.
* **Windows with the Microsoft compiler (MSVC).** The installers and the programs in the releases are built on Windows
  with MSVC by the release workflow and signed there. The MinGW builds here work the same but are not signed, so Windows
  SmartScreen will show a warning for them.
* **Android and iOS.** The window needs a different application shell there.
* **Packages** (`.deb`, `.pkg.tar.zst`, the Windows installer). Those are built by their own scripts, on a system of the
  kind they are for (see the `Makefile`), because their tools are part of that system.

## Changing it

* **Add a platform:** one line in `targets.txt` (the Rust target must exist for the stable compiler; `rustup target list`),
  then `cross.sh build <name>`; the image is rebuilt with its standard library.
* **Newer tools:** the versions of Rust, Zig and `cargo-zigbuild` are `ARG`s at the top of the `Dockerfile`; Zig is checked
  against the checksum published on ziglang.org, so give the new one as well (`ZIG_SHA256_X86_64`, `ZIG_SHA256_AARCH64`).
* **A fresh start:** `cross.sh clean` removes the cache volume; `docker rmi swift-download-manager-cross` removes the image.
