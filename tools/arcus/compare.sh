#!/usr/bin/env bash
# A render beside a recording (round 42): for each WAV, the fundamental near <near-Hz>, harmonics
# 1–10 against the first, the noise between harmonics by octave band, the centroid, the onset.
#
#   bash tools/arcus/compare.sh <near-Hz> <a.wav> [b.wav …]
set -euo pipefail
here=$(cd "$(dirname "$0")/../.." && pwd)
slot=${SLOT:-$here/tools/build/slot.sh}
cd "$here"
bash "$slot" cargo run -q -p cantus-dsp --release --example arcus_compare -- "$@"
