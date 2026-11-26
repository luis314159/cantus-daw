#!/usr/bin/env bash
# r42 arcus-classical — Arcus's four classical instruments through the sound lab (ADR 0160) at
# their defaults, on the strings-hq subjects (`tools/soundlab/subjects/arcus-*`, r42
# strings-refs): the held-out notes rendered by `arcus_match render <dir> violin viola cello
# bass` (every key of the violin and the viola, the held-out third of the cello and the bass),
# measured and judged on the held-out cells, then the classical pass's rows side by side with an
# earlier run (`summary.py`).
#
#   bash tools/arcus/classical/lab.sh <run> [<bin dir>] [<before run>]
#
# <bin dir> holds `arcus_match` already built (default target/release/examples: build it with
# `tools/build/slot.sh cargo build --release -p cantus-dsp --example arcus_match`); <before run>
# defaults to `hq1` (r42/integration's Arcus on these subjects, the strings-refs builder's run,
# copied with its references' cache into this pass's own work folder). Renders go to
# $CANTUS_WORK/arcus-classical/runs/<run>/; the lab's work to $SOUNDLAB_WORK (default
# $CANTUS_WORK/arcus-classical/soundlab — never the refs builder's soundlab-hq).
# INSTS (default all four) names the instruments to measure, e.g. INSTS="violin cello";
# ARCUS_SHAPE=name=value,… is handed to the renders (a stroke shape to try, `shape::Knobs`).
#
# The analysis's scratch subjects (`subjects/`) are kept for its runs `c1`…`c8` (by this script's
# earlier version, in git history).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)
run=$1
bin=${2:-$repo/target/release/examples}
before=${3:-hq1}
work=${CANTUS_WORK:-/f/projects/cantus-work}
export SOUNDLAB_WORK=${SOUNDLAB_WORK:-$work/arcus-classical/soundlab}
out=$work/arcus-classical/runs/$run
py=${CANTUS_PY:-/f/projects/cantus-refs/venv/Scripts/python.exe}
insts=${INSTS:-violin viola cello bass}
mkdir -p "$out"
# shellcheck disable=SC2086
"$bin/arcus_match" render "$out/notes" $insts > "$out/render.log" 2>&1
lab() {
  SOUNDLAB_WORKERS=${SOUNDLAB_WORKERS:-2} "$py" -I "$repo/tools/soundlab/lab.py" all \
    "$repo/tools/soundlab/subjects/arcus-$1/subject.json" "$run" \
    --from "$out/notes" --before "$before" > "$out/lab-$1.log" 2>&1
}
set -- $insts
while [ $# -gt 0 ]; do
  lab "$1" &
  if [ $# -gt 1 ]; then
    lab "$2" &
    shift
  fi
  shift
  wait
done
sl=$SOUNDLAB_WORK
cols=()
for i in $insts; do
  cols+=("$i<=$sl/arcus-$i/runs/$before" "$i=$sl/arcus-$i/runs/$run")
done
"$py" -I "$here/summary.py" "$out/summary.json" "${cols[@]}" | tee "$out/summary.txt"
