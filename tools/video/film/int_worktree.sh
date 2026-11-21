#!/usr/bin/env bash
# The soundtrack rendered by integration's engine (draft 3: round 41's Chorda, DSP_VERSION
# 11) without putting integration's history on r41/video: a throwaway branch and worktree
# from r41/integration with r41/video merged in, where only the soundtrack example is built.
#
#   bash tools/video/film/int_worktree.sh make     # the worktree, its cargo config, the merge
#   bash tools/video/film/int_worktree.sh merge    # r41/video merged in again (after a change)
#   bash tools/video/film/int_worktree.sh build    # the soundtrack example, release, -j 2
#
# The merge's only conflict so far is docs/DEVELOPING.md's tool table (both rows kept).
# Never commit integration's content onto r41/video; never push r41/video-int.
set -euo pipefail
MAIN="${CANTUS_MAIN:-/f/projects/cantus}"
WT="${VIDEO_INT:-/f/projects/daw-r41/video-int}"
HERE="$(cd "$(dirname "$0")/../../.." && pwd)"
case "${1:-}" in
  make)
    git -C "$MAIN" worktree add -b r41/video-int "$WT" r41/integration
    mkdir -p "$WT/.cargo"
    cp "$HERE/.cargo/config.toml" "$WT/.cargo/config.toml"
    git -C "$WT" merge --no-edit r41/video || {
      echo "resolve the conflicts in $WT (keep both sides of a table), then: git -C $WT commit --no-edit" >&2
      exit 1
    }
    ;;
  merge)
    git -C "$WT" merge --no-edit r41/video
    ;;
  build)
    export PATH="$HOME/.cargo/bin:$PATH"
    export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
    export CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
    cd "$WT"
    grep -n "pub const DSP_VERSION" crates/cantus-dsp/src/lib.rs
    bash tools/build/slot.sh cargo build --release -j 2 -p cantus-ui --example soundtrack
    echo "$WT/target/release/examples/soundtrack.exe"
    ;;
  *)
    echo "usage: int_worktree.sh make|merge|build" >&2
    exit 2
    ;;
esac
