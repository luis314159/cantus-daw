#!/usr/bin/env bash
# One calibration cycle of the jazz kit (Virtuosity Drums' tables, ids 17-24): render
# model:jazz's hits, calibrate against Virtuosity's own hits, regenerate the blob with both
# sets.   bash cycle_jazz.sh <in gains> <out gains> <meas dir name> <studio gains>
set -e
export TMP='F:\projects\daw-r40\tmp' TEMP='F:\projects\daw-r40\tmp' OPENBLAS_NUM_THREADS=1
P=/f/projects/daw-r40/downloads/chorda/venv/Scripts/python.exe
V=/f/projects/daw-r40/voicing-drums
R=F:/projects/daw-r40/tmp/meas9/pack-virtuosity-drums-kit-hits.flac
CONTENT='F:\projects\daw-r40\downloads\tymp-engine\content'
SRC=/f/projects/daw-r40/tymp-engine/crates/cantus-dsp/src
mkdir -p /f/projects/daw-r40/tmp/$3
cd /f/projects/daw-r40/tymp-engine
bash /f/projects/daw-r40/slot.sh cargo run -q -p cantus-ui --example tympanum_measure -- hits "F:\\projects\\daw-r40\\tmp\\$3" "$CONTENT" model:jazz > /dev/null
cd $V
cp out/$1 out/$2 2>/dev/null || echo '{}' > out/$2
$P -I calibrate.py "F:/projects/daw-r40/tmp/$3/model-jazz-hits.wav" $R out/$2
$P -I tables_blob.py $SRC out/fit7.json out/$4 0 out/fitv2.json out/$2 16
echo cycle done
