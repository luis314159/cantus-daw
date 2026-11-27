#!/usr/bin/env bash
# r42 arcus-classical pass 2 — the owner's classical page: the last ahora-* kept as antes2-*, new ahora-* cut from runs/<run> at the same windows and loudness (clips.py). page2.sh <run>
# page2.sh <run>: the classical page's pass-2 takes from runs/<run> (solos + quartet)
set -eu
B=${CANTUS_WORK:-F:/projects/cantus-work}/arcus-classical
L=${CANTUS_WORK:-F:/projects/cantus-work}/listen/arcus-clasico
R=$1
PY=/f/projects/cantus-refs/venv/Scripts/python.exe
C=$(cd "$(dirname "$0")" && pwd)/clips.py
cd $L
for k in violin viola violonchelo contrabajo cuarteto; do
  [ -f antes2-$k.wav ] || mv ahora-$k.wav antes2-$k.wav
done
for k in violin viola violonchelo contrabajo; do
  read t0 dur lufs < <($PY -I $C find antes2-$k.wav $B/runs/c11/solos/$k-interpretado.wav)
  t1=$(python -c "print($t0+$dur)")
  $PY -I $C cut ahora-$k.wav $B/runs/$R/solos/$k-interpretado.wav $t0 $t1 $lufs
done
lufs=$($PY -I $C lufs antes2-cuarteto.wav)
q=$B/runs/$R/quartet/arcus-cuarteto.wav
dur=$($PY -c "import soundfile as sf;print(sf.info('$q').duration)")
$PY -I $C cut ahora-cuarteto.wav $q 0 $dur $lufs
ls -la $L/*.wav | awk '{print $5, $9}'
