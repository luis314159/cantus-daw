#!/usr/bin/env bash
# measure.sh NAME: build arcus_match (release), render the held-out notes into $OUT/NAME and
# measure them by the study's estimator into $OUT/NAME.json (round 42 pass 4). OUT defaults to
# $CANTUS_WORK/arcus/p4; the estimator is tools/arcus/study, the bands $CANTUS_STUDY_REFDB, the
# Python $CANTUS_PY (tools/sound-refs). STUDY (an older study folder: tools, refdb, venv) overrides.
# ARCUS_FRICTION=thermal renders with the thermal law.
set -u
cd "$(git rev-parse --show-toplevel)"
. tools/sound-refs/cantus_paths.sh
export PATH="$HOME/.cargo/bin:$PATH"
d=${OUT:-$CANTUS_WORK/arcus/p4}
if [ -n "${STUDY:-}" ]; then
  tools=$STUDY/tools refdb=$STUDY/refdb py=$STUDY/venv/Scripts/python.exe
else
  tools=tools/arcus/study refdb=$CANTUS_STUDY_REFDB py=$CANTUS_PY
fi
mkdir -p "$d"
n=$1
bash tools/build/slot.sh cargo build -p cantus-dsp --release --example arcus_match > $d/$n.build.log 2>&1 || { echo BUILD FAILED; tail -20 $d/$n.build.log; exit 1; }
rm -rf $d/$n; target/release/examples/arcus_match.exe render $d/$n > /dev/null
"$py" -I tools/arcus/study_compare.py "$tools" "$refdb" $d/$n $d/$n.json > $d/$n.log 2>&1
echo MEASURED $n
