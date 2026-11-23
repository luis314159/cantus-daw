#!/usr/bin/env bash
# Arcus's side of the blind ABX (round 42): tools/arcus/abx_phrase.mid rendered by the cello
# and the double bass, 48 kHz 24-bit, peak −1 dBFS (abx-cello.wav, abx-bass.wav) and
# loudness-matched to −23 LUFS (abx-*-lufs23.wav) — the names the recorded strings use.
#
#   bash tools/arcus/abx.sh [out-dir]      # default: $ARCUS_LISTEN or ../listen/arcus
set -euo pipefail
here=$(cd "$(dirname "$0")/../.." && pwd)
out=${1:-${ARCUS_LISTEN:-$here/../listen/arcus}}
slot=${SLOT:-$here/tools/build/slot.sh}
mkdir -p "$out"
cd "$here"
bash "$slot" cargo run -p cantus-dsp --release --example arcus_abx -- tools/arcus/abx_phrase.mid "$out"
