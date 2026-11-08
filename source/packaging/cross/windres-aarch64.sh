#!/bin/sh
# windres for Windows on ARM. build.rs (through the winresource crate) calls
#   <windres> -I<folder> <resource file> <object file>
# and for ARM it names no target, so LLVM's windres would make an Intel/AMD object that cannot be linked into an ARM program.
# build-cross.sh sets WINDRES to this script for the aarch64 Windows targets only.
exec "$(ls /usr/lib/llvm-*/bin/llvm-windres | sort -V | tail -n 1)" --target=aarch64-w64-mingw32 "$@"
