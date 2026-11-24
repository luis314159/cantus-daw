#!/usr/bin/env bash
# measure.sh NAME: build arcus_match (release), render the held-out notes into $OUT/NAME and
# measure them by the study's estimator into $OUT/NAME.json (round 42 pass 4). OUT defaults to
# F:/projects/daw-r42/tmp/arcus/p4; STUDY to F:/projects/daw-r42/study (its tools, refdb, venv).
# ARCUS_FRICTION=thermal renders with the thermal law.
set -u
cd "$(git rev-parse --show-toplevel)"
export PATH="$HOME/.cargo/bin:$PATH"
d=${OUT:-/f/projects/daw-r42/tmp/arcus/p4}
st=${STUDY:-/f/projects/daw-r42/study}
mkdir -p "$d"
n=$1
bash tools/build/slot.sh cargo build -p cantus-dsp --release --example arcus_match > $d/$n.build.log 2>&1 || { echo BUILD FAILED; tail -20 $d/$n.build.log; exit 1; }
rm -rf $d/$n; target/release/examples/arcus_match.exe render $d/$n > /dev/null
$st/venv/Scripts/python.exe -I tools/arcus/study_compare.py $st/tools $st/refdb $d/$n $d/$n.json > $d/$n.log 2>&1
echo MEASURED $n
