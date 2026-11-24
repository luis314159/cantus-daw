#!/usr/bin/env bash
# Arcus's renders for the owner's ears (round 42, ADR 0157): the cello and the double bass —
# a solo note at pp, mf and ff, a four-bar legato phrase with a CC1 swell, pizzicato and
# tremolo — as 48 kHz 24-bit WAVs, straight from the DSP (`crates/cantus-dsp/examples/
# arcus_listen.rs`).
#
#   bash tools/arcus/listen.sh [out-dir] [levels]
#
# out-dir defaults to $ARCUS_LISTEN or ../listen/arcus beside the worktree; `levels` prints
# each render's peak and RMS. Every cargo command goes through tools/build/slot.sh (three
# builds at once on a shared machine); set SLOT to the slot script's path if this tree has
# none of its own.
set -euo pipefail
here=$(cd "$(dirname "$0")/../.." && pwd)
out=${1:-${ARCUS_LISTEN:-$here/../listen/arcus}}
shift || true
slot=${SLOT:-$here/tools/build/slot.sh}
mkdir -p "$out"
cd "$here"
bash "$slot" cargo run -p cantus-dsp --release --example arcus_listen -- "$out" "$@"
