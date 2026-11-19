#!/usr/bin/env bash
# One calibration cycle of the jazz kit (Virtuosity Drums' tables, ids 17-24): render
# model:jazz's hits, calibrate against Virtuosity's own hits, regenerate the blob with both
# sets.   bash cycle_jazz.sh <in gains> <out gains> <meas dir name> <studio gains>
# Environment as cycle.sh's (TYMP_CONTENT, TYMP_WORK, CANTUS_PY, SLOT).
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
W=${TYMP_WORK:-$ROOT/target/tympanum-work}
CONTENT=${TYMP_CONTENT:?set TYMP_CONTENT to the content root holding the recorded packs}
P=${CANTUS_PY:-python}
SLOT=${SLOT-bash $ROOT/tools/build/slot.sh}
export OPENBLAS_NUM_THREADS=1
R=${TYMP_REC:-$W/meas9/pack-virtuosity-drums-kit-hits.flac}
SRC=$ROOT/crates/cantus-dsp/src
mkdir -p "$W/$3"
cd "$ROOT"
$SLOT cargo run -q -p cantus-ui --example tympanum_measure -- hits "$W/$3" "$CONTENT" model:jazz > /dev/null
cp "$W/out/$1" "$W/out/$2" 2>/dev/null || echo '{}' > "$W/out/$2"
$P -I "$HERE/calibrate.py" "$W/$3/model-jazz-hits.wav" "$R" "$W/out/$2"
$P -I "$HERE/tables_blob.py" "$SRC" "$W/out/fit7.json" "$W/out/$4" 0 "$W/out/fitv2.json" "$W/out/$2" 16
echo cycle done
