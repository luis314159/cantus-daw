#!/usr/bin/env bash
# Arcus's violin and viola measured in one pass (round 42, arcus-upper): the study's notes
# rendered (`arcus_upper_listen -- <out> study`) and measured by the study's own estimator
# (`upper_study.py`: the arco scalars and the vibrato's AM per partial beside the real
# instruments' band), then the feature-by-feature comparison with the recordings and the CNSM
# scales' long-term spectrum (`upper-compare.sh`). Prints both; the records go to
# <out>/study/arcus-study.json and <out>/compare/compare.txt.
#
#   bash tools/arcus/upper-measure.sh [out-dir] [refs-dir] [refdb]
#
# Defaults (tools/sound-refs): out $CANTUS_WORK/listen/arcus-upper, refs
# $CANTUS_REFS/arcus-upper/refs, the study's refdb $CANTUS_STUDY_REFDB (an older study folder
# holding tools/ and refdb/ works too), the Python $CANTUS_PY.
set -euo pipefail
here=$(cd "$(dirname "$0")/../.." && pwd)
. "$here/tools/sound-refs/cantus_paths.sh"  # CANTUS_REFS, CANTUS_WORK (tools/sound-refs)
slot=${SLOT:-$here/tools/build/slot.sh}
out=${1:-$CANTUS_WORK/listen/arcus-upper}
refs=${2:-${ARCUS_UPPER_REFS:-$CANTUS_REFS/arcus-upper/refs}}
study=${3:-$CANTUS_STUDY_REFDB}
cd "$here"
rm -rf "$out/study/violin" "$out/study/viola"
bash "$slot" cargo run -p cantus-dsp --release --example arcus_upper_listen -- "$out" study
"$CANTUS_PY" -I tools/arcus/upper_study.py "$out/study" "$study" "$out/study/arcus-study.json"
bash tools/arcus/upper-compare.sh "$refs" "$out/compare" | grep -E "spectrum|centroid|CNSM|Klimke|Levaggi|Stoppani|suggests"
