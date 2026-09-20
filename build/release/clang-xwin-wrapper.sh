#!/bin/bash
# cc-rs falls back to gcc-mode clang for .S (GNU assembly) files even on
# MSVC targets, but cargo-xwin's CFLAGS_<target> use clang-cl's MSVC-style
# `/imsvc <PATH>` pairs which gcc-mode clang doesn't understand. Translate
# those pairs to plain `-I <PATH>` so the same CFLAGS work in both modes.
#
# Installed as `/usr/local/bin/clang` so it shadows `/usr/bin/clang` on PATH
# (clang-cl is invoked via cargo-xwin's own symlink, which is untouched).
args=()
while [ $# -gt 0 ]; do
  case "$1" in
    /imsvc)
      shift
      args+=("-I" "$1")
      ;;
    *)
      args+=("$1")
      ;;
  esac
  shift
done
exec /usr/lib/llvm-14/bin/clang "${args[@]}"
