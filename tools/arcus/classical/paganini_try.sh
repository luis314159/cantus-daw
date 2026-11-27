#!/usr/bin/env bash
# r42 arcus-classical pass 2 — try a stroke shape / EQ on the Paganini scores: paganini_try.sh <name> <paganini_render.exe> [ARCUS_SHAPE]
# (ARCUS_EQ=... in the environment tries a correction; PAGANINI_LIVE=1 renders without the look-ahead).
# The work copy is $CANTUS_WORK/arcus-classical/try/<name>, seeded from $PAGANINI_WORK (a copy of listen/paganini).
# pagtry.sh <name> <exe> [shape]: render the one-string var2 + theme aligned only, mix+compare in its own W
set -eu
name=$1; R=$2; shape=${3:-}
base=/f/projects/cantus-work/arcus-classical
W=$base/try/$name
mkdir -p $W/work
cp -n ${PAGANINI_WORK:-$base/paganini}/work/*.txt $base/paganini/work/notes.json $base/paganini/work/aula-*.wav $base/paganini/work/actual-*.wav $base/paganini/work/pasaje-original.wav $W/work/ 2>/dev/null || true
cp -n ${PAGANINI_WORK:-$base/paganini}/original.wav $W/ 2>/dev/null || true
for t in alineado-tema alineado-var2 alineado-var2-1cuerda partitura-tema partitura-var2; do
  ARCUS_SHAPE="$shape" "$R" "$W/work/arcus-$t.txt" "$W/work/mejora-$t-seco.wav" > /dev/null &
done
wait
py=/f/projects/cantus-refs/venv/Scripts/python.exe
repo=/f/projects/daw-r42/arcus-classical
$py -I $repo/tools/listening/paganini_mix.py "$W/work" "$W" > "$W/mix.log" 2>&1
$py -I $repo/tools/listening/paganini_compare.py "$W/work" "$W" > "$W/compare.log" 2>&1
$py -I $repo/tools/arcus/classical/paganini_shapes.py $W | tail -4
grep "mejora-alineado-1cuerda-sala ltas" $W/compare.log | head -1
