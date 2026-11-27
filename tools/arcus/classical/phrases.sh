#!/usr/bin/env bash
# r42 arcus-classical pass 2 — the solos through the engine (arcus_demo solos) and both phrase measures. phrases.sh <run> <arcus_demo.exe>
# phr.sh <run> <demo exe>: solos + phrase measures (HSD phrase.py and the strings-hq Musopen+HSD tool)
set -u
W=/f/projects/cantus-work/arcus-classical
R=$1; D=$2
repo=/f/projects/daw-r42/arcus-classical
PY=/f/projects/cantus-refs/venv/Scripts/python.exe
mkdir -p "$W/runs/$R/solos"
"$D" "$W/runs/$R/solos" solos > "$W/runs/$R/solos.log" 2>&1
cd $repo
$PY -I tools/arcus/classical/phrase.py "$W/runs/$R/phrase.json" "$W/runs/$R/solos" /f/projects/cantus-work/arcus-analisis/phrase.json > "$W/runs/$R/phrase.txt" 2>&1
for f in violin:violin viola:viola cello:violonchelo; do
  fam=${f%%:*}; n=${f##*:}
  $PY -I tools/soundlab/adapters/strings_hq_phrases.py measure tools/soundlab/subjects/arcus-$fam /f/projects/cantus-refs "$W/runs/$R/hqphrase-$fam.json" arcus=F:/projects/cantus-work/arcus-classical/runs/$R/solos/$n-interpretado.wav:$fam > "$W/runs/$R/hqphrase-$fam.txt" 2>&1 &
done
wait
cat "$W/runs/$R/phrase.txt"
for fam in violin viola cello; do echo "== $fam"; tail -8 "$W/runs/$R/hqphrase-$fam.txt"; done
