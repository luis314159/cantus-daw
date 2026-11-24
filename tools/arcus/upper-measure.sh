#!/usr/bin/env bash
# Arcus's violin and viola measured in one pass (round 42, arcus-upper): the study's notes
# rendered (`arcus_upper_listen -- <out> study`) and measured by the study's own estimator
# (`upper_study.py`: the arco scalars and the vibrato's AM per partial beside the real
# instruments' band), then the feature-by-feature comparison with the recordings and the CNSM
# scales' long-term spectrum (`upper-compare.sh`). Prints both; the records go to
# <out>/study/arcus-study.json and <out>/compare/compare.txt.
#
#   bash tools/arcus/upper-measure.sh [out-dir] [refs-dir] [study-dir]
set -euo pipefail
here=$(cd "$(dirname "$0")/../.." && pwd)
slot=${SLOT:-$here/tools/build/slot.sh}
out=${1:-$here/../listen/arcus-upper}
refs=${2:-${ARCUS_UPPER_REFS:-$here/../downloads/arcus-upper/refs}}
study=${3:-$here/../study}
cd "$here"
rm -rf "$out/study/violin" "$out/study/viola"
bash "$slot" cargo run -p cantus-dsp --release --example arcus_upper_listen -- "$out" study
"$study/venv/Scripts/python.exe" -I tools/arcus/upper_study.py "$out/study" "$study" "$out/study/arcus-study.json"
bash tools/arcus/upper-compare.sh "$refs" "$out/compare" | grep -E "spectrum|centroid|CNSM|Klimke|Levaggi|Stoppani|suggests"
