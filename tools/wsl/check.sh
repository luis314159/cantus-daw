#!/bin/bash
# GitHub Actions' Linux checks (RUSTFLAGS=-D warnings), on this Windows machine through WSL
# Ubuntu 24.04, without sudo: the -dev packages the build scripts need come from
# ~/linux-deps/root (tools/wsl/fetch-deps.sh, once). The repository is cloned into
# ~/cantus-ci (a Linux file system, fast) and the given ref checked out.
#
#   wsl -d Ubuntu-24.04 -- bash -lc 'bash /mnt/f/projects/cantus/tools/wsl/check.sh <ref> [clippy|test]'
#
# <ref> is a branch or commit of the main repository (/mnt/f/projects/cantus; a worktree's
# .git points at Windows paths, so clone from the main one).
set -o pipefail
ref="${1:-master}"
what="${2:-clippy}"
src="${CANTUS_SRC:-/mnt/f/projects/cantus}"
root="$HOME/linux-deps/root"
export PATH="$HOME/.cargo/bin:$root/usr/bin:$PATH"
export LD_LIBRARY_PATH="$root/usr/lib/x86_64-linux-gnu"
export PKG_CONFIG_PATH="$root/usr/lib/x86_64-linux-gnu/pkgconfig:$root/usr/share/pkgconfig"
export PKG_CONFIG_SYSROOT_DIR="$root"
export OPENSSL_DIR="$root/usr"
# bindgen (pipewire-sys, libspa-sys: the PipeWire host, ADR 0065) loads libclang from here.
export LIBCLANG_PATH="$root/usr/lib/llvm-18/lib"
export OPENSSL_LIB_DIR="$root/usr/lib/x86_64-linux-gnu"
export OPENSSL_INCLUDE_DIR="$root/usr/include"
export RUSTFLAGS="-D warnings"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"
# As ci.yml: no debug info, no incremental cache. With them the target folder reached
# 126 GB inside the WSL disk image, which lives on C: and filled it (2026-10-14).
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_TEST_DEBUG=0
export CARGO_INCREMENTAL=0
if [ ! -d "$HOME/cantus-ci/.git" ]; then
  git clone -q "$src" "$HOME/cantus-ci" || exit 1
fi
cd "$HOME/cantus-ci" || exit 1
git fetch -q "$src" "$ref" && git checkout -q --force FETCH_HEAD || exit 1
git log --oneline -1
if [ "$what" = "test" ]; then
  cargo test --workspace --no-fail-fast 2>&1 | grep -E "^test result|panicked|^error|FAILED" | tail -60
  echo "TEST EXIT ${PIPESTATUS[0]}"
else
  cargo clippy --workspace --all-targets --keep-going 2>&1 | grep -E "^(error|warning)|-->" | head -120
  echo "CLIPPY EXIT ${PIPESTATUS[0]}"
  # xtask (perf, null hashes, packaging) builds the plug-ins without their editor.
  cargo clippy -p cantus-plugins --no-default-features --all-targets 2>&1 | grep -E "^(error|warning)|-->" | head -40
  echo "PLUGINS WITHOUT EDITOR EXIT ${PIPESTATUS[0]}"
fi
