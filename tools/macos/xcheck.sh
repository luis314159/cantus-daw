#!/usr/bin/env bash
# Clippy for a Mac target from Windows, the whole workspace (ADR 0066, docs/DEVELOPING.md §4):
#   tools/macos/xcheck.sh aarch64-apple-darwin --workspace --all-targets
#   tools/macos/xcheck.sh x86_64-apple-darwin -p cantus-rt --all-targets
# Run from the repository's root in Git Bash, with LLVM installed (its llvm-ar) and Python.
# The C that ring, ogg and LAME compile is replaced by empty objects (fakecc.py), and
# ring's link step by nothing: clippy type-checks Rust and links nothing, so this checks
# every line of Rust a Mac would compile — and proves nothing about running it (the macOS
# runner does: macos.yml).
set -euo pipefail
t="${1:?usage: xcheck.sh <apple target> <clippy args…>}"
shift
u="${t//-/_}"
here="$(cd "$(dirname "$0")" && pwd -W 2>/dev/null || pwd)"
ar="${LLVM_AR:-C:\Program Files\LLVM\bin\llvm-ar.exe}"
ring="$(grep -A1 '^name = "ring"$' Cargo.lock | sed -n 's/^version = "\(.*\)"$/\1/p' | tr '.' '_')"
export "CC_$u=$here\fakecc.bat" "CXX_$u=$here\fakecc.bat" "AR_$u=$ar"
export RUSTFLAGS="${RUSTFLAGS:--D warnings}"
cargo clippy --target "$t" --config "target.$t.ring_core_${ring}_.rustc-link-lib=[]" "$@"
