#!/usr/bin/env bash
# One calibration cycle: render the modelled kit's hits through the engine with the tables
# as they are, calibrate level and tonal weight against DRSKit, regenerate the tables.
#   bash cycle.sh <in gains> <out gains> <meas dir name> [ab]
set -e
export TMP='F:\projects\daw-r40\tmp' TEMP='F:\projects\daw-r40\tmp' OPENBLAS_NUM_THREADS=1
P=/f/projects/daw-r40/downloads/chorda/venv/Scripts/python.exe
V=/f/projects/daw-r40/voicing-drums
# the jazz kit's tables too, once fitted: JAZZ="out/fitv2.json out/gainsv.json 16"
JAZZ=${JAZZ:-}
R=F:/projects/daw-r40/tmp/meas9/pack-drskit-tympanum-kit-hits.flac
CONTENT='F:\projects\daw-r40\downloads\tymp-engine\content'
mkdir -p /f/projects/daw-r40/tmp/$3
cd /f/projects/daw-r40/tymp-engine
bash /f/projects/daw-r40/slot.sh cargo run -q -p cantus-ui --example tympanum_measure -- hits "F:\\projects\\daw-r40\\tmp\\$3" "$CONTENT" model:studio > /dev/null
if [ "${4:-}" = "ab" ]; then
  bash /f/projects/daw-r40/slot.sh cargo run -q -p cantus-ui --example tympanum_measure -- ab "F:\\projects\\daw-r40\\tmp\\$3" "$CONTENT" drskit-tympanum/kit | tail -2
fi
cd $V
cp out/$1 out/$2
$P -I calibrate.py "F:/projects/daw-r40/tmp/$3/model-studio-hits.wav" $R out/$2
$P -I tables_blob.py /f/projects/daw-r40/tymp-engine/crates/cantus-dsp/src out/fit7.json out/$2 0 $JAZZ
echo cycle done
