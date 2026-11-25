#!/usr/bin/env bash
# One calibration cycle: render the modelled kit's hits through the engine with the tables
# as they are, calibrate level and tonal weight against DRSKit, regenerate the tables.
#   bash cycle.sh <in gains> <out gains> <meas dir name> [ab]
# Environment (README.md, "Paths"): TYMP_CONTENT (the content root with the recorded packs
# installed; default $CANTUS_REFS/tympanum/content, tools/sound-refs), TYMP_WORK (fits in
# out/, renders in <meas dir name>/, the recordings' hits in meas9/; default
# $CANTUS_WORK/tympanum-work), CANTUS_PY, SLOT (empty inside a build slot).
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
. "$ROOT/tools/sound-refs/cantus_paths.sh"  # CANTUS_REFS, CANTUS_WORK, CANTUS_PY
W=${TYMP_WORK:-$CANTUS_WORK/tympanum-work}
CONTENT=${TYMP_CONTENT:-$CANTUS_REFS/tympanum/content}
P=${CANTUS_PY:-python}
SLOT=${SLOT-bash $ROOT/tools/build/slot.sh}
export OPENBLAS_NUM_THREADS=1
# the jazz kit's tables too, once fitted: JAZZ="$W/out/fitv2.json $W/out/gainsv.json 16"
JAZZ=${JAZZ:-}
R=${TYMP_REC:-$W/meas9/pack-drskit-tympanum-kit-hits.flac}
mkdir -p "$W/$3"
cd "$ROOT"
$SLOT cargo run -q -p cantus-ui --example tympanum_measure -- hits "$W/$3" "$CONTENT" model:studio > /dev/null
if [ "${4:-}" = "ab" ]; then
  $SLOT cargo run -q -p cantus-ui --example tympanum_measure -- ab "$W/$3" "$CONTENT" drskit-tympanum/kit | tail -2
fi
cp "$W/out/$1" "$W/out/$2"
$P -I "$HERE/calibrate.py" "$W/$3/model-studio-hits.wav" "$R" "$W/out/$2"
$P -I "$HERE/tables_blob.py" "$ROOT/crates/cantus-dsp/src" "$W/out/fit7.json" "$W/out/$2" 0 $JAZZ
echo cycle done
